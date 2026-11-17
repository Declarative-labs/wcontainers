//! Root signatures and the feature answers of plan section 9, on the recording back end's
//! WebGPU-like capabilities.

// Only the recorder's capabilities are used here.
#[allow(dead_code)]
#[path = "../../wcontainers-directx-graphics-infrastructure/tests/support/record.rs"]
mod record;

use wcontainers_direct3d_12::features::*;
use wcontainers_direct3d_12::root::*;
use wcontainers_gpu::{Features, Format, FormatUse};

fn signature(version: u32) -> RootSignatureDesc {
    RootSignatureDesc {
        version,
        parameters: vec![
            RootParameter {
                kind: RootParameterKind::Constants {
                    register: 0,
                    space: 0,
                    count: 4,
                },
                visibility: 0,
            },
            RootParameter {
                kind: RootParameterKind::Descriptor {
                    kind: 3,
                    register: 1,
                    space: 2,
                    flags: if version == 1 { 0 } else { 4 },
                },
                visibility: 5,
            },
            RootParameter {
                kind: RootParameterKind::Table(vec![
                    DescriptorRange {
                        kind: 2,
                        count: 1,
                        register: 1,
                        space: 0,
                        flags: if version == 1 { 0 } else { 1 },
                        offset: 0,
                    },
                    DescriptorRange {
                        kind: 0,
                        count: u32::MAX,
                        register: 0,
                        space: 0,
                        flags: 0,
                        offset: u32::MAX,
                    },
                ]),
                visibility: 0,
            },
        ],
        samplers: vec![StaticSampler([
            0x15,
            3,
            3,
            3,
            0,
            1,
            1,
            0,
            0,
            f32::MAX.to_bits(),
            0,
            0,
            5,
        ])],
        flags: 1,
    }
}

#[test]
fn root_signatures_round_trip_in_both_versions() {
    for version in [1, 2] {
        let desc = signature(version);
        let blob = serialize(&desc);
        assert_eq!(&blob[..4], b"DXBC");
        assert_eq!(
            u32::from_le_bytes(blob[24..28].try_into().unwrap()) as usize,
            blob.len()
        );
        assert_eq!(deserialize(&blob).unwrap(), desc);
    }
}

#[test]
fn an_empty_version_1_0_signature_round_trips() {
    let desc = RootSignatureDesc {
        version: 1,
        parameters: vec![],
        samplers: vec![],
        flags: 1,
    };
    assert_eq!(deserialize(&serialize(&desc)).unwrap(), desc);
}

#[test]
fn malformed_root_signatures_are_refused() {
    let blob = serialize(&signature(2));
    assert!(deserialize(&blob[..blob.len() - 4]).is_err());
    assert!(deserialize(b"not a container").is_err());
    let mut bad = blob.clone();
    bad[44] = 9;
    assert_eq!(deserialize(&bad), Err("unknown root signature version"));
}

#[test]
fn root_signatures_binding_a_register_twice_for_a_stage_are_refused() {
    let range = |kind, count, register, space, offset| DescriptorRange {
        kind,
        count,
        register,
        space,
        flags: 0,
        offset,
    };
    let table = |ranges: Vec<DescriptorRange>, visibility| RootParameter {
        kind: RootParameterKind::Table(ranges),
        visibility,
    };
    let constants = |register, visibility| RootParameter {
        kind: RootParameterKind::Constants {
            register,
            space: 0,
            count: 4,
        },
        visibility,
    };
    let check = |parameters: Vec<RootParameter>| {
        RootSignatureDesc {
            version: 1,
            parameters,
            samplers: Vec::new(),
            flags: 0,
        }
        .validate()
        .is_ok()
    };
    // SRV t0-t7 and t8-t9 at overlapping table offsets: allowed; t7 twice: refused.
    assert!(check(vec![table(
        vec![range(0, 8, 0, 0, 0), range(0, 2, 8, 0, 7)],
        0
    )]));
    assert!(!check(vec![table(
        vec![range(0, 8, 0, 0, 0), range(0, 2, 7, 0, 8)],
        0
    )]));
    // The same registers as SRV and CBV, or in another space: allowed.
    assert!(check(vec![table(
        vec![range(0, 8, 0, 0, 0), range(2, 8, 0, 0, 8)],
        0
    )]));
    assert!(check(vec![table(
        vec![range(0, u32::MAX, 0, 0, 0), range(0, 1, 16, 1, 16)],
        0
    )]));
    // An unbounded range followed by an appended one, an empty range, a range past the last
    // register, samplers beside views: refused.
    assert!(!check(vec![table(
        vec![range(0, u32::MAX, 0, 0, 0), range(0, 1, 16, 1, u32::MAX)],
        0
    )]));
    assert!(!check(vec![table(vec![range(0, 0, 0, 0, 0)], 0)]));
    assert!(!check(vec![table(
        vec![range(0, 0x1000, 0xffff_f001, 0, 0)],
        0
    )]));
    assert!(!check(vec![table(
        vec![range(0, 1, 0, 0, 0), range(3, 1, 2, 0, 10)],
        0
    )]));
    // Root constants at b0 seen by every stage and by the vertex stage: refused; by the pixel and
    // vertex stages: allowed.
    assert!(!check(vec![constants(0, 0), constants(0, 1)]));
    assert!(check(vec![constants(0, 5), constants(0, 1)]));
}

