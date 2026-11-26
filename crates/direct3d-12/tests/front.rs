//! Root signatures and the feature answers of plan section 9, on the recording back end's
//! WebGPU-like capabilities.

// Only the recorder's capabilities are used here.
#[allow(dead_code)]
#[path = "../../directx-graphics-infrastructure/tests/support/record.rs"]
mod record;

use wcontainers_direct3d_12::features::*;
use wcontainers_direct3d_12::root::*;
use wcontainers_directx_graphics_infrastructure::stage_registers::{INPUT_SLOTS, OUTPUT_REGISTERS};
use wcontainers_gpu::{caps::Features, caps::FormatUse, format::Format};

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

/// The checksum Microsoft's compiler wrote into its containers (the shaders crate's fxc output,
/// with last blocks of 4 to 60 bytes, both of DXBC's paddings).
#[test]
fn dxbc_checksums_are_the_ones_fxc_writes() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../shaders/tests/own");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "dxbc") {
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(
                wcontainers_shaders::container::checksum(&bytes),
                bytes[4..20],
                "{}",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(checked >= 5, "{checked} containers");
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
    // Root arguments of 64 dwords: allowed; of 65: refused (tables 1 each, constants 4 each).
    let tables = |n: u32| (0..n).map(|i| table(vec![range(0, 1, i, 0, 0)], 0));
    assert!(check(tables(64).collect()));
    assert!(!check(tables(65).collect()));
    assert!(check(tables(60).chain([constants(0, 0)]).collect()));
    assert!(!check(tables(61).chain([constants(0, 0)]).collect()));
}

/// The recorder's capabilities with 11_0's counts: input slots and stage registers enough for
/// Direct3D's 32, half the slots laid out and half fetched in the program and a location for
/// every register but the position, and resource binding tier 1's counts with the helper block,
/// as wgpu gives (and more) on the RTX 3080 through Vulkan.
fn with_11_0_limits() -> wcontainers_gpu::caps::Caps {
    let mut caps = record::webgpu_like_caps();
    let l = &mut caps.limits;
    (l.vertex_streams, l.storage_buffers_per_stage) = (INPUT_SLOTS / 2, INPUT_SLOTS / 2);
    l.vertex_attributes = INPUT_SLOTS;
    l.inter_stage_slots = OUTPUT_REGISTERS - 1;
    l.textures_per_stage = TIER_1_SHADER_RESOURCES;
    l.uniform_buffers_per_stage = TIER_1_CONSTANT_BUFFERS + 1;
    caps
}

#[test]
fn no_device_without_32_input_slots_and_stage_registers() {
    // WebGPU's limits in Chrome on a desktop GPU fetch too few streams and pass too few values.
    let mut chrome = record::webgpu_like_caps();
    assert!(!serves_stage_registers(&chrome.limits));
    chrome.limits.inter_stage_slots = OUTPUT_REGISTERS - 1;
    assert!(!serves_stage_registers(&chrome.limits));
    assert!(has_abilities_of_11_0(&with_11_0_limits()));
    let mut caps = with_11_0_limits();
    caps.limits.inter_stage_slots -= 1;
    assert!(!has_abilities_of_11_0(&caps));
    let mut caps = with_11_0_limits();
    caps.limits.vertex_streams -= 1;
    assert!(!has_abilities_of_11_0(&caps));
}

#[test]
fn no_device_without_binding_tier_1_counts_or_eight_targets() {
    let shorts: [fn(&mut wcontainers_gpu::caps::Limits); 6] = [
        |l| l.textures_per_stage -= 1,
        // The helper block takes one of the stage's uniform buffers.
        |l| l.uniform_buffers_per_stage = TIER_1_CONSTANT_BUFFERS,
        |l| l.samplers_per_stage -= 1,
        |l| l.storage_textures_per_stage = TIER_1_UNORDERED_ACCESS_AT_11_0 - 1,
        |l| l.storage_buffers_per_stage = TIER_1_UNORDERED_ACCESS_AT_11_0 - 1,
        |l| l.color_targets -= 1,
    ];
    for (i, short) in shorts.into_iter().enumerate() {
        let mut caps = with_11_0_limits();
        short(&mut caps.limits);
        assert!(!has_abilities_of_11_0(&caps), "{i}");
    }
}

