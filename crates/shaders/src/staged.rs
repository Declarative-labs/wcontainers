//! The stage helper's forms (plan section 7) of geometry, hull and domain programs another route
//! brought into the common form: the same passes the DXBC and DXIL routes run, fed from a naga module
//! whose stage values are private variables, with the stage's declarations given beside it. What comes
//! back is what [`crate::stage::Stages::new`] and [`crate::stage::Stages::tessellated`] read.

use crate::ir::{builtin, primitive};
use crate::lower::geometry::{Geometry, GeometryPass, GeometryValue, Side};
use crate::lower::hull::{self, DomainPass, HullPass, Tessellation, TessellationValue};
use crate::stage::tess;
use crate::{ConvertError, Converted, cache};
use alloc::vec::Vec;
use naga::{Function, GlobalVariable, Handle, Module};
use wcontainers_gpu::{
    reflect::Reflection, reflect::SourceStage, reflect::VariantKey, shader::ShaderStage,
};

/// A geometry program's input primitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryInput {
    Points,
    Lines,
    LinesAdjacency,
    Triangles,
    TrianglesAdjacency,
}

/// A geometry program's output primitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryOutput {
    Points,
    LineStrip,
    TriangleStrip,
}

/// The domain a tessellator divides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TessellationDomain {
    Triangle,
    Quad,
    Isoline,
}

/// How the tessellator spaces its divisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Partitioning {
    Integer,
    FractionalOdd,
    FractionalEven,
}

/// The primitives the tessellator makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TessellatorOutput {
    Points,
    Lines,
    TrianglesClockwise,
    TrianglesCounterClockwise,
}

/// A built-in stage value, by what it holds. Each stage takes the ones its form has and refuses the
/// others by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltIn {
    Position,
    PrimitiveId,
    /// A geometry program's instance (GLSL's `gl_InvocationID` there).
    GeometryInstance,
    Layer,
    ViewportIndex,
    /// An array of floats.
    ClipDistance,
    /// The output control point a hull program's control point function runs for.
    ControlPointId,
    /// A hull program's input control points.
    ControlPointCount,
    TessellationCoordinate,
    /// Arrays of floats: four outer factors, two inner.
    OuterFactors,
    InnerFactors,
}

/// Where a stage value is passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// A location (a register of four components) and the first component it takes.
    Location {
        location: u32,
        component: u32,
    },
    BuiltIn(BuiltIn),
}

/// One stage value: a private variable of the module the program reads (an input) or writes (an
/// output). Its type is a scalar or vector, or an array of them one location a row; a geometry
/// program's inputs, a hull program's values and a domain program's inputs held one a control point
/// (all but `patch` ones) are arrayed by vertex or control point outermost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageVariable {
    pub variable: Handle<GlobalVariable>,
    pub output: bool,
    pub place: Place,
    /// A geometry program's output stream.
    pub stream: u32,
    /// One value for the patch, not one a control point: a hull program's patch outputs and a
    /// domain program's patch inputs.
    pub patch: bool,
}

/// A geometry program. `main` runs once per input primitive and instance, after its inputs are filled;
/// each emit of stream `s` calls `emit[s]`, each strip's end calls `cut`. Those are empty functions
/// without arguments, which this crate fills, placed before every function that calls them.
#[derive(Clone, Debug)]
pub struct GeometryProgram {
    pub main: Handle<Function>,
    /// One a stream, at least one.
    pub emit: Vec<Handle<Function>>,
    pub cut: Handle<Function>,
    pub input: GeometryInput,
    pub output: GeometryOutput,
    pub max_vertices: u32,
    /// Instances a primitive runs as, at least one.
    pub instances: u32,
    pub values: Vec<StageVariable>,
}

