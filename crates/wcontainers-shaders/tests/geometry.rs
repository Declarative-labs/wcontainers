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
    // fxc's and dxc's build of each.
    let builds = expected
        .iter()
        .flat_map(|e| ["dxbc", "dxil"].map(|b| (e, b)));
    for (&(shader, inputs, max, outputs, instances), build) in builds {
        let name = &format!("{shader}.{build}");
        let bytes = own(name);
        let source = match build {
            "dxbc" => Source::Dxbc(&bytes),
            _ => Source::Dxil(&bytes),
        };
        let c =
            convert(source, &VariantKey::default(), None).unwrap_or_else(|e| panic!("{name}: {e}"));
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

/// A geometry shader taking patches of four control points (`gs_patch`) takes them as primitives of
/// four vertices, in the compute and the replay form, from fxc's and dxc's builds.
#[test]
fn geometry_shaders_take_patches_as_primitives() {
    let replay = VariantKey {
        flags: VariantFlags::GEOMETRY_REPLAY,
        ..Default::default()
    };
    let (dxbc, dxil) = (own("gs_patch.dxbc"), own("gs_patch.dxil"));
    for source in [Source::Dxbc(&dxbc), Source::Dxil(&dxil)] {
        for key in [VariantKey::default(), replay.clone()] {
            let c = convert(source, &key, None).unwrap();
            assert_eq!(
                c.reflection.extras,
                StageExtras::Geometry {
                    input_vertices: 4,
                    max_output_vertices: 4,
                    output_vertices: 3,
                    instances: 1,
                    streams: 1,
                }
            );
            c.modules.iter().for_each(|m| drop(wgsl(m)));
        }
    }
}

/// Geometry shaders writing cull distances beside clip distances in one register (`gs_cull`,
/// `gs_cull_lines`, `gs_cull_points`) convert from fxc's and dxc's builds with their distances
/// reflected as the signature places them (two clip, three cull) and their compute form's records
/// holding the cull distances' slots past what the pass-through reads; the replay form refuses them by
/// name.
#[test]
fn geometry_shaders_write_cull_distances() {
    let replay = VariantKey {
        flags: VariantFlags::GEOMETRY_REPLAY,
        ..Default::default()
    };
    for (name, output) in [("gs_cull", 3), ("gs_cull_lines", 2), ("gs_cull_points", 1)] {
        for ext in ["dxbc", "dxil"] {
            let bytes = own(&format!("{name}.{ext}"));
            let source = |b| match ext {
                "dxbc" => Source::Dxbc(b),
                _ => Source::Dxil(b),
            };
            let c = convert(source(&bytes), &VariantKey::default(), None)
                .unwrap_or_else(|e| panic!("{name}.{ext}: {e}"));
            let StageExtras::Geometry {
                output_vertices, ..
            } = c.reflection.extras
            else {
                panic!("{name}.{ext}: {:?}", c.reflection.extras);
            };
            assert_eq!(output_vertices, output, "{name}.{ext}");
            let count = |system| -> u32 {
                c.reflection
                    .outputs
                    .iter()
                    .filter(|v| v.system == Some(system))
                    .map(|v| v.mask.count_ones())
                    .sum()
            };
            assert_eq!(
                (
                    count(SystemValue::ClipDistance),
                    count(SystemValue::CullDistance)
                ),
                (2, 3),
                "{name}.{ext}"
            );
            c.modules.iter().for_each(|m| drop(wgsl(m)));
            assert_eq!(
                convert(source(&bytes), &replay, None).err(),
                Some(ConvertError::Unsupported(
                    wcontainers_shaders::refusal::GEOMETRY_CULL_DISTANCE_REPLAY
                )),
                "{name}.{ext}"
            );
        }
    }
    wgsl(&wcontainers_shaders::stage::geometry_cull_program().unwrap());
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
        "gs_rows_lines",
        "gs_rows_streams",
        "gs_rows_out",
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
    // The last vertex stage's rewrites the stage helper's vertex program does not make.
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

/// Depth clamped behind a geometry shader: the pass-through keeps its position inside the clip
/// volume and passes the unclamped depth at the linked location, as a vertex program would.
#[test]
fn depth_clamping_reaches_the_pass_through() {
    let code = own("gs_quads.dxbc");
    let plain = convert(Source::Dxbc(&code), &VariantKey::default(), None).unwrap();
    let location = stage::locations(&plain.reflection.outputs)
        .last()
        .map_or(0, |l| l + 1);
    let key = VariantKey {
        flags: wcontainers_gpu::VariantFlags::DEPTH_CLAMP,
        linked: LinkedValues {
            location,
            clip_distances: 0,
        },
        ..Default::default()
    };
    let clamped = convert(Source::Dxbc(&code), &key, None).unwrap();
    let text = wgsl(&clamped.modules[1]);
    assert!(text.contains(&format!("@location({location})")), "{text}");
    assert!(text.contains("min("), "{text}");
    assert!(!wgsl(&plain.modules[1]).contains(&format!("@location({location})")));
    assert_ne!(clamped.modules[1].key, plain.modules[1].key);
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
    assert!(compute.contains(stage::LIST_VARIABLE), "{compute}");
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

/// The same lines drawn by Direct3D's aliased rule into a multisampled target: the pass-through
/// writes the line's ends flat at the coverage location, where the pixel program made with
/// `LINE_DIAMOND` reads them and discards the pixels the rule leaves; a pixel program keyed for both
/// line routes is refused. Drawn in `tests/stage_draw.rs`.
#[test]
fn aliased_lines_take_the_stage_helpers_route() {
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
            flags: VariantFlags::LINE_DIAMOND,
            linked: LinkedValues {
                location,
                clip_distances: 0,
            },
            ..Default::default()
        };
        let programs =
            stage::line_programs(&vs, &ps_key).unwrap_or_else(|e| panic!("{route}: {e}"));
        assert!(programs.diamond, "{route}");
        let ends = stage::line_coverage_location(&ps_key);
        let flat = format!("@location({ends}) @interpolate(flat");
        let passthrough = wgsl(&programs.passthrough);
        assert!(passthrough.contains(&flat), "{route}: {passthrough}");
        let ps = convert(read(&ps_code), &ps_key, None).unwrap_or_else(|e| panic!("{route}: {e}"));
        let fragment = wgsl(&ps.modules[0]);
        assert!(fragment.contains(&flat), "{route}: {fragment}");
        assert!(fragment.contains("fn diamond_exit"), "{route}");
        assert!(fragment.contains("discard"), "{route}");
        assert!(
            matches!(
                ps.reflection.extras,
                StageExtras::Pixel { discards: true, .. }
            ),
            "{route}"
        );
        let both = VariantKey {
            flags: VariantFlags::LINE_DIAMOND | VariantFlags::LINE_COVERAGE,
            ..ps_key.clone()
        };
        assert!(
            matches!(
                convert(read(&ps_code), &both, None),
                Err(ConvertError::Malformed(_))
            ),
            "{route}"
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

/// A geometry shader writing its primitives' numbers (`gs_numbered`, and `gs_numbered_layer` writing a
/// layer too) by both routes: its reflection lists the output, its records hold it after the layer's
/// slot, and under `PRIMITIVE_ID_FROM_STAGE` (and that with `CLIP_DISTANCE`'s linking) the pass-through
/// passes it as a flat value where `ps_numbered` made so reads it. Drawn in `tests/geometry_draw.rs`.
#[test]
fn geometry_shaders_pass_their_primitive_numbers_on() {
    for (route, dxil) in [("dxbc", false), ("dxil", true)] {
        let read = |code| match dxil {
            false => Source::Dxbc(code),
            true => Source::Dxil(code),
        };
        let gs_code = own(&format!("gs_numbered.{route}"));
        let ps_code = own(&format!("ps_numbered.{route}"));
        let plain = convert(read(&gs_code), &VariantKey::default(), None)
            .unwrap_or_else(|e| panic!("{route}: {e}"));
        assert!(stage::writes_primitive_id(&plain.reflection), "{route}");
        // The record: the position, each output location, then the number, which the pass-through
        // reads only when it passes it on.
        let slot = 1 + stage::locations(&plain.reflection.outputs).len();
        assert!(!reads_slot(&wgsl(&plain.modules[1]), slot), "{route}");
        let location = stage::locations(&plain.reflection.outputs)
            .last()
            .map_or(0, |l| l + 1);
        let key = VariantKey {
            flags: VariantFlags::PRIMITIVE_ID_FROM_STAGE,
            linked: LinkedValues {
                location,
                clip_distances: 0,
            },
            ..Default::default()
        };
        let at = stage::primitive_id_location(&key);
        let flat = format!("@location({at}) @interpolate(flat");
        let gs = convert(read(&gs_code), &key, None).unwrap_or_else(|e| panic!("{route}: {e}"));
        let passthrough = wgsl(&gs.modules[1]);
        assert!(passthrough.contains(&flat), "{route}: {passthrough}");
        assert!(reads_slot(&passthrough, slot), "{route}: {passthrough}");
        assert_ne!(gs.modules[1].key, plain.modules[1].key, "{route}");
        let ps = convert(read(&ps_code), &key, None).unwrap_or_else(|e| panic!("{route}: {e}"));
        let fragment = wgsl(&ps.modules[0]);
        assert!(fragment.contains(&flat), "{route}: {fragment}");
        assert!(!fragment.contains("primitive_index"), "{route}: {fragment}");
        // Passed on after the unclamped depth, as the last vertex stage passes them.
        let clamped = VariantKey {
            flags: VariantFlags::PRIMITIVE_ID_FROM_STAGE | VariantFlags::DEPTH_CLAMP,
            ..key.clone()
        };
        let gs = convert(read(&gs_code), &clamped, None).unwrap_or_else(|e| panic!("{route}: {e}"));
        let at = stage::primitive_id_location(&clamped);
        assert_eq!(at, location + 1, "{route}");
        let passthrough = wgsl(&gs.modules[1]);
        assert!(
            passthrough.contains(&format!("@location({at}) @interpolate(flat")),
            "{route}: {passthrough}"
        );
        // After a layer: the position, a location, the layer and viewport slot, then the number.
        let layered = own(&format!("gs_numbered_layer.{route}"));
        let gs = convert(read(&layered), &key, None).unwrap_or_else(|e| panic!("{route}: {e}"));
        assert!(stage::selects(&gs.reflection), "{route}");
        let selection = 1 + stage::locations(&gs.reflection.outputs).len();
        let passthrough = wgsl(&gs.modules[1]);
        assert!(
            passthrough.contains(&format!("@location({selection}) selection: vec4<u32>")),
            "{route}: {passthrough}"
        );
        assert!(
            reads_slot(&passthrough, selection + 1),
            "{route}: {passthrough}"
        );
    }
}

/// Whether a pass-through's entry point takes record slot `slot` (a `vec4<u32>` argument at that
/// location).
fn reads_slot(text: &str, slot: usize) -> bool {
    let at = format!("@location({slot}) ");
    text.match_indices(&at).any(|(i, _)| {
        let rest = &text[i + at.len()..];
        let argument = &rest[..rest.find([',', ')', '\n']).unwrap_or(rest.len())];
        !argument.starts_with('@') && argument.trim_end().ends_with("vec4<u32>")
    })
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

/// Each own hull and domain shader, as fxc and as dxc build it, declares its domain, partitioning,
/// output and control points, and the two builds reflect the same inputs.
#[test]
fn hull_and_domain_shaders_declare_what_they_are() {
    let converted = |name: &str, ext: &str| {
        let file = format!("{name}.{ext}");
        let bytes = own(&file);
        let source = match ext {
            "dxbc" => Source::Dxbc(&bytes),
            _ => Source::Dxil(&bytes),
        };
        convert(source, &VariantKey::default(), None).unwrap_or_else(|e| panic!("{file}: {e:?}"))
    };
    for (domain, name) in [(0u8, "tri"), (1, "quad"), (2, "line")] {
        let outputs: &[(u8, &str)] = match domain {
            2 => &[(0, "point"), (1, "line")],
            _ => &[(0, "point"), (2, "cw"), (3, "ccw")],
        };
        for (partitioning, mode) in ["integer", "pow2", "odd", "even"].iter().enumerate() {
            for &(output, out) in outputs {
                let hs = format!("hs_{name}_{mode}_{out}");
                // Powers of two partition as integers, as Direct3D's tessellator has them.
                let partitioning = if partitioning == 1 {
                    0
                } else {
                    partitioning as u8
                };
                let mut inputs = Vec::new();
                for ext in ["dxbc", "dxil"] {
                    let c = converted(&hs, ext);
                    assert_eq!(c.modules.len(), 1);
                    assert_eq!(c.modules[0].stage, wcontainers_gpu::ShaderStage::Compute);
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
                        "{hs}.{ext}"
                    );
                    wgsl(&c.modules[0]);
                    // A system value is linked by its kind; dxbc-spirv also names it.
                    let mut values = c.reflection.inputs;
                    for v in values.iter_mut().filter(|v| v.system.is_some()) {
                        v.semantic.clear();
                    }
                    inputs.push(values);
                }
                assert_eq!(inputs[0], inputs[1], "{hs}: the builds' inputs");
            }
        }
        let ds = format!("ds_{name}");
        for ext in ["dxbc", "dxil"] {
            let c = converted(&ds, ext);
            assert_eq!(c.modules[0].stage, wcontainers_gpu::ShaderStage::Vertex);
            assert_eq!(
                c.reflection.extras,
                StageExtras::Domain {
                    domain,
                    input_control_points: 1,
                },
                "{ds}.{ext}"
            );
            assert!(c.reflection.inputs.is_empty(), "{ds}.{ext}");
            wgsl(&c.modules[0]);
        }
    }
}

/// A hull shader handing the domain shader no control points (`hs_tri_none`, which dxc refuses to
/// build) and a domain shader taking none (`ds_tri_none`, fxc's and dxc's builds) convert.
#[test]
fn hulls_may_give_no_control_points() {
    let c = converted_own("hs_tri_none", "dxbc");
    assert!(
        matches!(
            c.reflection.extras,
            StageExtras::Hull {
                input_control_points: 1,
                output_control_points: 0,
                domain: 0,
                partitioning: 2,
                output_primitive: 2,
                point_locations: 0,
                ..
            }
        ),
        "{:?}",
        c.reflection.extras
    );
    wgsl(&c.modules[0]);
    for ext in ["dxbc", "dxil"] {
        let c = converted_own("ds_tri_none", ext);
        assert_eq!(
            c.reflection.extras,
            StageExtras::Domain {
                domain: 0,
                input_control_points: 0,
            },
            "{ext}"
        );
        wgsl(&c.modules[0]);
    }
}

/// An own shader as fxc (`dxbc`) or dxc (`dxil`) builds it, converted under the default key.
fn converted_own(name: &str, ext: &str) -> Converted {
    let file = format!("{name}.{ext}");
    let bytes = own(&file);
    let source = match ext {
        "dxbc" => Source::Dxbc(&bytes),
        _ => Source::Dxil(&bytes),
    };
    convert(source, &VariantKey::default(), None).unwrap_or_else(|e| panic!("{file}: {e:?}"))
}

/// Hull shaders taking three control points and giving four, each holding the position, a vector and
/// an array of rows (which dxc writes as one array of rows), and the domain shader reading them: for
/// these programs the two builds make the same records (the same extras and inputs). Records are not
/// the same for every program (a quad hull's patch constants can take another count of locations in
/// each build), and a front pairs a hull and a domain stage of one build only (drawn in
/// tests/tessellation_draw.rs).
#[test]
fn hull_values_make_the_same_records_on_both_routes() {
    for hs in [
        "hs_values_odd_ccw",
        "hs_values_even_cw",
        "hs_values_integer_point",
    ] {
        let [dxbc, dxil] = ["dxbc", "dxil"].map(|ext| converted_own(hs, ext));
        let StageExtras::Hull {
            input_control_points,
            output_control_points,
            ..
        } = dxbc.reflection.extras
        else {
            panic!("{hs}: {:?}", dxbc.reflection.extras);
        };
        assert_eq!(
            (input_control_points, output_control_points),
            (3, 4),
            "{hs}"
        );
        assert_eq!(dxbc.reflection.extras, dxil.reflection.extras, "{hs}");
        // In any order; a system value is linked by its kind.
        let named = |c: Converted| {
            let mut values = c.reflection.inputs;
            for v in values.iter_mut().filter(|v| v.system.is_some()) {
                v.semantic.clear();
            }
            values.sort_by_key(|v| format!("{v:?}"));
            values
        };
        assert_eq!(named(dxbc), named(dxil), "{hs}: the builds' inputs");
    }
    for ext in ["dxbc", "dxil"] {
        let c = converted_own("ds_values", ext);
        assert_eq!(
            c.reflection.extras,
            StageExtras::Domain {
                domain: 0,
                input_control_points: 4,
            },
            "ds_values.{ext}"
        );
        wgsl(&c.modules[0]);
    }
}

/// dxc's build of each own geometry shader reflects the outputs fxc's does: each at its stream,
/// register and semantic, the streams past the first included (`gs_streams`), each row of an array
/// at its own register (`gs_rows_*`).
#[test]
fn dxil_geometry_outputs_match_dxbc_on_every_stream() {
    for gs in [
        "gs_quads",
        "gs_cut",
        "gs_instances",
        "gs_lines",
        "gs_points",
        "gs_unplaced",
        "gs_adjacent",
        "gs_line_adjacent",
        "gs_clip",
        "gs_streams",
        "gs_rows_lines",
        "gs_rows_streams",
        "gs_rows_out",
    ] {
        let keyed = |c: Converted| {
            let mut keys: Vec<_> = c
                .reflection
                .outputs
                .iter()
                .map(|v| {
                    (
                        v.stream,
                        v.semantic.clone(),
                        v.semantic_index,
                        v.system,
                        v.register,
                        v.location,
                        v.scalar,
                    )
                })
                .collect();
            keys.sort_by_key(|k| format!("{k:?}"));
            keys
        };
        let [dxbc, dxil] = ["dxbc", "dxil"].map(|ext| keyed(converted_own(gs, ext)));
        assert_eq!(dxbc, dxil, "{gs}");
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
