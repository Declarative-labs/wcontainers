//! Geometry shaders converted for the stage helper: what each declares, the programs made of them, and
//! what is refused. Drawn against real Direct3D 11 in `tests/geometry_draw.rs`.

use std::path::Path;
use wcontainers_gpu::{
    HelperValue, LinkedValues, StageExtras, SystemValue, VariantFlags, VariantKey, VertexFormat,
};
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

/// The replay form (`GEOMETRY_REPLAY`, for back ends without compute) is a vertex program writing a
/// record a slot per location, the same records the compute form writes, which the same pass-through
/// draws; WebGL 2's back end writes it and the vertex program it captures.
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn geometry_shaders_have_a_replay_form() {
    let replay = VariantKey {
        flags: VariantFlags::GEOMETRY_REPLAY,
        ..Default::default()
    };
    let drawn = convert(
        Source::Dxbc(&own("vs_geometry.dxbc")),
        &VariantKey::default(),
        None,
    )
    .unwrap();
    let names = [
        "gs_quads",
        "gs_cut",
        "gs_instances",
        "gs_lines",
        "gs_points",
        "gs_adjacent",
        "gs_line_adjacent",
        "gs_clip",
        "gs_layers",
        "gs_streams",
    ];
    for name in names {
        let bytes = own(&format!("{name}.dxbc"));
        let compute = convert(Source::Dxbc(&bytes), &VariantKey::default(), None)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let c =
            convert(Source::Dxbc(&bytes), &replay, None).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(c.reflection, compute.reflection, "{name}");
        assert_eq!(c.modules.len(), 2, "{name}");
        assert_eq!(
            c.modules[0].stage,
            wcontainers_gpu::ShaderStage::Vertex,
            "{name}"
        );
        assert_eq!(wgsl(&c.modules[1]), wgsl(&compute.modules[1]), "{name}");
        // A slot per record slot the pass-through reads, whether the slot's primitive is whole, then
        // the position.
        let passthrough_slots = c.modules[1].module.entry_points[0].function.arguments.len();
        let result = c.modules[0].module.entry_points[0]
            .function
            .result
            .as_ref()
            .unwrap();
        let naga::TypeInner::Struct { members, .. } = &c.modules[0].module.types[result.ty].inner
        else {
            panic!("{name}: the replay form's outputs");
        };
        assert_eq!(members.len(), passthrough_slots + 2, "{name}");
        let settings = wcontainers_gpu_webgl::glsl::GlslSettings {
            flip_y: false,
            noperspective: true,
        };
        wcontainers_gpu_webgl::glsl::write(&c.modules[0], settings)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let captured = stage::capture_program(&drawn, &c).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(captured.stage, wcontainers_gpu::ShaderStage::Vertex);
        wcontainers_gpu_webgl::glsl::write(&captured, settings)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
    }
    // Only a geometry shader has a replay form.
    assert!(matches!(
        convert(Source::Dxbc(&own("vs_geometry.dxbc")), &replay, None),
        Err(ConvertError::Malformed(_))
    ));
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
        Some(ConvertError::Unsupported(stage::VERTEX_INPUT_NOT_PULLED))
    );
    // The last vertex stage's rewrites are not made for the stage helper's vertex program yet.
    let key = VariantKey {
        flags: wcontainers_gpu::VariantFlags::CLIP_DEPTH_REWRITE,
        ..Default::default()
    };
    assert_eq!(
        convert(Source::Dxbc(&own("gs_quads.dxbc")), &key, None).err(),
        Some(ConvertError::Unsupported(
            wcontainers_shaders::refusal::GEOMETRY_VARIANT
        ))
    );
}

