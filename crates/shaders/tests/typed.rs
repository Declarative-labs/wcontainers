//! Typed buffers of a program another route brought into the common form
//! (`wcontainers_shaders::typed`): `cs_texels` written as such a module (`tests/own/typed_texels.wgsl`)
//! takes the views `convert` gives fxc's build, on both routes that are not native, and is refused by
//! the DXBC route's names. Drawn against real Direct3D 11 in `tests/compute_draw.rs`.

mod forms;

use forms::{TYPED_TEXELS, typed_texels};
use naga::{AddressSpace, ImageClass, TypeInner};
use std::path::Path;
use wcontainers_gpu::{Format, HelperValue, TexelBufferRoute, VariantKey};
use wcontainers_shaders::typed::{self, TypedBuffer};
use wcontainers_shaders::{ConvertError, Converted, Source, convert, refusal};

fn key(route: TexelBufferRoute) -> VariantKey {
    VariantKey {
        texel_buffers: route,
        texel_formats: vec![
            (0, Format::Rgba8Unorm),
            (1, Format::Rgba16Uint),
            (2, Format::Rg8Sint),
            (3, Format::Rg32Float),
        ],
        storage_formats: vec![(0, Format::Rgba16Float), (1, Format::R32Uint)],
        ..Default::default()
    }
}

/// The (set, binding) of every view in the helper constants, in order.
fn views(c: &Converted) -> Vec<(u32, u32)> {
    let mut v: Vec<_> = c
        .reflection
        .helpers
        .iter()
        .filter_map(|h| match h.value {
            HelperValue::TexelBufferView { set, binding } => Some((set, binding)),
            _ => None,
        })
        .collect();
    v.sort();
    v
}

fn refused(r: Result<Converted, ConvertError>) -> &'static str {
    match r {
        Err(ConvertError::Unsupported(name)) => name,
        other => panic!("{:?}", other.map(|_| ())),
    }
}

#[test]
fn typed_buffers_of_a_module_take_the_views_convert_gives() {
    let k = key(TexelBufferRoute::Storage);
    let made = typed_texels(&k, &TYPED_TEXELS).unwrap();
    let bytes =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/own/cs_texels.dxbc"))
            .unwrap();
    let fxc = convert(Source::Dxbc(&bytes), &k, None).unwrap();
    assert_eq!(views(&made), views(&fxc));
    assert_eq!(views(&made).len(), TYPED_TEXELS.len());
    let m = &made.modules[0].module;
    for name in TYPED_TEXELS {
        let g = m
            .global_variables
            .iter()
            .find(|(_, g)| g.name.as_deref() == Some(name));
        let (_, g) = g.unwrap();
        assert!(matches!(g.space, AddressSpace::Storage { .. }), "{name}");
    }

    // On the texture route the texture slots' buffers are 2D textures of their views' kinds.
    let k = key(TexelBufferRoute::Texture);
    let read_only = &TYPED_TEXELS[..4];
    let made = typed_texels(&k, read_only).unwrap();
    assert_eq!(views(&made), views(&fxc)[..4]);
    let m = &made.modules[0].module;
    for (name, kind) in read_only.iter().zip([
        naga::ScalarKind::Float,
        naga::ScalarKind::Uint,
        naga::ScalarKind::Sint,
        naga::ScalarKind::Float,
    ]) {
        let (_, g) = m
            .global_variables
            .iter()
            .find(|(_, g)| g.name.as_deref() == Some(*name))
            .unwrap();
        assert_eq!(g.space, AddressSpace::Handle, "{name}");
        assert!(
            matches!(
                m.types[g.ty].inner,
                TypeInner::Image { class: ImageClass::Sampled { kind: k, multi: false }, .. }
                    if k == kind
            ),
            "{name}"
        );
    }
}

#[test]
fn typed_buffers_of_a_module_are_refused_by_name() {
    let plain = key(TexelBufferRoute::Storage);
    assert_eq!(
        refused(typed_texels(
            &VariantKey {
                texel_buffers: TexelBufferRoute::Native,
                ..plain.clone()
            },
            &TYPED_TEXELS
        )),
        refusal::TYPED_BUFFER
    );
    let mut k = plain.clone();
    k.texel_formats.remove(0);
    assert_eq!(
        refused(typed_texels(&k, &TYPED_TEXELS)),
        refusal::TYPED_BUFFER_FORMAT
    );
    let mut k = plain.clone();
    k.storage_formats[1].1 = Format::Rg16Uint;
    assert_eq!(
        refused(typed_texels(&k, &TYPED_TEXELS)),
        refusal::TYPED_BUFFER_ATOMIC_FORMAT
    );
    let mut k = plain.clone();
    k.texel_buffers = TexelBufferRoute::Texture;
    assert_eq!(
        refused(typed_texels(&k, &TYPED_TEXELS)),
        refusal::TYPED_BUFFER_STORAGE_AS_TEXTURE
    );

    // A stand-in whose arguments or result are not the function's, and a store into a view bound
    // read only, are malformed.
    let malformed = |kind, store: Option<&str>| {
        let (mut m, _) = forms::module("typed_texels");
        let mut reflection = forms::typed_texels_reflection();
        let buffer = TypedBuffer {
            global: forms::variable(&m, "floats"),
            kind,
            load: Some(forms::function(&m, "floats_load")),
            store: store.map(|s| forms::function(&m, s)),
            length: None,
            atomics: Vec::new(),
        };
        typed::buffers(&mut m, &mut reflection, &plain, &[buffer])
    };
    assert!(malformed(naga::ScalarKind::Float, None).is_ok());
    for (kind, store) in [
        (naga::ScalarKind::Uint, None),
        (naga::ScalarKind::Float, Some("written_store")),
    ] {
        assert!(
            matches!(malformed(kind, store), Err(ConvertError::Malformed(_))),
            "{kind:?} {store:?}"
        );
    }
}
