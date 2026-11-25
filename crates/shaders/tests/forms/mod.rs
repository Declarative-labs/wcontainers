//! The own geometry, hull and domain shaders written as another route hands them to
//! `wcontainers_shaders::staged` (`tests/own/staged_*.wgsl`), and their forms made from them.

#![allow(dead_code)]

use naga::{Function, GlobalVariable, Handle, Module};
use std::path::Path;
use wcontainers_gpu::{
    ReflectedBinding, Reflection, SourceStage, StageExtras, StageValue, VariantKey,
};
use wcontainers_shaders::Converted;
use wcontainers_shaders::staged::{
    self, BuiltIn, DomainProgram, GeometryInput, GeometryOutput, GeometryProgram, HullProgram,
    Partitioning, Place, StageVariable, TessellationDomain, TessellatorOutput,
};

/// The module of `tests/own/{name}.wgsl`, and the key its route gives it.
pub fn module(name: &str) -> (Module, [u8; wcontainers_shaders::cache::KEY_BYTES]) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/own/{name}.wgsl"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let module = naga::front::wgsl::parse_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
    (module, *blake3::hash(text.as_bytes()).as_bytes())
}

pub fn function(m: &Module, name: &str) -> Handle<Function> {
    m.functions
        .iter()
        .find(|(_, f)| f.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no function {name}"))
        .0
}

pub fn variable(m: &Module, name: &str) -> Handle<GlobalVariable> {
    m.global_variables
        .iter()
        .find(|(_, g)| g.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no variable {name}"))
        .0
}

/// A reflection with no resources and no stage values, of `stage`.
pub fn bare(stage: SourceStage, bindings: Vec<ReflectedBinding>) -> Reflection {
    Reflection {
        stage,
        inputs: Vec::new(),
        outputs: Vec::new(),
        bindings,
        helpers: Vec::new(),
        helper_size: 0,
        sampled_pairs: Vec::new(),
        extras: StageExtras::None,
        slow_paths: Vec::new(),
    }
}

fn located(m: &Module, name: &str, output: bool, location: u32) -> StageVariable {
    StageVariable {
        variable: variable(m, name),
        output,
        place: Place::Location {
            location,
            component: 0,
        },
        stream: 0,
        patch: false,
    }
}

fn built_in(m: &Module, name: &str, output: bool, b: BuiltIn, patch: bool) -> StageVariable {
    StageVariable {
        variable: variable(m, name),
        output,
        place: Place::BuiltIn(b),
        stream: 0,
        patch,
    }
}

/// `gs_cut` of `geometry.hlsl` (its registers the locations), its forms as `variant` asks.
pub fn gs_cut(variant: &VariantKey) -> Result<Converted, wcontainers_shaders::ConvertError> {
    let (m, key) = module("staged_geometry");
    let program = GeometryProgram {
        main: function(&m, "gs_cut"),
        emit: vec![function(&m, "emit")],
        cut: function(&m, "cut"),
        input: GeometryInput::Triangles,
        output: GeometryOutput::TriangleStrip,
        max_vertices: 8,
        instances: 1,
        values: vec![
            built_in(&m, "position_in", false, BuiltIn::Position, false),
            located(&m, "colour_in", false, 1),
            located(&m, "ids_in", false, 2),
            built_in(&m, "primitive", false, BuiltIn::PrimitiveId, false),
            built_in(&m, "position_out", true, BuiltIn::Position, false),
            located(&m, "colour", true, 1),
            located(&m, "ids", true, 2),
        ],
    };
    staged::geometry(
        m,
        &program,
        bare(SourceStage::Geometry, Vec::new()),
        variant,
        &key,
    )
}

/// The tessellator's declarations as `tessellation.hlsl` names them in its hull shaders' names.
fn declarations(
    domain: &str,
    mode: &str,
    out: &str,
) -> (TessellationDomain, Partitioning, TessellatorOutput) {
    let domain = match domain {
        "tri" => TessellationDomain::Triangle,
        "quad" => TessellationDomain::Quad,
        _ => TessellationDomain::Isoline,
    };
    // pow2 partitioning is integer partitioning, as both Direct3D routes take it.
    let partitioning = match mode {
        "odd" => Partitioning::FractionalOdd,
        "even" => Partitioning::FractionalEven,
        _ => Partitioning::Integer,
    };
    let output = match out {
        "point" => TessellatorOutput::Points,
        "line" => TessellatorOutput::Lines,
        "cw" => TessellatorOutput::TrianglesClockwise,
        _ => TessellatorOutput::TrianglesCounterClockwise,
    };
    (domain, partitioning, output)
}

/// `hs_{domain}_{mode}_{out}` of `tessellation.hlsl`: one control point in and out, the factors read
/// from the constant buffer `bindings` binds (the route's reflection of it).
pub fn hull(
    domain: &str,
    mode: &str,
    out: &str,
    bindings: &[ReflectedBinding],
) -> Result<Converted, wcontainers_shaders::ConvertError> {
    let (mut m, key) = module("staged_hull");
    let constants = variable(&m, "constants");
    let [bound] = bindings else {
        panic!("{bindings:?}")
    };
    m.global_variables[constants].binding = Some(naga::ResourceBinding {
        group: bound.set,
        binding: bound.binding,
    });
    let (domain_declared, partitioning, output) = declarations(domain, mode, out);
    let program = HullProgram {
        control_point: function(&m, "control_point"),
        patch_constant: Some(function(&m, &format!("{domain}_factors"))),
        domain: domain_declared,
        partitioning,
        output,
        input_points: 1,
        output_points: 1,
        values: vec![
            located(&m, "id_in", false, 0),
            located(&m, "id_out", true, 0),
            built_in(&m, "point", false, BuiltIn::ControlPointId, false),
            built_in(&m, "primitive", false, BuiltIn::PrimitiveId, false),
            built_in(&m, "outer", true, BuiltIn::OuterFactors, true),
            built_in(&m, "inner", true, BuiltIn::InnerFactors, true),
        ],
    };
    staged::hull(
        m,
        &program,
        bare(SourceStage::Hull, bindings.to_vec()),
        &VariantKey::default(),
        &key,
    )
}

/// `ds_{domain}` of `tessellation.hlsl`, its outputs reflected as `outputs` (the route's reflection).
pub fn domain(
    domain: &str,
    outputs: Vec<StageValue>,
) -> Result<Converted, wcontainers_shaders::ConvertError> {
    let (mut m, key) = module("staged_domain");
    let entry = format!("ds_{domain}");
    m.entry_points.retain(|ep| ep.name == entry);
    let (domain_declared, _, _) = declarations(domain, "", "");
    let program = DomainProgram {
        domain: domain_declared,
        input_points: 1,
        values: vec![
            built_in(
                &m,
                "coordinate",
                false,
                BuiltIn::TessellationCoordinate,
                false,
            ),
            built_in(&m, "primitive", false, BuiltIn::PrimitiveId, false),
        ],
    };
    let reflection = Reflection {
        outputs,
        ..bare(SourceStage::Domain, Vec::new())
    };
    staged::domain(m, &program, reflection, &VariantKey::default(), &key)
}