/// A geometry shader writing three clip distances over two signature elements (`gs_clip`): the
/// reflection counts all three, the pass-through writes them as the built-in, and under
/// `CLIP_DISTANCE` it passes them at the linked location for the pixel program instead.
#[test]
fn geometry_clip_distances_reach_the_pass_through() {
    let code = own("gs_clip.dxbc");
    let plain = convert(Source::Dxbc(&code), &VariantKey::default(), None).unwrap();
    let components: u32 = plain
        .reflection
        .outputs
        .iter()
        .filter(|v| v.system == Some(SystemValue::ClipDistance))
        .map(|v| v.mask.count_ones())
        .sum();
    assert_eq!(components, 3, "{:?}", plain.reflection.outputs);
    let text = wgsl(&plain.modules[1]);
    assert!(text.contains("@builtin(clip_distances)"), "{text}");
    assert!(text.contains("array<f32, 3>"), "{text}");
    let location = stage::locations(&plain.reflection.outputs)
        .last()
        .map_or(0, |l| l + 1);
    let key = VariantKey {
        flags: wcontainers_gpu::VariantFlags::CLIP_DISTANCE,
        linked: LinkedValues {
            location,
            clip_distances: 3,
        },
        ..Default::default()
    };
    let linked = convert(Source::Dxbc(&code), &key, None).unwrap();
    let text = wgsl(&linked.modules[1]);
    assert!(!text.contains("clip_distances"), "{text}");
    assert!(text.contains(&format!("@location({location})")), "{text}");
    assert_ne!(linked.modules[1].key, plain.modules[1].key);
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

/// Under `LAYER_FROM_STAGE` the pass-through passes the named layer as a flat value after the linked
/// values and reads the views' layer count, and the pixel shader reads its layer from that value.
#[test]
fn the_named_layer_passes_from_the_stage_to_the_pixel_shader() {
    let key = VariantKey {
        flags: VariantFlags::LAYER_FROM_STAGE,
        linked: LinkedValues {
            location: 1,
            clip_distances: 0,
        },
        ..Default::default()
    };
    let gs = convert(Source::Dxbc(&own("gs_layers.dxbc")), &key, None).unwrap();
    let passthrough = wgsl(&gs.modules[1]);
    assert!(
        passthrough.contains("@location(1) @interpolate(flat) named_layer: u32"),
        "{passthrough}"
    );
    assert!(passthrough.contains("layer_count"), "{passthrough}");
    let ps = convert(Source::Dxbc(&own("ps_layer.dxbc")), &key, None).unwrap();
    let pixel = wgsl(&ps.modules[0]);
    assert!(pixel.contains("@location(1) @interpolate(flat)"), "{pixel}");
    let values: Vec<_> = ps.reflection.helpers.iter().map(|h| h.value).collect();
    assert_eq!(values, [HelperValue::Viewport]);
}

#[test]
fn the_stream_output_program_validates() {
    let m = stage::stream_program().unwrap();
    assert_eq!(m.stage, wcontainers_gpu::ShaderStage::Compute);
    wgsl(&m);
}

/// A vertex shader writing cull distances (`vs_cull`: three over one register, sharing it with a clip
/// distance) converts by both routes, lists both from its signature, and gives the stage helper's
/// cull route its programs: records of the position, both locations and a slot each of clip and cull
/// distances, and a pass-through passing the clip distance on. Drawn in `tests/geometry_draw.rs`.
#[test]
fn cull_distances_take_the_stage_helpers_route() {
    let key = VariantKey {
        pulled: vec![(0, VertexFormat::Float32x3), (1, VertexFormat::Float32x4)],
        ..Default::default()
    };
    let distances = |c: &Converted, system| -> u32 {
        c.reflection
            .outputs
            .iter()
            .filter(|v| v.system == Some(system))
            .map(|v| v.mask.count_ones())
            .sum()
    };
    fn source<'a>(route: &str, code: &'a [u8]) -> Source<'a> {
        match route {
            "dxbc" => Source::Dxbc(code),
            _ => Source::Dxil(code),
        }
    }
    for route in ["dxbc", "dxil"] {
        let code = own(&format!("vs_cull.{route}"));
        let source = |code| source(route, code);
        let plain = convert(source(&code), &VariantKey::default(), None)
            .unwrap_or_else(|e| panic!("{route}: {e}"));
        assert_eq!(distances(&plain, SystemValue::CullDistance), 3, "{route}");
        assert_eq!(distances(&plain, SystemValue::ClipDistance), 1, "{route}");
        let c = convert(source(&code), &key, None).unwrap_or_else(|e| panic!("{route}: {e}"));
        let programs = stage::cull_programs(&c).unwrap_or_else(|e| panic!("{route}: {e}"));
        assert_eq!(programs.locations, stage::locations(&c.reflection.outputs));
        assert_eq!(programs.locations.len(), 2, "{route}");
        assert_eq!(
            (programs.clip_distances, programs.cull_distances),
            (1, 3),
            "{route}"
        );
        assert_eq!(
            programs.compute.stage,
            wcontainers_gpu::ShaderStage::Compute
        );
        assert!(wgsl(&programs.compute).contains("fn main"), "{route}");
        let passthrough = wgsl(&programs.passthrough);
        assert!(passthrough.contains("clip_distances"), "{route}");
        assert!(!passthrough.contains("cull"), "{route}");
    }
    // Without cull distances the route is refused.
    assert!(stage::cull_programs(&vertex()).is_err());
}

