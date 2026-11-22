//! Geometry, hull and domain programs handed over as naga modules (`wcontainers_shaders::staged`, the
//! entry the NIR route takes) make the forms the Direct3D routes make of the same programs: the same
//! declarations, stage values and modules. Drawn against real Direct3D 11 in `tests/geometry_draw.rs`
//! and `tests/tessellation_draw.rs`.

mod forms;

use std::path::Path;
use wcontainers_gpu::{ShaderStage, StageValue, VariantFlags, VariantKey};
use wcontainers_shaders::staged::{self, DomainProgram, GeometryProgram};
use wcontainers_shaders::{ConvertError, Converted, Source, convert};

fn dxbc(name: &str) -> Converted {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/own/{name}.dxbc"));
    let bytes = std::fs::read(&path).unwrap();
    convert(Source::Dxbc(&bytes), &VariantKey::default(), None)
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// A stage value without what only a Direct3D route gives: its semantic, and a system value's
/// register in the container's signature (fxc's `SV_PrimitiveID` has none, written as `u32::MAX`).
fn placed(values: &[StageValue]) -> Vec<StageValue> {
    values
        .iter()
        .map(|v| StageValue {
            semantic: String::new(),
            semantic_index: 0,
            register: if v.system.is_some() { 0 } else { v.register },
            ..v.clone()
        })
        .collect()
}

fn stages(c: &Converted) -> Vec<ShaderStage> {
    c.modules.iter().map(|m| m.stage).collect()
}

#[test]
fn geometry_programs_from_a_module_take_the_direct3d_routes_forms() {
    for flags in [VariantFlags::empty(), VariantFlags::GEOMETRY_REPLAY] {
        let key = VariantKey {
            flags,
            ..Default::default()
        };
        let ours = forms::gs_cut(&key).unwrap();
        let bytes =
            std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/own/gs_cut.dxbc"))
                .unwrap();
        let theirs = convert(Source::Dxbc(&bytes), &key, None).unwrap();
        assert_eq!(ours.reflection.stage, theirs.reflection.stage);
        assert_eq!(
            ours.reflection.extras, theirs.reflection.extras,
            "{flags:?}"
        );
        assert_eq!(
            placed(&ours.reflection.inputs),
            placed(&theirs.reflection.inputs)
        );
        assert_eq!(
            placed(&ours.reflection.outputs),
            placed(&theirs.reflection.outputs)
        );
        assert_eq!(stages(&ours), stages(&theirs), "{flags:?}");
        // The pass-through draws the same values.
        assert_eq!(
            ours.modules[1].module.entry_points[0]
                .function
                .result
                .is_some(),
            theirs.modules[1].module.entry_points[0]
                .function
                .result
                .is_some()
        );
    }
}

#[test]
fn hull_and_domain_programs_from_a_module_take_the_direct3d_routes_forms() {
    for (domain, mode, out) in [
        ("tri", "integer", "ccw"),
        ("tri", "odd", "point"),
        ("quad", "even", "cw"),
        ("quad", "pow2", "point"),
        ("line", "odd", "line"),
    ] {
        let name = format!("hs_{domain}_{mode}_{out}");
        let theirs = dxbc(&name);
        let ours = forms::hull(domain, mode, out, &theirs.reflection.bindings).unwrap();
        assert_eq!(ours.reflection.stage, theirs.reflection.stage, "{name}");
        assert_eq!(ours.reflection.extras, theirs.reflection.extras, "{name}");
        assert_eq!(
            placed(&ours.reflection.inputs),
            placed(&theirs.reflection.inputs),
            "{name}"
        );
        assert_eq!(
            ours.reflection.bindings, theirs.reflection.bindings,
            "{name}"
        );
        assert_eq!(stages(&ours), [ShaderStage::Compute], "{name}");

        let theirs = dxbc(&format!("ds_{domain}"));
        let ours = forms::domain(domain, theirs.reflection.outputs.clone()).unwrap();
        assert_eq!(
            ours.reflection.stage, theirs.reflection.stage,
            "ds_{domain}"
        );
        assert_eq!(
            ours.reflection.extras, theirs.reflection.extras,
            "ds_{domain}"
        );
        assert_eq!(stages(&ours), [ShaderStage::Vertex], "ds_{domain}");
        // The entry point's outputs are at the locations the reflection names.
        let ep = &ours.modules[0].module.entry_points[0];
        let result = ep.function.result.as_ref().unwrap();
        let naga::TypeInner::Struct { members, .. } =
            &ours.modules[0].module.types[result.ty].inner
        else {
            panic!("ds_{domain}: outputs");
        };
        for v in ours
            .reflection
            .outputs
            .iter()
            .filter(|v| v.system.is_none())
        {
            assert!(
                members.iter().any(|m| matches!(
                    m.binding,
                    Some(naga::Binding::Location { location, .. }) if location == v.location
                )),
                "ds_{domain}: {v:?}"
            );
        }
    }
}

/// What a program handed over does not declare as its form needs is refused by name.
#[test]
fn programs_from_a_module_are_refused_by_name() {
    // A stream an output names without its emit function.
    let (m, key) = forms::module("staged_geometry");
    let mut values = vec![staged::StageVariable {
        variable: forms::variable(&m, "colour"),
        output: true,
        place: staged::Place::Location {
            location: 1,
            component: 0,
        },
        stream: 1,
        patch: false,
    }];
    let program = |values: Vec<staged::StageVariable>, output| GeometryProgram {
        main: forms::function(&m, "gs_cut"),
        emit: vec![forms::function(&m, "emit")],
        cut: forms::function(&m, "cut"),
        input: staged::GeometryInput::Triangles,
        output,
        max_vertices: 8,
        instances: 1,
        values,
    };
    let reflection = || forms::bare(wcontainers_gpu::SourceStage::Geometry, Vec::new());
    let plain = VariantKey::default();
    // Several streams of strips, which Direct3D does not take.
    let strips = staged::geometry(
        m.clone(),
        &program(values.clone(), staged::GeometryOutput::TriangleStrip),
        reflection(),
        &plain,
        &key,
    );
    assert_eq!(
        strips.unwrap_err(),
        ConvertError::Unsupported(wcontainers_shaders::refusal::GEOMETRY_STREAMS)
    );
    let points = staged::geometry(
        m.clone(),
        &program(values.clone(), staged::GeometryOutput::Points),
        reflection(),
        &plain,
        &key,
    );
    assert!(
        matches!(points, Err(ConvertError::Malformed(_))),
        "{points:?}"
    );
    // A built-in no geometry form has.
    values[0] = staged::StageVariable {
        place: staged::Place::BuiltIn(staged::BuiltIn::TessellationCoordinate),
        stream: 0,
        ..values[0]
    };
    let unknown = program(values, staged::GeometryOutput::TriangleStrip);
    let unknown = staged::geometry(m.clone(), &unknown, reflection(), &plain, &key);
    assert_eq!(
        unknown.unwrap_err(),
        ConvertError::Unsupported(wcontainers_shaders::refusal::GEOMETRY_OUTPUT)
    );
    // A domain program whose patch has no control points.
    let (m, key) = forms::module("staged_domain");
    let mut m = m;
    m.entry_points.truncate(1);
    let empty = staged::domain(
        m,
        &DomainProgram {
            domain: staged::TessellationDomain::Triangle,
            input_points: 0,
            values: Vec::new(),
        },
        forms::bare(wcontainers_gpu::SourceStage::Domain, Vec::new()),
        &plain,
        &key,
    );
    assert_eq!(
        empty.unwrap_err(),
        ConvertError::Unsupported(wcontainers_shaders::refusal::DOMAIN_WITHOUT_CONTROL_POINTS)
    );
}