/// A hull program (OpenGL's tessellation control program). `control_point` runs once per output
/// control point, in order, with [`BuiltIn::ControlPointId`] set, then `patch_constant` once where
/// given. The tessellator's declarations, which OpenGL takes from the evaluation program, are given here.
#[derive(Clone, Debug)]
pub struct HullProgram {
    pub control_point: Handle<Function>,
    pub patch_constant: Option<Handle<Function>>,
    pub domain: TessellationDomain,
    pub partitioning: Partitioning,
    pub output: TessellatorOutput,
    pub input_points: u32,
    pub output_points: u32,
    pub values: Vec<StageVariable>,
}

/// A domain program (OpenGL's tessellation evaluation program): the module's one vertex entry point,
/// its inputs the private variables of `values`, filled from the tessellator's point its vertex
/// number names before the entry point's own code runs; its outputs its entry point's own.
#[derive(Clone, Debug)]
pub struct DomainProgram {
    pub domain: TessellationDomain,
    /// The control points of its patch: the hull program's output control points.
    pub input_points: u32,
    pub values: Vec<StageVariable>,
}

/// Why a program handed over is malformed.
const NO_ENTRY_POINT: &str = "a domain program without its one vertex entry point";

/// A geometry program's compute form (or its replay form, under `VariantFlags::GEOMETRY_REPLAY`) and
/// the vertex program drawing its output, as [`crate::convert`] makes them, from `module`, whose entry
/// points are replaced. `reflection` is the route's reflection of the program's resources; its stage
/// values, stage and extras are made here. The modules carry [`cache::rewritten_key`] of `route_key`.
pub fn geometry(
    module: Module,
    program: &GeometryProgram,
    reflection: Reflection,
    variant: &VariantKey,
    route_key: &[u8; cache::KEY_BYTES],
) -> Result<Converted, ConvertError> {
    let made = crate::contained(move || {
        let (mut module, mut reflection) = (module, reflection);
        crate::unbuilt(variant, None)?;
        let mut values = Vec::new();
        // Every stream with an emit function, and every stream an output names.
        let mut stream_mask = 1u64
            .checked_shl(program.emit.len() as u32)
            .map_or(u64::MAX, |past| past - 1);
        for v in &program.values {
            let builtin = v.place_builtin();
            let side = v.side();
            if side == Side::Output {
                stream_mask |= 1u64.checked_shl(v.stream).unwrap_or(u64::MAX);
            }
            values.push(GeometryValue {
                var: v.variable,
                side,
                location: v.location(),
                builtin,
                stream: v.stream,
                ty: ty(&module, v.variable, crate::refusal::GEOMETRY_INPUT_SHAPE)?,
            });
        }
        let mut geometry = Geometry::declared(
            geometry_input(program.input),
            geometry_output(program.output),
            program.max_vertices,
            program.instances,
            stream_mask,
        )?;
        geometry.emit = program.emit.clone();
        geometry.cut = Some(program.cut);
        module.entry_points.clear();
        let mut pass = GeometryPass {
            module: &mut module,
            variant,
            geometry: &mut geometry,
            values: &values,
            reflected: Vec::new(),
        };
        pass.functions()?;
        let extras = pass.entry_point(program.main)?;
        let [inputs, outputs] =
            crate::dxil::staged::place(pass.reflected, |v| values[v].stream, [&[], &[]]);
        let stage = match module.entry_points[0].stage {
            naga::ShaderStage::Vertex => ShaderStage::Vertex,
            _ => ShaderStage::Compute,
        };
        reflection.stage = SourceStage::Geometry;
        reflection.extras = extras;
        reflection.inputs = inputs;
        reflection.outputs = outputs;
        formed(module, reflection, stage, variant)
    })?;
    let (module, reflection, stage) = made;
    crate::finished(
        module,
        reflection,
        stage,
        variant,
        cache::rewritten_key(route_key, variant),
    )
}

