//! Geometry shaders converted for the stage helper: what each declares, the programs made of them, and
//! what is refused. Drawn against real Direct3D 11 in `tests/geometry_draw.rs`.

use std::path::Path;
use wcontainers_gpu::{StageExtras, SystemValue, VariantKey, VertexFormat};
use wcontainers_shaders::{ConvertError, Converted, Source, convert, stage};

fn own(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/own")
            .join(name),
    )
    .unwrap()
}

fn wgsl(m: &wcontainers_gpu::ShaderModule) -> String {
    naga::back::wgsl::write_string(&m.module, &m.info, naga::back::wgsl::WriterFlags::empty())
        .unwrap()
}

/// `vs_geometry` with both inputs pulled, as the stage helper needs it.
fn vertex() -> Converted {
    let key = VariantKey {
        pulled: vec![(0, VertexFormat::Float32x3), (1, VertexFormat::Float32x4)],
        ..Default::default()
    };
    convert(Source::Dxbc(&own("vs_geometry.dxbc")), &key, None).unwrap()
}

#[test]
fn geometry_shaders_declare_what_they_are() {
    // (shader, input vertices, maximum output vertices, output vertices per primitive, instances)
    let expected = [
        ("gs_quads", 1, 4, 3, 1),
        ("gs_cut", 3, 8, 3, 1),
        ("gs_instances", 3, 3, 3, 3),
        ("gs_lines", 2, 3, 2, 1),
        ("gs_points", 1, 4, 1, 1),
        ("gs_adjacent", 6, 3, 3, 1),
        ("gs_line_adjacent", 4, 4, 2, 1),
    ];
    let vertex = vertex();
    for (name, inputs, max, outputs, instances) in expected {
        let c = convert(
            Source::Dxbc(&own(&format!("{name}.dxbc"))),
            &VariantKey::default(),
            None,
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            c.reflection.extras,
            StageExtras::Geometry {
                input_vertices: inputs,
                max_output_vertices: max,
                output_vertices: outputs,
                instances,
                streams: 1,
            },
            "{name}"
        );
        assert_eq!(c.modules.len(), 2, "{name}");
        assert_eq!(c.modules[0].stage, wcontainers_gpu::ShaderStage::Compute);
        assert_eq!(c.modules[1].stage, wcontainers_gpu::ShaderStage::Vertex);
        assert_ne!(c.modules[0].key, c.modules[1].key);
        // The outputs the pixel stage links to: the position and two locations.
        let outputs = &c.reflection.outputs;
        assert!(
            outputs
                .iter()
                .any(|v| v.system == Some(SystemValue::Position))
        );
        assert_eq!(stage::locations(outputs), [1, 2], "{name}");
        for m in &c.modules {
            let text = wgsl(m);
            assert!(text.contains("fn main"), "{name}");
        }
        // The vertex stage's compute form writes the records this shader reads.
        let records = stage::vertex_program(&vertex, &c).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(records.stage, wcontainers_gpu::ShaderStage::Compute);
        wgsl(&records);
        // A cache record gives the same modules back.
        let back = wcontainers_shaders::cache::decode(
            c.modules[0].key,
            &wcontainers_shaders::cache::encode(&c),
        )
        .unwrap();
        assert_eq!(back.modules.len(), 2);
        assert_eq!(back.modules[1].key, c.modules[1].key);
    }
}

/// A DXIL vertex program's outputs keep their own widths (`vs_transform`: a float2 and a uint): its
/// compute form fills each record slot's first components and zeroes the rest.
#[test]
fn narrower_vertex_outputs_fill_their_record_slots() {
    let key = VariantKey {
        pulled: vec![(0, VertexFormat::Float32x3), (1, VertexFormat::Float32x2)],
        ..Default::default()
    };
    let vs = convert(Source::Dxil(&own("vs_transform.dxil")), &key, None).unwrap();
    let gs = convert(
        Source::Dxbc(&own("gs_quads.dxbc")),
        &VariantKey::default(),
        None,
    )
    .unwrap();
    let records = stage::vertex_program(&vs, &gs).unwrap();
    let text = wgsl(&records);
    assert!(text.contains("vec4<u32>("), "{text}");
}

#[test]
fn geometry_refusals_are_named() {
    // A vertex program whose inputs are not pulled cannot run as compute.
    let plain = convert(
        Source::Dxbc(&own("vs_geometry.dxbc")),
        &VariantKey::default(),
        None,
    )
    .unwrap();
    let gs = convert(
        Source::Dxbc(&own("gs_quads.dxbc")),
        &VariantKey::default(),
        None,
    )
    .unwrap();
    assert_eq!(
        stage::vertex_program(&plain, &gs).err(),
        Some(ConvertError::Unsupported("VertexInputNotPulled"))
    );
    // The last vertex stage's rewrites are not made for the stage helper's vertex program yet.
    let key = VariantKey {
        flags: wcontainers_gpu::VariantFlags::CLIP_DEPTH_REWRITE,
        ..Default::default()
    };
    assert_eq!(
        convert(Source::Dxbc(&own("gs_quads.dxbc")), &key, None).err(),
        Some(ConvertError::Unsupported("GeometryVariant"))
    );
}

