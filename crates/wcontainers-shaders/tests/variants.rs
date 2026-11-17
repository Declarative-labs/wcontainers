//! What `convert` refuses and what a variant changes.

use std::path::Path;
use wcontainers_gpu::{Interpolation, VariantFlags, VariantKey};
use wcontainers_shaders::{ConvertError, Source, convert};

fn read(path: &str) -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap()
}

fn rig(name: &str) -> Vec<u8> {
    read(&format!("../wcontainers-graphics-test/shaders/{name}"))
}

fn wgsl(c: &wcontainers_shaders::Converted) -> String {
    let m = &c.modules[0];
    naga::back::wgsl::write_string(&m.module, &m.info, naga::back::wgsl::WriterFlags::empty())
        .unwrap()
}

/// Every rewrite not built yet is refused by name on both routes, never answered with the plain
/// program.
#[test]
fn unbuilt_variants_are_refused_by_name() {
    let dxbc = rig("dxbc/samplers.ps_sample.dxbc");
    let dxil = rig("dxil/samplers.ps_sample.dxil");
    let cases: Vec<(VariantKey, &str)> = vec![
        (
            VariantKey {
                flags: VariantFlags::CLIP_DEPTH_REWRITE,
                ..Default::default()
            },
            "VariantFlags",
        ),
        (
            VariantKey {
                filter_in_shader: 1,
                ..Default::default()
            },
            "FilterInShader",
        ),
        (
            VariantKey {
                lod_bias: 1,
                ..Default::default()
            },
            "LodBiasInShader",
        ),
        (
            VariantKey {
                constants_as_storage: 1,
                ..Default::default()
            },
            "ConstantsAsStorage",
        ),
        (
            VariantKey {
                min_lod_clamped: 1,
                ..Default::default()
            },
            "MinLodClamp",
        ),
        (
            VariantKey {
                samples: 4,
                ..Default::default()
            },
            "SampleCount",
        ),
        (
            VariantKey {
                layout: [1; 32],
                ..Default::default()
            },
            "RootSignatureLayout",
        ),
    ];
    for (key, name) in cases {
        for source in [Source::Dxbc(&dxbc), Source::Dxil(&dxil)] {
            let got = convert(source, &key, None).map(|_| ());
            assert_eq!(got, Err(ConvertError::Unsupported(name)), "{key:?}");
        }
    }
    let got = convert(Source::Dxbc(&dxbc), &VariantKey::default(), Some(&[0; 4])).map(|_| ());
    assert_eq!(got, Err(ConvertError::Unsupported("RootSignatureLayout")));
    assert!(convert(Source::Dxbc(&dxbc), &VariantKey::default(), None).is_ok());
    assert!(convert(Source::Dxil(&dxil), &VariantKey::default(), None).is_ok());
}

/// A container whose checksum does not match its bytes is refused, as Direct3D refuses it.
#[test]
fn a_container_with_a_wrong_checksum_is_refused() {
    for (name, mut bytes, dxil) in [
        ("triangle.ps.dxbc", rig("dxbc/triangle.ps.dxbc"), false),
        ("triangle.ps.dxil", rig("dxil/triangle.ps.dxil"), true),
    ] {
        let at = bytes.len() - 8;
        bytes[at] ^= 1;
        let source = if dxil {
            Source::Dxil(&bytes)
        } else {
            Source::Dxbc(&bytes)
        };
        match convert(source, &VariantKey::default(), None) {
            Err(ConvertError::Rejected(why)) => assert!(why.contains("checksum"), "{name}: {why}"),
            other => panic!("{name}: {:?}", other.map(|_| ())),
        }
    }
}

/// Two programs at one register in two register spaces are refused until a root signature numbers
/// them, on both routes.
#[test]
fn register_spaces_are_refused_under_direct3d_11_numbering() {
    for source in [
        Source::Dxbc(&read("tests/own/ps_spaces.dxbc")),
        Source::Dxil(&read("tests/own/ps_spaces.dxil")),
    ] {
        let got = convert(source, &VariantKey::default(), None).map(|_| ());
        assert_eq!(got, Err(ConvertError::Unsupported("RegisterSpace")));
    }
}

/// A vertex program writes each output as the pixel program reads it, taken from the pixel program's
/// reflection, so the two link on WebGPU and GLSL ES.
#[test]
fn vertex_outputs_take_the_pixel_programs_interpolation() {
    let ps = convert(
        Source::Dxbc(&read("tests/own/ps_flat.dxbc")),
        &VariantKey::default(),
        None,
    )
    .unwrap();
    let asked: Vec<_> = ps
        .reflection
        .inputs
        .iter()
        .filter(|v| v.system.is_none() && v.interpolation != Interpolation::Perspective)
        .map(|v| (v.location, v.interpolation))
        .collect();
    assert_eq!(asked, [(1, Interpolation::Flat)]);
    let key = VariantKey {
        output_interpolation: asked,
        ..Default::default()
    };
    let vs = convert(Source::Dxbc(&read("tests/own/vs_shade.dxbc")), &key, None).unwrap();
    let text = wgsl(&vs);
    assert!(text.contains("@location(1) @interpolate(flat)"), "{text}");
    assert!(!text.contains("@location(2) @interpolate(flat)"), "{text}");
    let out = vs
        .reflection
        .outputs
        .iter()
        .find(|v| v.location == 1)
        .unwrap();
    assert_eq!(out.interpolation, Interpolation::Flat);

    // The same on the DXIL route, for the rig's vertex shader's colour.
    let key = VariantKey {
        output_interpolation: vec![(1, Interpolation::LinearCentroid)],
        ..Default::default()
    };
    let vs = convert(Source::Dxil(&rig("dxil/triangle.vs.dxil")), &key, None).unwrap();
    let text = wgsl(&vs);
    assert!(
        text.contains("@location(1) @interpolate(linear, centroid)"),
        "{text}"
    );
}