/// A hull program's compute form, as [`crate::convert`] makes it, from `module`, whose entry points are
/// replaced; otherwise as [`geometry`].
pub fn hull(
    module: Module,
    program: &HullProgram,
    reflection: Reflection,
    variant: &VariantKey,
    route_key: &[u8; cache::KEY_BYTES],
) -> Result<Converted, ConvertError> {
    let made = crate::contained(move || {
        let (mut module, mut reflection) = (module, reflection);
        crate::unbuilt(variant, None)?;
        let values =
            tessellation_values(&module, &program.values, crate::refusal::HULL_INPUT_SHAPE)?;
        let (domain, partitioning, output) = (
            domain_code(program.domain),
            match program.partitioning {
                Partitioning::Integer => hull::PARTITIONING_INTEGER,
                Partitioning::FractionalOdd => hull::PARTITIONING_FRACTIONAL_ODD,
                Partitioning::FractionalEven => hull::PARTITIONING_FRACTIONAL_EVEN,
            },
            match program.output {
                TessellatorOutput::Points => tess::OUTPUT_POINT,
                TessellatorOutput::Lines => tess::OUTPUT_LINE,
                TessellatorOutput::TrianglesClockwise => hull::OUTPUT_CLOCKWISE,
                TessellatorOutput::TrianglesCounterClockwise => hull::OUTPUT_COUNTER_CLOCKWISE,
            },
        );
        module.entry_points.clear();
        let mut pass = HullPass {
            module: &mut module,
            tessellation: Tessellation {
                domain,
                partitioning,
                output,
                points_in: program.input_points,
                points_out: program.output_points,
                constants: 0,
            },
            values: &values,
            reflected: Vec::new(),
        };
        let extras = pass.entry_point(program.control_point, program.patch_constant)?;
        let [inputs, _] = crate::dxil::staged::place(pass.reflected, |_| 0, [&[], &[]]);
        reflection.stage = SourceStage::Hull;
        reflection.extras = extras;
        reflection.inputs = inputs;
        reflection.outputs = Vec::new();
        formed(module, reflection, ShaderStage::Compute, variant)
    })?;
    let (module, reflection, stage) = made;
    crate::finished(
        module,
        reflection,
        stage,
        variant,
        cache::rewritten_key(route_key, variant),
    )
}

/// A domain program's vertex form, as [`crate::convert`] makes it, from `module`; otherwise as
/// [`geometry`], its outputs' reflection kept from `reflection`.
pub fn domain(
    module: Module,
    program: &DomainProgram,
    reflection: Reflection,
    variant: &VariantKey,
    route_key: &[u8; cache::KEY_BYTES],
) -> Result<Converted, ConvertError> {
    let made = crate::contained(move || {
        let (mut module, mut reflection) = (module, reflection);
        crate::unbuilt(variant, None)?;
        match module.entry_points.as_slice() {
            [ep] if ep.stage == naga::ShaderStage::Vertex => {}
            _ => return Err(ConvertError::Malformed(NO_ENTRY_POINT.into())),
        }
        if program.input_points == 0 {
            return Err(ConvertError::Unsupported(
                crate::refusal::DOMAIN_WITHOUT_CONTROL_POINTS,
            ));
        }
        let values =
            tessellation_values(&module, &program.values, crate::refusal::DOMAIN_INPUT_SHAPE)?;
        if values.iter().any(|v| v.side == Side::Output) {
            return Err(ConvertError::Unsupported(crate::refusal::DOMAIN_INPUT));
        }
        let (function, extras) = DomainPass {
            module: &mut module,
            tessellation: Tessellation {
                domain: domain_code(program.domain),
                points_in: program.input_points,
                ..Tessellation::default()
            },
            values: &values,
        }
        .inputs()?;
        hull::call_domain_inputs(&mut module, function)?;
        module.entry_points[0].name = crate::ENTRY_POINT.into();
        reflection.stage = SourceStage::Domain;
        reflection.extras = extras;
        reflection.inputs = Vec::new();
        formed(module, reflection, ShaderStage::Vertex, variant)
    })?;
    let (module, reflection, stage) = made;
    crate::finished(
        module,
        reflection,
        stage,
        variant,
        cache::rewritten_key(route_key, variant),
    )
}