/// A geometry shader writing its layer and viewport (`gs_layers`): both are in its reflection, its
/// records end with their slot, and the pass-through reads that slot and the run's helper constants.
#[test]
fn layered_geometry_shaders_name_each_vertex_run() {
    let gs = convert(
        Source::Dxbc(&own("gs_layers.dxbc")),
        &VariantKey::default(),
        None,
    )
    .unwrap();
    let systems: Vec<_> = gs
        .reflection
        .outputs
        .iter()
        .filter_map(|v| v.system)
        .collect();
    assert!(
        systems.contains(&SystemValue::RenderTargetArrayIndex),
        "{systems:?}"
    );
    assert!(
        systems.contains(&SystemValue::ViewportArrayIndex),
        "{systems:?}"
    );
    assert!(stage::selects(&gs.reflection));
    let passthrough = wgsl(&gs.modules[1]);
    assert!(
        passthrough.contains("@location(1) selection: vec4<u32>"),
        "{passthrough}"
    );
    assert!(
        passthrough.contains("@group(3) @binding(0)"),
        "{passthrough}"
    );
    // Its compute form writes the slot after the position: two slots of four words per vertex.
    let compute = wgsl(&gs.modules[0]);
    assert!(compute.contains("stage_list"), "{compute}");
    let plain = convert(
        Source::Dxbc(&own("gs_quads.dxbc")),
        &VariantKey::default(),
        None,
    )
    .unwrap();
    assert!(!stage::selects(&plain.reflection));
}

#[test]
fn the_stream_output_program_validates() {
    let m = stage::stream_program().unwrap();
    assert_eq!(m.stage, wcontainers_gpu::ShaderStage::Compute);
    wgsl(&m);
}

#[test]
fn hull_and_domain_shaders_declare_what_they_are() {
    for (domain, name) in [(0u8, "tri"), (1, "quad"), (2, "line")] {
        let outputs: &[(u8, &str)] = match domain {
            2 => &[(0, "point"), (1, "line")],
            _ => &[(0, "point"), (2, "cw"), (3, "ccw")],
        };
        for (partitioning, mode) in ["integer", "pow2", "odd", "even"].iter().enumerate() {
            for &(output, out) in outputs {
                let file = format!("hs_{name}_{mode}_{out}.dxbc");
                let c = convert(Source::Dxbc(&own(&file)), &VariantKey::default(), None)
                    .unwrap_or_else(|e| panic!("{file}: {e:?}"));
                assert_eq!(c.modules.len(), 1);
                assert_eq!(c.modules[0].stage, wcontainers_gpu::ShaderStage::Compute);
                // Powers of two partition as integers, as Direct3D's tessellator has them.
                let partitioning = if partitioning == 1 {
                    0
                } else {
                    partitioning as u8
                };
                assert_eq!(
                    c.reflection.extras,
                    StageExtras::Hull {
                        input_control_points: 1,
                        output_control_points: 1,
                        domain,
                        partitioning,
                        output_primitive: output,
                        max_tess_factor: 64,
                        point_locations: 1,
                        constant_locations: 0,
                    },
                    "{file}"
                );
                wgsl(&c.modules[0]);
            }
        }
        let file = format!("ds_{name}.dxbc");
        let c = convert(Source::Dxbc(&own(&file)), &VariantKey::default(), None)
            .unwrap_or_else(|e| panic!("{file}: {e:?}"));
        assert_eq!(c.modules[0].stage, wcontainers_gpu::ShaderStage::Vertex);
        assert!(
            matches!(c.reflection.extras, StageExtras::Domain { domain: d, .. } if d == domain),
            "{file}"
        );
        wgsl(&c.modules[0]);
    }
}

#[test]
fn the_tessellator_program_validates() {
    let m = stage::tessellator_program().unwrap();
    assert_eq!(m.stage, wcontainers_gpu::ShaderStage::Compute);
    wgsl(&m);
}

/// Prints the WGSL of the stage helper's programs for one geometry shader, for reading.
#[test]
#[ignore = "prints programs for reading"]
fn print_geometry_programs() {
    let name = std::env::var("WCONTAINERS_GS").unwrap_or_else(|_| "gs_cut".into());
    let c = convert(
        Source::Dxbc(&own(&format!("{name}.dxbc"))),
        &VariantKey::default(),
        None,
    )
    .unwrap();
    for m in &c.modules {
        println!("{}", wgsl(m));
    }
    println!("{}", wgsl(&stage::vertex_program(&vertex(), &c).unwrap()));
}
