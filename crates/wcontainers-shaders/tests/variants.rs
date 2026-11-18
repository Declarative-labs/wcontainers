//! What `convert` refuses and what a variant changes.

use std::path::Path;
use wcontainers_gpu::{
    AddedBinding, DrawBaseSource, HelperSlot, HelperValue, Interpolation, LinkedValues, SlotKind,
    VariantFlags, VariantKey, VertexFormat,
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
        entry_point: "main".into(),
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
                flags: VariantFlags::EARLY_TESTS | VariantFlags::CLIP_DEPTH_REWRITE,
                ..Default::default()
            },
            "EarlyTestsInShader",
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
    for (name, reason) in [
        ("samplers.ps_compare", "FilterInShaderOnComparison"),
        ("textures.ps_cube", "FilterInShaderOnCube"),
    ] {
        assert_eq!(
            convert(
                Source::Dxbc(&rig(&format!("dxbc/{name}.dxbc"))),
                &every,
                None
            )
            .map(|_| ()),
            Err(ConvertError::Unsupported(reason))
        );
    }
}