#[test]
fn webgpu_reports_11_0_and_tier_1() {
    let caps = record::webgpu_like_caps();
    assert_eq!(feature_level(&caps), Some(FEATURE_LEVEL_11_0));
    let o = options(&caps);
    assert_eq!(o.resource_binding_tier, 1);
    assert_eq!(o.resource_heap_tier, 2);
    assert_eq!(o.tiled_resources_tier, 0);
    assert_eq!(o.conservative_rasterization_tier, 0);
    assert!(!o.rovs_supported && !o.standard_swizzle_64kb_supported);
    assert!(!o.double_precision_float_shader_ops && !o.output_merger_logic_op);
    let o1 = options1(&caps);
    assert!(!o1.wave_ops && !o1.int64_shader_ops);
    assert_eq!(shader_model(0x66), Ok(SHADER_MODEL_6_0));
    assert_eq!(shader_model(0x51), Ok(0x51));
    for named_nothing in [0, 0x52, 0x89] {
        assert!(shader_model(named_nothing).is_err());
    }
    assert!(!depth_bounds(&caps));
    assert_eq!(ABSENT_TIER, 0);
    // R8G8B8A8_UNORM, then A8_UNORM, which the recorder lacks.
    assert_eq!(quality_levels(&caps, 28, 4), Some(1));
    assert_eq!(quality_levels(&caps, 28, 8), Some(0));
    assert_eq!(quality_levels(&caps, 65, 1), Some(1));
    assert_eq!(quality_levels(&caps, 65, 4), Some(0));
    assert_eq!(quality_levels(&caps, 28, 0), None);
    assert_eq!(quality_levels(&caps, 66, 1), None);
    assert_eq!(
        max_feature_level(&caps, &[0x9100, 0xa000, 0xb000, 0xc000]),
        Some(FEATURE_LEVEL_11_0)
    );
    assert_eq!(max_feature_level(&caps, &[0xa000, 0x9300]), Some(0xa000));
    assert_eq!(max_feature_level(&caps, &[0xc200]), None);
    assert_eq!(
        (root_signature(1), root_signature(2), root_signature(3)),
        (1, 2, ROOT_SIGNATURE_1_1)
    );
    assert_eq!(
        options(&caps).max_gpu_virtual_address_bits_per_resource,
        VIRTUAL_ADDRESS_BITS
    );
    assert!(!Architecture::default().uma);
}

#[test]
fn resource_tables_give_tier_3_but_not_11_1() {
    let mut caps = record::webgpu_like_caps();
    caps.features |= Features::RESOURCE_TABLES | Features::SUBGROUPS | Features::LOGIC_OP;
    assert_eq!(feature_level(&caps), Some(FEATURE_LEVEL_11_0));
    assert_eq!(options(&caps).resource_binding_tier, 3);
    assert!(options1(&caps).wave_ops);
}

#[test]
fn no_device_without_compute_11_0_sizes_or_d32_s8_targets() {
    let mut caps = record::webgpu_like_caps();
    caps.features.remove(Features::COMPUTE);
    assert_eq!(feature_level(&caps), None);
    let mut caps = record::webgpu_like_caps();
    caps.limits.max_texture_layers = 256;
    assert_eq!(feature_level(&caps), None);
    let mut caps = record::webgpu_like_caps();
    caps.formats.uses[Format::Depth32FloatStencil8.index()] = FormatUse::empty();
    assert_eq!(feature_level(&caps), None);
}

#[test]
fn signatures_convert_between_versions_as_the_deserializer_does() {
    let old = signature(1);
    let new = old.at_version(2);
    assert_eq!(new.version, 2);
    let RootParameterKind::Descriptor { flags, .. } = new.parameters[1].kind else {
        unreachable!()
    };
    // Root descriptors of 1.0 are data volatile; ranges are descriptors and data volatile.
    assert_eq!(flags, 2);
    let RootParameterKind::Table(ranges) = &new.parameters[2].kind else {
        unreachable!()
    };
    assert!(ranges.iter().all(|r| r.flags == 3));
    let back = new.at_version(1);
    assert_eq!(back, old);
    // A signature at its own version is itself.
    assert_eq!(signature(2).at_version(2), signature(2));
}