#[test]
fn webgpu_reports_11_0_and_tier_1() {
    let caps = with_11_0_limits();
    assert!(has_abilities_of_11_0(&caps));
    // The recorder holds a few formats only, short of Microsoft's 11_0 table: no device.
    assert!(!unmet_format_requirements(&caps).is_empty());
    assert_eq!(feature_level(&caps), None);
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
    assert!(shader_model(0x67).is_err());
    assert_eq!(shader_model(0x51), Ok(0x51));
    for named_nothing in [0, 0x52, 0x89] {
        assert!(shader_model(named_nothing).is_err());
    }
    // Absent for Direct3D 12 whatever the back end has (plan section 9); the front refuses them.
    let mut wide = caps.clone();
    wide.features |= Features::SHADER_I64 | Features::DEPTH_BOUNDS;
    assert!(!options1(&wide).int64_shader_ops);
    assert_eq!(ABSENT_TIER, 0);
    // R8G8B8A8_UNORM, then A8_UNORM, which the recorder lacks.
    assert_eq!(quality_levels(&caps, 28, 4), Some(1));
    assert_eq!(quality_levels(&caps, 28, 8), Some(0));
    assert_eq!(quality_levels(&caps, 65, 1), Some(1));
    assert_eq!(quality_levels(&caps, 65, 4), Some(0));
    assert_eq!(quality_levels(&caps, 28, 0), None);
    assert_eq!(quality_levels(&caps, 66, 1), None);
    use wcontainers_directx_graphics_infrastructure::required::level::L10_0;
    let own = FEATURE_LEVEL_11_0;
    assert_eq!(
        highest_of(own, &[0x9100, L10_0, FEATURE_LEVEL_11_0, 0xc000]),
        Some(FEATURE_LEVEL_11_0)
    );
    assert_eq!(highest_of(own, &[L10_0, 0x9300]), Some(L10_0));
    assert_eq!(highest_of(own, &[0xc200]), None);
    assert_eq!(max_feature_level(&caps, &[L10_0]), None);
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

/// The shared Direct3D 12 constants are the SDK's.
#[cfg(windows)]
#[test]
fn shared_constants_are_the_sdks() {
    use wcontainers_directx_graphics_infrastructure::d3d;
    use windows::Win32::Graphics::Direct3D12 as sdk;
    assert_eq!(
        d3d::TEXTURE_DATA_PLACEMENT_ALIGNMENT,
        u64::from(sdk::D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT)
    );
    assert_eq!(
        d3d::SMALL_RESOURCE_PLACEMENT_ALIGNMENT,
        u64::from(sdk::D3D12_SMALL_RESOURCE_PLACEMENT_ALIGNMENT)
    );
    assert_eq!(
        d3d::DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT,
        u64::from(sdk::D3D12_DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT)
    );
    assert_eq!(
        d3d::DEFAULT_MSAA_RESOURCE_PLACEMENT_ALIGNMENT,
        u64::from(sdk::D3D12_DEFAULT_MSAA_RESOURCE_PLACEMENT_ALIGNMENT)
    );
}

#[test]
fn atomics_on_typed_storage_views_are_answered_through_a_buffer() {
    use wcontainers_directx_graphics_infrastructure::{atomic_operations, dxgi, support2};
    // The recorder stores into R32_UINT but has no atomics on storage textures.
    let caps = record::webgpu_like_caps();
    assert!(!caps.formats.uses[Format::R32Uint as usize].contains(FormatUse::STORAGE_ATOMIC));
    let uint = format_support(&caps, dxgi::R32_UINT).support2;
    assert_eq!(
        uint & atomic_operations(dxgi::R32_UINT),
        atomic_operations(dxgi::R32_UINT)
    );
    assert_ne!(atomic_operations(dxgi::R32_UINT), 0);
    assert_eq!(
        atomic_operations(dxgi::R32_FLOAT),
        support2::UAV_ATOMIC_EXCHANGE
    );
    // The recorder copies R32_FLOAT out but does not store into it: its storage views go through
    // a buffer (`StorageFormatThroughBuffer`), with its atomics.
    assert_eq!(
        format_support(&caps, dxgi::R32_FLOAT).support2,
        support2::UAV_TYPED_STORE | support2::UAV_TYPED_LOAD | support2::UAV_ATOMIC_EXCHANGE
    );
    // No atomics where no storage view is answered: a format neither stored nor copied out.
    let mut held = caps.clone();
    held.formats.uses[Format::R32Float as usize].remove(FormatUse::COPY_OUT);
    assert_eq!(format_support(&held, dxgi::R32_FLOAT).support2, 0);
    let mut stored = caps.clone();
    stored.formats.uses[Format::R32Float as usize] |= FormatUse::STORAGE_WRITE;
    assert_eq!(
        format_support(&stored, dxgi::R32_FLOAT).support2 & support2::UAV_ATOMIC_EXCHANGE,
        support2::UAV_ATOMIC_EXCHANGE
    );
    assert_eq!(atomic_operations(dxgi::R8G8B8A8_UINT), 0);
}

#[test]
fn resource_tables_give_tier_3_but_not_11_1() {
    let mut caps = with_11_0_limits();
    caps.features |= Features::RESOURCE_TABLES | Features::SUBGROUPS | Features::LOGIC_OP;
    assert!(has_abilities_of_11_0(&caps));
    assert_eq!(options(&caps).resource_binding_tier, 3);
    assert!(options1(&caps).wave_ops);
}

#[test]
fn no_device_without_the_raster_and_blend_abilities_of_11_0() {
    // A depth bias clamp, independent blending and a partial sample mask are each 11_0's, and
    // served only by the back end.
    for ability in LEVEL_11_0_FIXED_FUNCTION.iter() {
        let mut caps = with_11_0_limits();
        caps.features.remove(ability);
        assert!(!has_abilities_of_11_0(&caps), "{ability:?}");
    }
}

#[test]
fn browsers_without_polygon_mode_or_depth_clamping_get_11_0() {
    // Section 9: WebGPU's 11_0 sizes and depth32float-stencil8 give 11_0; depth clamping is done in
    // the programs, and wireframe is refused by name until the stage helper draws it.
    let mut caps = with_11_0_limits();
    caps.features
        .remove(Features::POLYGON_MODE | Features::DEPTH_CLAMP);
    assert!(has_abilities_of_11_0(&caps));
}

#[test]
fn no_device_without_compute_11_0_sizes_or_d32_s8_targets() {
    let mut caps = with_11_0_limits();
    caps.features.remove(Features::COMPUTE);
    assert!(!has_abilities_of_11_0(&caps));
    let mut caps = with_11_0_limits();
    caps.limits.max_texture_layers = 256;
    assert!(!has_abilities_of_11_0(&caps));
    let mut caps = with_11_0_limits();
    caps.formats.uses[Format::Depth32FloatStencil8 as usize] = FormatUse::empty();
    assert!(!has_abilities_of_11_0(&caps));
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