/// A vertex shader's lines (`vs_line`: a colour and a flat number) take the stage helper's line route
/// by both routes: records of the position and both locations, the colour taken along the line and
/// the number from the first vertex, and a pass-through writing the coverage at the pixel program's
/// coverage location without perspective, where the pixel program made with `LINE_COVERAGE` reads it.
/// Drawn in `tests/stage_draw.rs`.
#[test]
fn antialiased_lines_take_the_stage_helpers_route() {
    let key = VariantKey {
        pulled: vec![(0, VertexFormat::Float32x3), (1, VertexFormat::Float32x4)],
        ..Default::default()
    };
    for (route, dxil) in [("dxbc", false), ("dxil", true)] {
        let read = |code| match dxil {
            false => Source::Dxbc(code),
            true => Source::Dxil(code),
        };
        let vs_code = own(&format!("vs_line.{route}"));
        let ps_code = own(&format!("ps_line.{route}"));
        let vs = convert(read(&vs_code), &key, None).unwrap_or_else(|e| panic!("{route}: {e}"));
        let location = stage::locations(&vs.reflection.outputs)
            .last()
            .map_or(0, |l| l + 1);
        let ps_key = VariantKey {
            flags: VariantFlags::LINE_COVERAGE,
            linked: LinkedValues {
                location,
                clip_distances: 0,
            },
            ..Default::default()
        };
        let programs =
            stage::line_programs(&vs, &ps_key).unwrap_or_else(|e| panic!("{route}: {e}"));
        assert_eq!(programs.locations.len(), 2, "{route}");
        assert_eq!((programs.interpolated, programs.linear), (1, 0), "{route}");
        let coverage = stage::line_coverage_location(&ps_key);
        assert_eq!(coverage, location, "{route}");
        let passthrough = wgsl(&programs.passthrough);
        assert!(
            passthrough.contains(&format!("@location({coverage}) @interpolate(linear")),
            "{route}: {passthrough}"
        );
        let ps = convert(read(&ps_code), &ps_key, None).unwrap_or_else(|e| panic!("{route}: {e}"));
        let fragment = wgsl(&ps.modules[0]);
        assert!(
            fragment.contains(&format!("@location({coverage}) @interpolate(linear")),
            "{route}: {fragment}"
        );
    }
}

/// A pixel shader reading its primitive's number (`ps_primitive`) behind `vs_line` takes the stage
/// helper's primitive route by both routes: the pass-through writes the number as a flat u32 where
/// the pixel program made with `PRIMITIVE_ID_FROM_STAGE` reads it, which no longer reads the
/// built-in. Drawn in `tests/stage_draw.rs`.
#[test]
fn primitive_numbers_take_the_stage_helpers_route() {
    let key = VariantKey {
        pulled: vec![(0, VertexFormat::Float32x3), (1, VertexFormat::Float32x4)],
        ..Default::default()
    };
    for (route, dxil) in [("dxbc", false), ("dxil", true)] {
        let read = |code| match dxil {
            false => Source::Dxbc(code),
            true => Source::Dxil(code),
        };
        let vs_code = own(&format!("vs_line.{route}"));
        let ps_code = own(&format!("ps_primitive.{route}"));
        let vs = convert(read(&vs_code), &key, None).unwrap_or_else(|e| panic!("{route}: {e}"));
        let plain = convert(read(&ps_code), &VariantKey::default(), None)
            .unwrap_or_else(|e| panic!("{route}: {e}"));
        assert!(
            wgsl(&plain.modules[0]).contains("primitive_index"),
            "{route}"
        );
        let location = stage::locations(&vs.reflection.outputs)
            .last()
            .map_or(0, |l| l + 1);
        let ps_key = VariantKey {
            flags: VariantFlags::PRIMITIVE_ID_FROM_STAGE,
            linked: LinkedValues {
                location,
                clip_distances: 0,
            },
            ..Default::default()
        };
        let programs =
            stage::primitive_programs(&vs, &ps_key).unwrap_or_else(|e| panic!("{route}: {e}"));
        let at = stage::primitive_id_location(&ps_key);
        assert_eq!(at, location, "{route}");
        let flat = format!("@location({at}) @interpolate(flat");
        let passthrough = wgsl(&programs.passthrough);
        assert!(passthrough.contains(&flat), "{route}: {passthrough}");
        let ps = convert(read(&ps_code), &ps_key, None).unwrap_or_else(|e| panic!("{route}: {e}"));
        let fragment = wgsl(&ps.modules[0]);
        assert!(fragment.contains(&flat), "{route}: {fragment}");
        assert!(!fragment.contains("primitive_index"), "{route}: {fragment}");
    }
}

#[test]
fn the_primitive_program_validates() {
    let program = stage::primitive_program().unwrap();
    assert!(wgsl(&program).contains("fn main"));
}

#[test]
fn the_line_program_validates() {
    let program = stage::line_program().unwrap();
    assert!(wgsl(&program).contains("fn main"));
}

#[test]
fn the_strip_program_validates() {
    let program = stage::strip_program().unwrap();
    assert!(wgsl(&program).contains("fn main"));
}

#[test]
fn the_cull_program_validates() {
    let program = stage::cull_program().unwrap();
    assert!(wgsl(&program).contains("fn main"));
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
