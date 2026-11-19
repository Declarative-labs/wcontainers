//! What `convert` refuses and what a variant changes.

use std::path::Path;
use wcontainers_gpu::{
    AddedBinding, DrawBaseSource, Format, HelperSlot, HelperValue, Interpolation, LinkedValues,
    ScalarKind, SlotKind, SystemValue, TargetInShader, TexelBufferRoute, VariantFlags, VariantKey,
    VertexFormat,
};
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

/// The GLSL ES 3.00 the WebGL 2 back end writes (its options, as `tests/corpus.rs` copies them).
fn glsl(c: &wcontainers_shaders::Converted) -> Result<String, String> {
    let m = &c.modules[0];
    let options = naga::back::glsl::Options {
        version: naga::back::glsl::Version::Embedded {
            version: 300,
            is_webgl: true,
        },
        writer_flags: naga::back::glsl::WriterFlags::FORCE_POINT_SIZE,
        ..Default::default()
    };
    let pipeline = naga::back::glsl::PipelineOptions {
        shader_stage: m.module.entry_points[0].stage,
        entry_point: wcontainers_shaders::ENTRY_POINT.into(),
        multiview: None,
    };
    let mut text = String::new();
    naga::back::glsl::Writer::new(
        &mut text,
        &m.module,
        &m.info,
        &options,
        &pipeline,
        naga::proc::BoundsCheckPolicies::default(),
    )
    .and_then(|mut w| w.write().map(|_| ()))
    .map_err(|e| format!("{e}"))?;
    Ok(text)
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
                flags: VariantFlags::PROVOKING_LAST,
                ..Default::default()
            },
            "ProvokingRewrite",
        ),
        (
            VariantKey {
                flags: VariantFlags::LINE_COVERAGE | VariantFlags::CLIP_DEPTH_REWRITE,
                ..Default::default()
            },
            "AntialiasedLinesAsQuads",
        ),
        (
            VariantKey {
                storage_packed: 1,
                ..Default::default()
            },
            "StoragePacked",
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

/// Direct3D 12's numbering (DXIL, and DXBC of shader model 5.1, which only Direct3D 12 runs): each
/// kind's slots are the program's resources in (space, register) order, so two textures at one
/// register in two spaces get the first two texture bindings and the sampled pairs name slots.
/// Shader model 5.0 keeps Direct3D 11's numbering, the register.
#[test]
fn register_spaces_are_numbered_by_the_programs_own_slots() {
    for source in [
        Source::Dxbc(&read("tests/own/ps_spaces.dxbc")),
        Source::Dxil(&read("tests/own/ps_spaces.dxil")),
    ] {
        let c = convert(source, &VariantKey::default(), None).unwrap();
        let mut bindings: Vec<_> = c
            .reflection
            .bindings
            .iter()
            .map(|b| (b.kind, b.space, b.register, b.set, b.binding, b.slot()))
            .collect();
        bindings.sort_by_key(|b| b.4);
        assert_eq!(
            bindings,
            [
                (SlotKind::Sampler, 0, 0, 1, 16, 0),
                (SlotKind::Texture, 0, 0, 1, 32, 0),
                (SlotKind::Texture, 1, 0, 1, 33, 1),
            ]
        );
        let mut pairs = c.reflection.sampled_pairs.clone();
        pairs.sort();
        assert_eq!(pairs, [(0, 0), (1, 0)]);
    }
    // Shader model 5.0: the register is the slot, gaps kept.
    let c = convert(
        Source::Dxbc(&read("tests/own/ps_textured.dxbc")),
        &VariantKey::default(),
        None,
    )
    .unwrap();
    assert!(c.reflection.bindings.iter().all(|b| b.slot() == b.register));
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

fn own_pair(name: &str) -> [Vec<u8>; 2] {
    [
        read(&format!("tests/own/{name}.dxbc")),
        read(&format!("tests/own/{name}.dxil")),
    ]
}

/// Converts by both routes under `key`; returns each route's result.
fn both(code: &[Vec<u8>; 2], key: &VariantKey) -> [wcontainers_shaders::Converted; 2] {
    [
        convert(Source::Dxbc(&code[0]), key, None).unwrap(),
        convert(Source::Dxil(&code[1]), key, None).unwrap(),
    ]
}

fn slot(value: HelperValue, offset: u32) -> HelperSlot {
    HelperSlot { value, offset }
}

/// The last vertex stage's rewrites: the viewport adjustment's helper follows the draw's bases in one
/// block, and clip depth is rewritten.
#[test]
fn vertex_variants_extend_the_helper_block_on_both_routes() {
    let vs = own_pair("vs_transform");
    let key = VariantKey {
        flags: VariantFlags::VIEWPORT_ADJUST
            | VariantFlags::VIEWPORT_DEPTH_FLIP
            | VariantFlags::CLIP_DEPTH_REWRITE,
        draw_base: DrawBaseSource::Helpers,
        ..Default::default()
    };
    for c in both(&vs, &key) {
        assert_eq!(
            c.reflection.helpers,
            [
                slot(HelperValue::DrawBase, 0),
                slot(HelperValue::ViewportAdjust, 16)
            ]
        );
        assert_eq!(c.reflection.helper_size, 32);
        let text = wgsl(&c);
        assert!(text.contains("viewport: vec4<f32>"), "{text}");
        assert!(text.contains("2f *"), "{text}");
    }
    let key = VariantKey {
        flags: VariantFlags::VIEWPORT_ADJUST,
        ..Default::default()
    };
    for c in both(&vs, &key) {
        assert_eq!(c.reflection.helpers, [slot(HelperValue::ViewportAdjust, 0)]);
        assert_eq!(c.reflection.helper_size, 16);
    }
}

/// The pixel stage's rewrites on both routes: the sample mask, counting, dual-source outputs, the
/// level-of-detail bias and level counts from the helper block.
/// Early tests in the program read the attachment's depth and stencil at their helper bindings,
/// each covered sample's on four samples, and discard in the prologue; other counts are refused.
#[test]
fn early_tests_read_the_attachment_before_the_program() {
    let textured = own_pair("ps_textured");
    let key = |samples, flags| VariantKey {
        flags: VariantFlags::EARLY_TESTS | flags,
        samples,
        ..Default::default()
    };
    for c in both(&textured, &key(1, VariantFlags::empty())) {
        assert_eq!(c.reflection.helpers, [slot(HelperValue::EarlyTests, 0)]);
        assert_eq!(c.reflection.helper_size, 32);
        let text = wgsl(&c);
        assert!(
            text.contains("@group(3) @binding(9) \nvar early_depth: texture_depth_2d;"),
            "{text}"
        );
        assert!(
            text.contains("@group(3) @binding(10) \nvar early_stencil: texture_2d<u32>;"),
            "{text}"
        );
        assert!(text.contains("discard;"), "{text}");
    }
    for c in both(&textured, &key(4, VariantFlags::empty())) {
        let text = wgsl(&c);
        assert!(text.contains("texture_depth_multisampled_2d"), "{text}");
        assert!(text.contains("@builtin(sample_mask)"), "{text}");
    }
    for c in both(&textured, &key(1, VariantFlags::DEPTH_BIAS)) {
        assert_eq!(
            c.reflection.helpers,
            [
                slot(HelperValue::DepthBias, 0),
                slot(HelperValue::EarlyTests, 16)
            ]
        );
    }
    for (samples, name) in [(0, "SampleCount"), (2, "EarlyTestsSamples")] {
        for source in [Source::Dxbc(&textured[0]), Source::Dxil(&textured[1])] {
            let got = convert(source, &key(samples, VariantFlags::empty()), None).map(|_| ());
            assert_eq!(got, Err(ConvertError::Unsupported(name)));
        }
    }
}

#[test]
fn pixel_variants_convert_on_both_routes() {
    let textured = own_pair("ps_textured");
    let key = VariantKey {
        flags: VariantFlags::SAMPLE_MASK,
        ..Default::default()
    };
    for c in both(&textured, &key) {
        assert_eq!(c.reflection.helpers, [slot(HelperValue::SampleMask, 0)]);
        assert!(wgsl(&c).contains("@builtin(sample_mask)"));
    }
    let key = VariantKey {
        flags: VariantFlags::COUNTING,
        ..Default::default()
    };
    for c in both(&textured, &key) {
        let text = wgsl(&c);
        assert!(text.contains("vec4<f32>(1f, 1f, 1f, "), "{text}");
        // The entry point writes location 0 alone.
        let m = &c.modules[0].module;
        let result = m.entry_points[0].function.result.as_ref().unwrap();
        let naga::TypeInner::Struct { members, .. } = &m.types[result.ty].inner else {
            panic!("{text}")
        };
        let bindings: Vec<_> = members.iter().map(|m| m.binding.clone()).collect();
        assert!(
            matches!(
                &bindings[..],
                [Some(naga::Binding::Location { location: 0, .. })]
            ),
            "{text}"
        );
    }
    let key = VariantKey {
        lod_bias: 1,
        ..Default::default()
    };
    for c in both(&textured, &key) {
        assert_eq!(
            c.reflection.helpers,
            [slot(HelperValue::LodBias { sampler: 0 }, 0)]
        );
        let text = wgsl(&c);
        assert!(text.contains("textureSampleBias("), "{text}");
    }
    let key = VariantKey {
        flags: VariantFlags::DUAL_SOURCE,
        ..Default::default()
    };
    for c in both(&own_pair("ps_dual"), &key) {
        let text = wgsl(&c);
        assert!(text.contains("@blend_src(1)"), "{text}");
    }
    assert_eq!(
        convert(Source::Dxbc(&textured[0]), &key, None).map(|_| ()),
        Err(ConvertError::Unsupported("DualSourceWithoutTwoTargets"))
    );
    let key = VariantKey {
        flags: VariantFlags::TEXTURE_INFO,
        ..Default::default()
    };
    for c in both(&own_pair("ps_levels"), &key) {
        assert_eq!(
            c.reflection.helpers,
            [slot(HelperValue::TextureLevels { texture: 0 }, 0)]
        );
        let text = wgsl(&c);
        assert!(!text.contains("textureNumLevels"), "{text}");
    }
}

/// A target whose format takes another type than the program's output gets the output's bits as the
/// format's type, as Direct3D writes them; normalised formats and combining are refused by name.
#[test]
fn targets_of_another_type_take_the_outputs_bits() {
    let target = |format, combine| VariantKey {
        targets_in_shader: vec![TargetInShader {
            target: 0,
            format,
            combine,
        }],
        ..Default::default()
    };
    let scalar = |c: &wcontainers_shaders::Converted| {
        let o = c.reflection.outputs.iter();
        o.filter(|o| o.system == Some(SystemValue::Target(0)))
            .map(|o| o.scalar)
            .collect::<Vec<_>>()
    };
    for (pair, format, cast, kind) in [
        (
            "ps_textured",
            Format::Rgba32Uint,
            "bitcast<vec4<u32>>",
            ScalarKind::Uint,
        ),
        (
            "ps_textured",
            Format::R32Sint,
            "bitcast<vec4<i32>>",
            ScalarKind::Sint,
        ),
        (
            "ps_integer",
            Format::Rgba32Float,
            "bitcast<vec4<f32>>",
            ScalarKind::Float,
        ),
    ] {
        let dxbc = read(&format!("tests/own/{pair}.dxbc"));
        let dxil = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/own/{pair}.dxil")),
        );
        let mut sources = vec![Source::Dxbc(&dxbc)];
        sources.extend(dxil.as_deref().ok().map(Source::Dxil));
        for source in sources {
            let c = convert(source, &target(format, false), None).unwrap();
            let text = wgsl(&c);
            assert!(text.contains(cast), "{pair}: {text}");
            assert_eq!(scalar(&c), [kind], "{pair}");
        }
    }
    // The same type: the program is unchanged.
    let code = own_pair("ps_textured");
    let plain = convert(Source::Dxbc(&code[0]), &VariantKey::default(), None).unwrap();
    let same = convert(
        Source::Dxbc(&code[0]),
        &target(Format::Rgba32Float, false),
        None,
    )
    .unwrap();
    assert_eq!(wgsl(&plain), wgsl(&same));
    for (key, name) in [
        (target(Format::Rgba8Unorm, false), "NormAsInteger"),
        (target(Format::Rgba32Float, true), "BlendInShader"),
    ] {
        let got = convert(Source::Dxbc(&code[0]), &key, None).map(|_| ());
        assert_eq!(got, Err(ConvertError::Unsupported(name)));
    }
    let vs = own_pair("vs_transform");
    let got = convert(
        Source::Dxbc(&vs[0]),
        &target(Format::Rgba32Uint, false),
        None,
    )
    .map(|_| ());
    assert_eq!(got, Err(ConvertError::Unsupported("TargetsInShader")));
}

/// The minimum level-of-detail clamp: samples go through the merged sampler of their pair in the helper
/// set, composed with the sampler's bias in one callee; loads and gathers are checked against the
/// clamp; comparisons are refused, since their out-of-bounds answer depends on the sampler's function.
#[test]
fn min_lod_clamp_samples_through_the_merged_sampler() {
    let key = VariantKey {
        lod_bias: 1,
        min_lod_clamped: 1,
        ..Default::default()
    };
    for c in both(&own_pair("ps_textured"), &key) {
        assert_eq!(
            c.reflection.helpers,
            [
                slot(HelperValue::LodBias { sampler: 0 }, 0),
                // Right after the bias, as std140 packs it (GLSL ES reads the block so).
                slot(HelperValue::MinLod { texture: 0 }, 8)
            ]
        );
        let text = wgsl(&c);
        assert!(text.contains("@group(3) @binding(16)"), "{text}");
        assert!(text.contains("textureSampleBias("), "{text}");
        assert!(text.contains("select("), "{text}");
    }
    let key = VariantKey {
        min_lod_clamped: 1,
        ..Default::default()
    };
    for name in [
        "dxbc/samplers.ps_gather.dxbc",
        "dxil/samplers.ps_gather.dxil",
    ] {
        let source = rig(name);
        let source = if name.ends_with("dxil") {
            Source::Dxil(&source)
        } else {
            Source::Dxbc(&source)
        };
        let c = convert(source, &key, None).unwrap();
        let text = wgsl(&c);
        assert!(text.contains("textureGather("), "{text}");
        assert!(!text.contains("@group(3) @binding(16)"), "{text}");
        assert!(text.contains("select("), "{text}");
    }
    for name in [
        "dxbc/samplers.ps_compare.dxbc",
        "dxil/samplers.ps_compare.dxil",
    ] {
        let source = rig(name);
        let source = if name.ends_with("dxil") {
            Source::Dxil(&source)
        } else {
            Source::Dxbc(&source)
        };
        assert_eq!(
            convert(source, &key, None).map(|_| ()),
            Err(ConvertError::Unsupported("MinLodOnComparison"))
        );
    }
}

/// A vertex program's clip distances become the common form's built-in array, on both routes.
#[test]
fn clip_distances_are_the_built_in_array() {
    for c in both(&own_pair("vs_clip"), &VariantKey::default()) {
        let text = wgsl(&c);
        assert!(text.contains("enable clip_distances;"), "{text}");
        assert!(
            text.contains("@builtin(clip_distances) b1_: array<f32, 2>")
                || text.contains("array<f32, 2>"),
            "{text}"
        );
        assert!(
            c.reflection
                .outputs
                .iter()
                .any(|o| o.system == Some(wcontainers_gpu::SystemValue::ClipDistance)),
            "{:?}",
            c.reflection.outputs
        );
    }
}

/// Clip distances are listed as the signature has them, one value per register with its mask, so a
/// front counting their components gets every distance, also when they span two registers; the
/// program clips by that many in the program.
#[test]
fn clip_distances_are_listed_per_signature_register() {
    use wcontainers_gpu::SystemValue;
    for (name, count, registers) in [("vs_clip", 2, 1), ("vs_clip_six", 6, 2)] {
        let code = own_pair(name);
        for c in both(&code, &VariantKey::default()) {
            let clips: Vec<_> = c
                .reflection
                .outputs
                .iter()
                .filter(|o| o.system == Some(SystemValue::ClipDistance))
                .collect();
            let components: u32 = clips.iter().map(|o| o.mask.count_ones()).sum();
            assert_eq!((components, clips.len()), (count, registers), "{clips:?}");
            assert!(
                wgsl(&c).contains(&format!("array<f32, {count}>")),
                "{}",
                wgsl(&c)
            );
        }
        let location = both(&code, &VariantKey::default())[0]
            .reflection
            .outputs
            .iter()
            .filter(|o| o.system.is_none())
            .map(|o| o.location + 1)
            .max()
            .unwrap();
        let key = VariantKey {
            flags: VariantFlags::CLIP_DISTANCE,
            linked: LinkedValues {
                location,
                clip_distances: count as u8,
            },
            ..Default::default()
        };
        for c in both(&code, &key) {
            assert!(!wgsl(&c).contains("clip_distances"), "{}", wgsl(&c));
        }
    }
}

/// Constant buffers read as storage move to the storage range, named as added.
#[test]
fn constants_read_as_storage_are_added_bindings() {
    let key = VariantKey {
        constants_as_storage: 1,
        ..Default::default()
    };
    for c in both(&own_pair("vs_transform"), &key) {
        let b = c
            .reflection
            .bindings
            .iter()
            .find(|b| b.added.is_some())
            .unwrap();
        assert_eq!(b.kind, SlotKind::Storage);
        assert_eq!(b.binding, SlotKind::Storage.first_binding());
        assert_eq!(b.added, Some(AddedBinding::ConstantsAsStorage(0)));
        assert!(wgsl(&c).contains("var<storage>"), "{}", wgsl(&c));
        // Its registers are runtime-sized, so an index past the declared array reads the buffer.
        let m = &c.modules[0].module;
        let (_, g) = m
            .global_variables
            .iter()
            .find(|(_, g)| g.binding.is_some_and(|r| r.binding == b.binding))
            .unwrap();
        let array = match &m.types[g.ty].inner {
            naga::TypeInner::Struct { members, .. } => members.last().unwrap().ty,
            _ => g.ty,
        };
        assert!(
            matches!(
                m.types[array].inner,
                naga::TypeInner::Array {
                    size: naga::ArraySize::Dynamic,
                    ..
                }
            ),
            "{}",
            wgsl(&c)
        );
    }
}

/// The targets' sample count is read only where the variant fixes it, on both routes.
#[test]
fn the_sample_count_comes_from_the_variant() {
    let code = own_pair("ps_samples");
    let key = VariantKey {
        samples: 4,
        ..Default::default()
    };
    for (source, fixed) in [
        (Source::Dxbc(&code[0]), Source::Dxbc(&code[0])),
        (Source::Dxil(&code[1]), Source::Dxil(&code[1])),
    ] {
        assert_eq!(
            convert(source, &VariantKey::default(), None).map(|_| ()),
            Err(ConvertError::Unsupported("SampleCount"))
        );
        let text = wgsl(&convert(fixed, &key, None).unwrap());
        assert!(text.contains("4u"), "{text}");
    }
}

/// Depth bias in the program writes the biased depth from its helper, on both routes, and leaves a
/// depth the program writes itself as it is.
#[test]
fn depth_bias_writes_the_biased_depth() {
    let key = VariantKey {
        flags: VariantFlags::DEPTH_BIAS,
        ..Default::default()
    };
    for c in both(&own_pair("ps_textured"), &key) {
        assert_eq!(c.reflection.helpers, [slot(HelperValue::DepthBias, 0)]);
        let text = wgsl(&c);
        assert!(text.contains("@builtin(frag_depth)"), "{text}");
        assert!(text.contains("dpdx("), "{text}");
    }
    let own = convert(Source::Dxbc(&read("tests/own/ps_depth.dxbc")), &key, None).unwrap();
    let text = wgsl(&own);
    assert_eq!(text.matches("@builtin(frag_depth)").count(), 1, "{text}");
}

/// Bases from an indirect draw's arguments: the bases' block becomes the arguments' place, and the
/// argument buffer is read in set 3, on both routes.
#[test]
fn draw_bases_from_arguments_read_the_argument_buffer() {
    let key = VariantKey {
        draw_base: DrawBaseSource::Arguments,
        ..Default::default()
    };
    for c in both(&own_pair("vs_transform"), &key) {
        assert_eq!(c.reflection.helpers, [slot(HelperValue::DrawArguments, 0)]);
        let text = wgsl(&c);
        assert!(
            text.contains("@group(3) @binding(11) \nvar<storage> draw_arguments: array<u32>"),
            "{text}"
        );
        assert!(text.contains("arguments_offset"), "{text}");
    }
}

/// The draw's arguments keep the helper block's other members: a typed view's place
/// (`TexelBufferView`) stays where it was, read by the program as before.
#[test]
fn draw_arguments_keep_the_typed_views_place() {
    for draw_base in [DrawBaseSource::Helpers, DrawBaseSource::Arguments] {
        let key = VariantKey {
            draw_base,
            texel_buffers: TexelBufferRoute::Storage,
            texel_formats: vec![(0, Format::R32Float)],
            ..Default::default()
        };
        for c in both(&own_pair("vs_texels"), &key) {
            let bases = match draw_base {
                DrawBaseSource::Arguments => HelperValue::DrawArguments,
                _ => HelperValue::DrawBase,
            };
            assert_eq!(c.reflection.helpers[0], slot(bases, 0), "{draw_base:?}");
            assert!(
                c.reflection
                    .helpers
                    .iter()
                    .any(|h| matches!(h.value, HelperValue::TexelBufferView { .. })),
                "{:?}",
                c.reflection.helpers
            );
        }
    }
}

/// Clip distances and depth clamping in the program, on both routes: the vertex program writes its
/// distances and unclamped z at the linked location instead of the built-in, and the pixel program
/// reads them there, discards (one sample) or clears samples (four), and writes the clamped depth.
#[test]
fn clip_distances_and_depth_clamp_pass_through_the_linked_location() {
    let vs = own_pair("vs_clip");
    let ps = own_pair("ps_textured");
    let plain = both(&vs, &VariantKey::default());
    let location = plain[0]
        .reflection
        .outputs
        .iter()
        .filter(|o| o.system.is_none())
        .map(|o| o.location + 1)
        .max()
        .unwrap();
    let flags = VariantFlags::CLIP_DISTANCE | VariantFlags::DEPTH_CLAMP;
    let linked = LinkedValues {
        location,
        clip_distances: 2,
    };
    let key = VariantKey {
        flags,
        linked,
        ..Default::default()
    };
    for c in both(&vs, &key) {
        let text = wgsl(&c);
        assert!(!text.contains("clip_distances"), "{text}");
        assert!(text.contains(&format!("@location({location})")), "{text}");
        assert_eq!(
            wcontainers_shaders::capabilities(&c.modules[0].module),
            wcontainers_shaders::WEBGPU_CAPABILITIES
        );
    }
    for samples in [1, 4] {
        let key = VariantKey {
            flags,
            linked,
            samples,
            ..Default::default()
        };
        for c in both(&ps, &key) {
            assert_eq!(c.reflection.helpers, [slot(HelperValue::DepthRange, 0)]);
            let text = wgsl(&c);
            assert!(text.contains(&format!("@location({location})")), "{text}");
            assert!(text.contains("discard"), "{text}");
            assert!(text.contains("@builtin(frag_depth)"), "{text}");
            assert_eq!(
                text.contains("@builtin(sample_mask)"),
                samples == 4,
                "{text}"
            );
        }
    }
    let refused = |key: VariantKey, code: &[Vec<u8>; 2], name| {
        for source in [Source::Dxbc(&code[0]), Source::Dxil(&code[1])] {
            assert_eq!(
                convert(source, &key, None).map(|_| ()),
                Err(ConvertError::Unsupported(name))
            );
        }
    };
    let key = VariantKey {
        flags: VariantFlags::CLIP_DISTANCE,
        linked,
        ..Default::default()
    };
    refused(key.clone(), &ps, "SampleCount");
    refused(
        VariantKey {
            samples: 2,
            ..key.clone()
        },
        &ps,
        "ClipDistanceSamples",
    );
    refused(
        VariantKey {
            linked: LinkedValues {
                location,
                clip_distances: 1,
            },
            ..key.clone()
        },
        &vs,
        "LinkedClipDistanceCount",
    );
    refused(
        VariantKey {
            linked: LinkedValues {
                location: 0,
                clip_distances: 2,
            },
            ..key
        },
        &vs,
        "LinkedLocationTaken",
    );
}

/// Mirror-once addressing in the program: the sampler's state from the helper block picks the axes to
/// fold, and the implicit level becomes the unfolded coordinate's gradients; gathers, comparisons and
/// texel offsets are refused by name.
#[test]
fn mirror_once_folds_the_coordinate_with_its_own_gradients() {
    let key = VariantKey {
        mirror_once_in_shader: 1,
        ..Default::default()
    };
    for c in both(&own_pair("ps_textured"), &key) {
        assert_eq!(
            c.reflection.helpers,
            [slot(HelperValue::SamplerState { sampler: 0 }, 0)]
        );
        assert_eq!(c.reflection.helper_size, 48);
        let text = wgsl(&c);
        assert!(text.contains("textureSampleGrad("), "{text}");
        assert!(text.contains("abs("), "{text}");
        assert!(text.contains("== 4u"), "{text}");
    }
    let every = VariantKey {
        mirror_once_in_shader: !0,
        ..Default::default()
    };
    for (name, reason) in [
        ("samplers.ps_gather", "MirrorOnceOnGather"),
        ("samplers.ps_compare", "MirrorOnceOnComparison"),
    ] {
        for source in [
            Source::Dxbc(&rig(&format!("dxbc/{name}.dxbc"))),
            Source::Dxil(&rig(&format!("dxil/{name}.dxil"))),
        ] {
            assert_eq!(
                convert(source, &every, None).map(|_| ()),
                Err(ConvertError::Unsupported(reason))
            );
        }
    }
}

/// Pulled vertex inputs leave the entry point, each read from its own added storage binding by the
/// helper block's offset, stride, divisor and bias, on both routes.
#[test]
fn pulled_inputs_are_added_bindings_read_by_the_helpers() {
    let key = VariantKey {
        pulled: vec![(0, VertexFormat::Float32x3), (1, VertexFormat::Unorm16x2)],
        ..Default::default()
    };
    for c in both(&own_pair("vs_transform"), &key) {
        let added: Vec<_> = c
            .reflection
            .bindings
            .iter()
            .filter_map(|b| b.added)
            .collect();
        assert_eq!(
            added,
            [AddedBinding::PulledStream(0), AddedBinding::PulledStream(1)]
        );
        assert_eq!(
            c.reflection.helpers,
            [
                slot(HelperValue::PulledStream { input: 0 }, 0),
                slot(HelperValue::PulledStream { input: 1 }, 16)
            ]
        );
        let text = wgsl(&c);
        let main = &text[text.find("fn main(").unwrap()..];
        let inputs = &main[..main.find("->").unwrap()];
        assert!(!inputs.contains("@location"), "{text}");
        assert!(inputs.contains("@builtin(instance_index)"), "{text}");
        assert!(text.contains("arrayLength((&pulled_1_))"), "{text}");
    }
}

/// Constant buffers read through a texture: the buffer's binding becomes an added texture binding,
/// read by texel fetches, on both routes, for a fixed row (`vs_transform`) and a row picked at run
/// time (`ps_rows`).
#[test]
fn constants_read_through_a_texture_are_texel_fetches() {
    let key = VariantKey {
        constants_as_texture: 1,
        ..Default::default()
    };
    for name in ["vs_transform", "ps_rows"] {
        let code = [
            read(&format!("tests/own/{name}.dxbc")),
            read(&format!("tests/own/{name}.dxil")),
        ];
        for c in both(&code, &key) {
            let b = c
                .reflection
                .bindings
                .iter()
                .find(|b| b.added.is_some())
                .unwrap();
            assert_eq!(b.kind, SlotKind::Texture);
            assert_eq!(b.added, Some(AddedBinding::ConstantsAsTexture(0)));
            let text = wgsl(&c);
            assert!(text.contains("texture_2d<u32>"), "{text}");
            assert!(text.contains("textureLoad"), "{text}");
            assert!(!text.contains("var<uniform>"), "{text}");
        }
    }
}

/// Filtering in the program: every sample of a filtered texture, or through a sampler whose border the
/// program applies, becomes a call into the filter library reading texels with loads, by the sampler's
/// state and border colour from the helper block; the pair leaves the reflection. Every shape of texture
/// the library serves converts on both routes; comparisons and cubes are refused by name.
#[test]
fn filtering_in_the_program_reads_texels_with_loads() {
    for key in [
        VariantKey {
            filter_in_shader: 1,
            ..Default::default()
        },
        VariantKey {
            border_in_shader: 1,
            lod_bias: 1,
            flags: VariantFlags::TEXTURE_INFO,
            ..Default::default()
        },
    ] {
        for c in both(&own_pair("ps_textured"), &key) {
            assert!(c.reflection.sampled_pairs.is_empty());
            let values: Vec<_> = c.reflection.helpers.iter().map(|s| s.value).collect();
            assert!(values.contains(&HelperValue::SamplerState { sampler: 0 }));
            assert!(values.contains(&HelperValue::BorderColor { sampler: 0 }));
            let text = wgsl(&c);
            assert!(!text.contains("textureSample"), "{text}");
            assert!(text.contains("textureLoad("), "{text}");
            // GLSL ES has no level count, which the WebGL 2 back end's fronts read from the helpers.
            if key.flags.contains(VariantFlags::TEXTURE_INFO) {
                assert!(!text.contains("textureNumLevels"), "{text}");
                glsl(&c).unwrap_or_else(|e| panic!("{key:?}: {e}\n{text}"));
            }
        }
    }
    let every = VariantKey {
        filter_in_shader: !0,
        flags: VariantFlags::TEXTURE_INFO,
        ..Default::default()
    };
    for name in [
        "samplers.ps_gather",
        "samplers.ps_sample",
        "textures.ps_1d",
        "textures.ps_2d_array",
        "textures.ps_3d",
        "textures.ps_mips",
    ] {
        let c = convert(
            Source::Dxbc(&rig(&format!("dxbc/{name}.dxbc"))),
            &every,
            None,
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!wgsl(&c).contains("textureSample"), "{name}");
        if name != "textures.ps_1d" {
            glsl(&c).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        assert!(!wgsl(&c).contains("textureGather"), "{name}");
    }
    // Comparisons take depth textures, which back ends compare and filter natively; the plain reads
    // of a depth texture are filtered from its loads.
    let c = convert(
        Source::Dxbc(&rig("dxbc/samplers.ps_compare.dxbc")),
        &every,
        None,
    )
    .unwrap();
    assert!(wgsl(&c).contains("textureSampleCompare"), "{}", wgsl(&c));
    let shadow = own_pair("ps_shadow");
    for c in both(&shadow, &every) {
        let text = wgsl(&c);
        assert!(text.contains("textureSampleCompare"), "{text}");
        assert!(text.contains("filter_sample"), "{text}");
        assert!(!text.contains("textureSample("), "{text}");
        assert!(!text.contains("textureGather("), "{text}");
    }
    assert_eq!(
        convert(
            Source::Dxbc(&rig("dxbc/textures.ps_cube.dxbc")),
            &every,
            None
        )
        .map(|_| ()),
        Err(ConvertError::Unsupported("FilterInShaderOnCube"))
    );
}

/// A four-channel float storage texture both read and written (shader model 6's half writes) has
/// no read-write storage form in WebGPU: plain, it is refused by name; through a buffer, it is
/// read and written there.
#[test]
fn read_written_four_channel_storage_goes_through_a_buffer() {
    let code = read("tests/own/ps_halves.dxil");
    let mut key = VariantKey {
        storage_formats: vec![(0, wcontainers_gpu::Format::Rgba32Float)],
        ..VariantKey::default()
    };
    let refused = convert(Source::Dxil(&code), &key, None).map(|c| wgsl(&c));
    assert!(
        matches!(
            refused,
            Err(ConvertError::Unsupported("ReadWriteStorageFormat"))
        ),
        "{refused:?}"
    );
    key.texels_through_buffer = 1;
    let text = wgsl(&convert(Source::Dxil(&code), &key, None).unwrap());
    assert!(!text.contains("texture_storage"), "{text}");
}

/// An array of textures is one texture per element, at the registers Direct3D 12 gives the range,
/// and an element picked at run time is chosen by a switch.
#[test]
fn texture_arrays_are_a_texture_per_element() {
    let c = convert(
        Source::Dxil(&read("tests/own/ps_array.dxil")),
        &VariantKey::default(),
        None,
    )
    .unwrap();
    let mut textures: Vec<_> = c
        .reflection
        .bindings
        .iter()
        .filter(|b| b.kind == SlotKind::Texture)
        .map(|b| (b.register, b.slot()))
        .collect();
    textures.sort_unstable();
    assert_eq!(textures, [(0, 0), (1, 1), (2, 2), (3, 3)]);
    let text = wgsl(&c);
    assert!(text.contains("switch"), "{text}");
    assert_eq!(text.matches("texture_2d<f32>;").count(), 4, "{text}");
}

/// A texture both compared against and read plainly (`ps_shadow`: compared samples, a sample, a
/// sample at level 0, a load and a gather) is one depth texture by both routes, its plain reads
/// widened to (depth, 0, 0, 1), with no other texture declared.
#[test]
fn textures_compared_and_read_plainly_are_depth_textures() {
    let code = own_pair("ps_shadow");
    for (route, c) in ["dxbc", "dxil"]
        .into_iter()
        .zip(both(&code, &VariantKey::default()))
    {
        let text = wgsl(&c);
        assert_eq!(
            text.matches("texture_depth_2d;").count(),
            1,
            "{route}: {text}"
        );
        assert!(!text.contains("texture_2d<"), "{route}: {text}");
        assert!(text.contains("textureSampleCompare("), "{route}: {text}");
        assert!(text.contains("textureLoad("), "{route}: {text}");
        assert!(text.contains("textureGather("), "{route}: {text}");
    }
}

/// Shader model 4.0 loads of a multisampled texture (`ps_msloop`: the sample picked in an unrolled
/// loop, at `pos.xy`; `ps_msswap`: at `pos.yx`) read at the coordinates each names.
#[test]
fn shader_model_4_multisampled_loads_keep_their_coordinates() {
    let code = |name: &str| read(&format!("tests/own/{name}.dxbc"));
    let [straight, swapped] = ["ps_msloop", "ps_msswap"].map(|name| {
        let c = convert(Source::Dxbc(&code(name)), &VariantKey::default(), None).unwrap();
        let text = wgsl(&c);
        eprintln!("{name}:\n{text}");
        text
    });
    assert_ne!(straight, swapped);
}

/// A multisampled array texture is refused by name until the key gives its layer count; then each
/// layer after the first is an added texture binding, and loads switch on the layer, by both routes.
#[test]
fn multisampled_array_layers_are_a_texture_each() {
    let code = own_pair("ps_msarray");
    for (route, source) in [
        ("dxbc", Source::Dxbc(&code[0])),
        ("dxil", Source::Dxil(&code[1])),
    ] {
        assert_eq!(
            convert(source, &VariantKey::default(), None).unwrap_err(),
            ConvertError::Unsupported("MultisampledArrayTexture"),
            "{route}"
        );
        let key = VariantKey {
            texture_layers: vec![(0, 3)],
            ..VariantKey::default()
        };
        let c = convert(source, &key, None).unwrap();
        let mut textures: Vec<_> = c
            .reflection
            .bindings
            .iter()
            .filter(|b| b.kind == SlotKind::Texture)
            .map(|b| (b.binding, b.added))
            .collect();
        textures.sort_unstable_by_key(|t| t.0);
        let first = SlotKind::Texture.first_binding();
        assert_eq!(
            textures,
            [
                (first, None),
                (first + 1, Some(AddedBinding::TextureLayer(0, 1))),
                (first + 2, Some(AddedBinding::TextureLayer(0, 2))),
            ],
            "{route}"
        );
        let text = wgsl(&c);
        assert_eq!(
            text.matches("texture_multisampled_2d<f32>;").count(),
            3,
            "{route}: {text}"
        );
        assert!(text.contains("switch"), "{route}: {text}");
    }
}

/// A view's component mapping: reads of the slot are composed from the mapping, by both routes; a
/// gather of a component gathers the one it maps to, or is the constant it maps to.
#[test]
fn swizzled_slots_compose_what_they_read() {
    use wcontainers_gpu::ViewComponent as C;
    for (name, map) in [
        ("ps_textured", [C::B, C::One, C::Zero, C::R]),
        ("ps_mingather", [C::G, C::R, C::Zero, C::A]),
        ("ps_mingather", [C::One, C::One, C::One, C::One]),
    ] {
        let code = own_pair(name);
        let key = VariantKey {
            swizzled: vec![(0, map)],
            ..VariantKey::default()
        };
        let plain = both(&code, &VariantKey::default());
        for (c, plain) in both(&code, &key).iter().zip(&plain) {
            assert_ne!(wgsl(c), wgsl(plain), "{name} {map:?}");
        }
    }
}

/// A mapped depth view read whole takes the mapping in the (depth, 0, 0, 1) it reads, by both routes;
/// comparisons read the first component, so a mapping moving red away from it is refused by name.
#[test]
fn mapped_depth_views_compose_what_they_read() {
    use wcontainers_gpu::ViewComponent as C;
    let key = |map| VariantKey {
        swizzled: vec![(0, map)],
        ..VariantKey::default()
    };
    let code = own_pair("ps_shadow_wide");
    let plain = both(&code, &VariantKey::default());
    for (c, plain) in both(&code, &key([C::R, C::A, C::One, C::G]))
        .iter()
        .zip(&plain)
    {
        assert_ne!(wgsl(c), wgsl(plain));
    }
    both(&own_pair("ps_shadow"), &key([C::R, C::R, C::R, C::One]));
    let moved = [C::Zero, C::R, C::A, C::G];
    for name in ["ps_shadow", "ps_shadow_wide"] {
        let code = own_pair(name);
        for (route, source) in [Source::Dxbc(&code[0]), Source::Dxil(&code[1])]
            .into_iter()
            .enumerate()
        {
            match convert(source, &key(moved), None) {
                Ok(c) => panic!("{name} route {route} converted:\n{}", wgsl(&c)),
                Err(e) => assert_eq!(
                    e,
                    ConvertError::Unsupported(wcontainers_shaders::SWIZZLED_DEPTH)
                ),
            }
        }
    }
}

/// Wave minimums and maximums of signed values the converted DXIL holds as unsigned words read them
/// as signed and give the result back as the words.
#[test]
fn wave_minimums_of_the_other_signedness_read_their_operands_as_named() {
    let code = read("tests/own/ps_wave_signed.dxil");
    let c = convert(Source::Dxil(&code), &VariantKey::default(), None).unwrap();
    let text = wgsl(&c);
    for op in ["subgroupMin(bitcast<i32>(", "subgroupMax(bitcast<i32>("] {
        assert!(text.contains(op), "{op}: {text}");
    }
}

/// Atomic minimums and maximums of the other signedness than their memory's other operations are
/// compare-exchange loops, reported as the program's own slow path, by both routes.
#[test]
fn atomics_of_the_other_signedness_are_exchange_loops() {
    for c in both(&own_pair("cs_atomics"), &VariantKey::default()) {
        assert_eq!(
            c.reflection.slow_paths,
            [wcontainers_gpu::SlowPath::AtomicExchangeLoop]
        );
        assert!(wgsl(&c).contains("atomicCompareExchangeWeak"));
    }
    for name in ["ps_textured", "cs_shared"] {
        for c in both(&own_pair(name), &VariantKey::default()) {
            assert!(c.reflection.slow_paths.is_empty(), "{name}");
        }
    }
}

/// Raw and structured shader resource views are storage buffers at texture slots: by both routes
/// they give their element's size, and reading views from an offset gives each of them, and the
/// storage view, a helper value.
#[test]
fn shader_resource_buffers_give_their_stride_and_offsets() {
    let code = own_pair("cs_readonly");
    for c in both(&code, &VariantKey::default()) {
        let read: Vec<_> = c
            .reflection
            .bindings
            .iter()
            .filter(|b| b.kind == SlotKind::Texture)
            .collect();
        assert_eq!(read.len(), 2, "{:?}", c.reflection.bindings);
        let word = core::mem::size_of::<u32>() as u32;
        assert!(read.iter().all(|b| b.stride == word), "{read:?}");
    }
    let key = VariantKey {
        flags: VariantFlags::STORAGE_OFFSET,
        ..Default::default()
    };
    for c in both(&code, &key) {
        let mut offsets: Vec<_> = c
            .reflection
            .helpers
            .iter()
            .filter_map(|h| match h.value {
                HelperValue::StorageOffset { set, binding } => Some((set, binding)),
                _ => None,
            })
            .collect();
        offsets.sort();
        let mut moved: Vec<_> = c
            .reflection
            .bindings
            .iter()
            .filter(|b| b.stride != 0)
            .map(|b| (b.set, b.binding))
            .collect();
        moved.sort();
        assert_eq!(offsets.len(), 3, "{:?}", c.reflection.helpers);
        assert_eq!(offsets, moved, "{:?}", c.reflection.bindings);
    }
}

/// Every storage view (not the counters) gives its element's size, by both routes; reading views from
/// an offset (`STORAGE_OFFSET`) gives each of them a helper value and a valid module.
#[test]
fn storage_bindings_give_their_stride_and_offsets() {
    let code = own_pair("cs_atomics");
    for c in both(&code, &VariantKey::default()) {
        let storage: Vec<_> = c
            .reflection
            .bindings
            .iter()
            .filter(|b| b.kind == SlotKind::Storage)
            .collect();
        assert_eq!(storage.len(), 5, "{:?}", c.reflection.bindings);
        assert!(storage.iter().all(|b| b.stride == 4), "{storage:?}");
    }
    let key = VariantKey {
        flags: VariantFlags::STORAGE_OFFSET,
        ..Default::default()
    };
    for c in both(&code, &key) {
        let offsets: Vec<_> = c
            .reflection
            .helpers
            .iter()
            .filter_map(|h| match h.value {
                HelperValue::StorageOffset { set, binding } => Some((set, binding)),
                _ => None,
            })
            .collect();
        let mut storage: Vec<_> = c
            .reflection
            .bindings
            .iter()
            .filter(|b| b.kind == SlotKind::Storage && b.stride != 0)
            .map(|b| (b.set, b.binding))
            .collect();
        storage.sort();
        let mut sorted = offsets.clone();
        sorted.sort();
        assert_eq!(sorted, storage, "{:?}", c.reflection.helpers);
    }
}