/// The rewrites after a stage's form is made: 15.4's, then the variant's.
fn formed(
    mut module: Module,
    mut reflection: Reflection,
    stage: ShaderStage,
    variant: &VariantKey,
) -> Result<(Module, Reflection, ShaderStage), ConvertError> {
    crate::route_rewrites(&mut module, &mut reflection, stage, variant)?;
    crate::varied(&mut module, &mut reflection, stage, variant)?;
    Ok((module, reflection, stage))
}

impl StageVariable {
    fn side(&self) -> Side {
        match self.output {
            true => Side::Output,
            false => Side::Input,
        }
    }

    fn location(&self) -> Option<(u32, u32)> {
        match self.place {
            Place::Location {
                location,
                component,
            } => Some((location, component)),
            Place::BuiltIn(_) => None,
        }
    }

    /// Its built-in as `ir::builtin` numbers it.
    fn place_builtin(&self) -> Option<u64> {
        let Place::BuiltIn(b) = self.place else {
            return None;
        };
        Some(match b {
            BuiltIn::Position => builtin::POSITION,
            BuiltIn::PrimitiveId => builtin::PRIMITIVE_ID,
            BuiltIn::GeometryInstance => builtin::GS_INSTANCE_ID,
            BuiltIn::Layer => builtin::LAYER_INDEX,
            BuiltIn::ViewportIndex => builtin::VIEWPORT_INDEX,
            BuiltIn::ClipDistance => builtin::CLIP_DISTANCE,
            BuiltIn::ControlPointId => builtin::TESS_CONTROL_POINT_ID,
            BuiltIn::ControlPointCount => builtin::TESS_CONTROL_POINT_COUNT_IN,
            BuiltIn::TessellationCoordinate => builtin::TESS_COORD,
            BuiltIn::OuterFactors => builtin::TESS_FACTOR_OUTER,
            BuiltIn::InnerFactors => builtin::TESS_FACTOR_INNER,
        })
    }
}

/// The type of a stage variable as the passes take it, or the refusal `shape`.
fn ty(
    module: &Module,
    variable: Handle<GlobalVariable>,
    shape: &'static str,
) -> Result<crate::ir::Ty, ConvertError> {
    let global = module
        .global_variables
        .try_get(variable)
        .map_err(|e| ConvertError::Malformed(alloc::format!("{e}")))?;
    crate::dxil::staged::ir_ty(module, global.ty, shape)
}

/// A hull or domain program's values as the passes take them.
fn tessellation_values(
    module: &Module,
    values: &[StageVariable],
    shape: &'static str,
) -> Result<Vec<TessellationValue>, ConvertError> {
    values
        .iter()
        .map(|v| {
            let location = v.location();
            let builtin = v.place_builtin();
            Ok(TessellationValue {
                var: v.variable,
                side: v.side(),
                location,
                builtin,
                per_point: !v.patch && (location.is_some() || builtin == Some(builtin::POSITION)),
                ty: ty(module, v.variable, shape)?,
            })
        })
        .collect()
}

fn geometry_input(p: GeometryInput) -> u64 {
    match p {
        GeometryInput::Points => primitive::POINTS,
        GeometryInput::Lines => primitive::LINES,
        GeometryInput::LinesAdjacency => primitive::LINES_ADJ,
        GeometryInput::Triangles => primitive::TRIANGLES,
        GeometryInput::TrianglesAdjacency => primitive::TRIANGLES_ADJ,
    }
}

fn geometry_output(p: GeometryOutput) -> u64 {
    match p {
        GeometryOutput::Points => primitive::POINTS,
        GeometryOutput::LineStrip => primitive::LINES,
        GeometryOutput::TriangleStrip => primitive::TRIANGLES,
    }
}

fn domain_code(d: TessellationDomain) -> u8 {
    match d {
        TessellationDomain::Triangle => hull::DOMAIN_TRIANGLE,
        TessellationDomain::Quad => hull::DOMAIN_QUAD,
        TessellationDomain::Isoline => hull::DOMAIN_ISOLINE,
    }
}
