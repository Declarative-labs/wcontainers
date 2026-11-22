//! A pixel program's inputs taken at another place in the pixel (`EvaluateAttributeCentroid`,
//! `EvaluateAttributeAtSample`, `EvaluateAttributeSnapped`), which WGSL lacks. The reader reads
//! GLSL.std.450's `InterpolateAt*` as calls of stand-in functions (`front::spv::INTERPOLATE_AT_*`)
//! taking the input's value loaded through its pointer. As on the DXBC route (`lower/evaluate.rs`), the
//! entry point, where every lane of a quad runs, takes each evaluated input's slopes by derivatives once
//! the inputs are stored (an interpolated value is affine on the screen, over w where
//! perspective-correct), and each stand-in becomes a function moving the value by its slopes to the
//! place asked for. Places are in pixels from the pixel's centre; samples are at Direct3D's standard
//! positions; the centroid is Direct3D's: the centre where every sample is covered, else the first
//! covered sample.

use super::each_block;
use super::registers::decode;
use crate::ConvertError;
use crate::lower::evaluate::{
    CENTROID, PIXEL_CENTRE, PLACE, SAMPLE_POSITIONS, SLOPE_X, SLOPE_Y, W_AT, W_SLOPE,
};
use crate::variant::{FOUR_SAMPLES, SAMPLE_POSITION_STEPS, global_of, push, vector};
use alloc::format;
use alloc::vec::Vec;
use naga::front::spv::{INTERPOLATE_AT_CENTROID, INTERPOLATE_AT_OFFSET, INTERPOLATE_AT_SAMPLE};
use naga::{
    AddressSpace, BinaryOperator, Binding, Block, BuiltIn, DerivativeAxis, DerivativeControl,
    Expression, Function, FunctionArgument, FunctionResult, GlobalVariable, Handle, Interpolation,
    Literal, MathFunction, Module, Sampling, Scalar, ShaderStage, Span, Statement,
    SwizzleComponent, Type, TypeInner, VectorSize,
};
use wcontainers_gpu::SystemValue;

type Result<T> = core::result::Result<T, ConvertError>;

/// Where an evaluation takes its input.
#[derive(Clone, Copy, PartialEq)]
enum Place {
    Centroid,
    Sample,
    Offset,
}

/// A reader's stand-in: its function, the place it evaluates at, and the value's type.
#[derive(Clone, Copy)]
struct StandIn {
    function: Handle<Function>,
    place: Place,
    ty: Handle<Type>,
}

/// An evaluated input: the reader's private copy, the entry point's argument it is stored from, and
/// the companions the entry point leaves for the evaluations (none where it is flat).
struct Input {
    global: Handle<GlobalVariable>,
    argument: usize,
    companions: Option<Companions>,
}

#[derive(Clone, Copy)]
struct Companions {
    /// The slopes of the value (times w where perspective-correct) along x and y, of its own type.
    x: Handle<GlobalVariable>,
    y: Handle<GlobalVariable>,
    /// w where the value was interpolated (none where it is not perspective-correct), and that place.
    w: Option<Handle<GlobalVariable>>,
    place: Handle<GlobalVariable>,
}

/// What the entry point leaves for every evaluation.
#[derive(Default)]
struct Shared {
    /// w's slopes (the position's w, one over the clip w), along x and y.
    w_slope: Option<Handle<GlobalVariable>>,
    centroid: Option<Handle<GlobalVariable>>,
    samples: Option<Handle<GlobalVariable>>,
}

/// What a malformed program names.
const NOT_LOADED: &str = "a value not loaded";
const NOT_FLOATS: &str = "an interpolated input not of floats";
const UNSIZED: &str = "an input array of no size";
const NO_PROGRAM: &str = "an entry point calling no program";
const NO_VALUE: &str = "a call without its value";
const NOT_FOUND: &str = "an input not found";
const NO_CENTROID: &str = "a centroid";
const NO_SAMPLES: &str = "a sample's place";
const NO_PLACE: &str = "a call's place";

fn malformed(what: &str) -> ConvertError {
    ConvertError::Malformed(format!(
        "{what} an evaluation elsewhere in the pixel cannot take"
    ))
}

/// Replaces every evaluation of an input elsewhere in the pixel in `module`, drawn into targets of
/// `samples` samples. Gives the system values it added to the entry point to compute them, which the
/// program's reflection leaves out.
pub(super) fn inputs(module: &mut Module, samples: u8) -> Result<Vec<SystemValue>> {
    let stand_ins = stand_ins(module);
    if stand_ins.is_empty() {
        return Ok(Vec::new());
    }
    if module.entry_points[0].stage != ShaderStage::Fragment {
        return Err(ConvertError::Unsupported(crate::refusal::INTERPOLATE_AT));
    }
    let stored = stored_inputs(&module.entry_points[0].function);
    // Each evaluated input, with the places it is evaluated at and whether it is read otherwise.
    let mut evaluated: Vec<(Handle<GlobalVariable>, usize, bool)> = Vec::new();
    let mut read_plainly = Vec::new();
    for f in functions(module) {
        let mut at_samples = Vec::new();
        for (value, place) in calls(f, &stand_ins) {
            let g = evaluated_global(f, value)?;
            let &(_, argument) = stored
                .iter()
                .find(|(s, _)| *s == g)
                .ok_or(ConvertError::Unsupported(crate::refusal::INTERPOLATE_AT))?;
            match evaluated.iter_mut().find(|e| e.0 == g) {
                Some(e) => e.2 &= place == Place::Sample,
                None => evaluated.push((g, argument, place == Place::Sample)),
            }
            if place == Place::Sample {
                at_samples.push(value);
            }
        }
        for (h, e) in f.expressions.iter() {
            if let Expression::Load { pointer } = *e
                && !at_samples.contains(&h)
                && let Some(g) = global_of(f, pointer)
            {
                read_plainly.push(g);
            }
        }
    }
    let ep = &mut module.entry_points[0].function;
    if per_sample(ep) {
        for &(g, argument, only_at_samples) in &evaluated {
            if only_at_samples && !read_plainly.contains(&g) {
                interpolate_at_sample(ep, argument);
            }
        }
    }

    let mut added = Vec::new();
    let mut shared = Shared::default();
    let inputs = declare(module, &evaluated, &mut shared)?;
    let mut ep = core::mem::take(&mut module.entry_points[0].function);
    let done = prologue(
        module,
        &mut ep,
        &inputs,
        &stand_ins,
        samples,
        &mut shared,
        &mut added,
    );
    module.entry_points[0].function = ep;
    done?;
    for s in &stand_ins {
        module.functions[s.function] = evaluation(module, s);
    }
    let callers: Vec<_> = module
        .functions
        .iter()
        .map(|(h, _)| h)
        .filter(|h| !stand_ins.iter().any(|s| s.function == *h))
        .collect();
    for h in callers {
        let mut f = core::mem::take(&mut module.functions[h]);
        let done = replace(module, &mut f, &stand_ins, &inputs, &shared);
        module.functions[h] = f;
        done?;
    }
    let mut f = core::mem::take(&mut module.entry_points[0].function);
    let done = replace(module, &mut f, &stand_ins, &inputs, &shared);
    module.entry_points[0].function = f;
    done?;
    Ok(added)
}

fn stand_ins(module: &Module) -> Vec<StandIn> {
    module
        .functions
        .iter()
        .filter_map(|(function, f)| {
            let place = match f.name.as_deref()? {
                INTERPOLATE_AT_CENTROID => Place::Centroid,
                INTERPOLATE_AT_SAMPLE => Place::Sample,
                INTERPOLATE_AT_OFFSET => Place::Offset,
                _ => return None,
            };
            Some(StandIn {
                function,
                place,
                ty: f.result.as_ref()?.ty,
            })
        })
        .collect()
}

/// The functions that may call a stand-in: every other function and the entry point.
fn functions(module: &Module) -> impl Iterator<Item = &Function> {
    module
        .functions
        .iter()
        .filter(|(_, f)| !is_stand_in(f))
        .map(|(_, f)| f)
        .chain(core::iter::once(&module.entry_points[0].function))
}

fn is_stand_in(f: &Function) -> bool {
    matches!(
        f.name.as_deref(),
        Some(INTERPOLATE_AT_CENTROID | INTERPOLATE_AT_SAMPLE | INTERPOLATE_AT_OFFSET)
    )
}

/// The reader's private copies of the entry point's located inputs, and the argument each is stored
/// from.
fn stored_inputs(ep: &Function) -> Vec<(Handle<GlobalVariable>, usize)> {
    ep.body
        .iter()
        .filter_map(|s| match *s {
            Statement::Store { pointer, value } => {
                match (&ep.expressions[pointer], &ep.expressions[value]) {
                    (&Expression::GlobalVariable(g), &Expression::FunctionArgument(i))
                        if matches!(
                            ep.arguments.get(i as usize)?.binding,
                            Some(Binding::Location { .. })
                        ) =>
                    {
                        Some((g, i as usize))
                    }
                    _ => None,
                }
            }
            _ => None,
        })
        .collect()
}

/// The value argument and place of every call of a stand-in in `f`.
fn calls(f: &Function, stand_ins: &[StandIn]) -> Vec<(Handle<Expression>, Place)> {
    let mut found = Vec::new();
    let mut stack = alloc::vec![&f.body];
    while let Some(block) = stack.pop() {
        for s in block.iter() {
            match s {
                Statement::Call {
                    function,
                    arguments,
                    ..
                } => {
                    if let (Some(s), Some(&value)) = (
                        stand_ins.iter().find(|s| s.function == *function),
                        arguments.first(),
                    ) {
                        found.push((value, s.place));
                    }
                }
                Statement::Block(b) => stack.push(b),
                Statement::If { accept, reject, .. } => stack.extend([accept, reject]),
                Statement::Loop {
                    body, continuing, ..
                } => stack.extend([body, continuing]),
                Statement::Switch { cases, .. } => stack.extend(cases.iter().map(|c| &c.body)),
                _ => {}
            }
        }
    }
    found
}

/// The input an evaluation's value is loaded from.
fn evaluated_global(f: &Function, value: Handle<Expression>) -> Result<Handle<GlobalVariable>> {
    match f.expressions[value] {
        Expression::Load { pointer } => {
            global_of(f, pointer).ok_or(ConvertError::Unsupported(crate::refusal::INTERPOLATE_AT))
        }
        _ => Err(malformed(NOT_LOADED)),
    }
}

/// Whether the program runs once per sample.
fn per_sample(ep: &Function) -> bool {
    ep.arguments.iter().any(|a| {
        matches!(
            a.binding,
            Some(
                Binding::BuiltIn(BuiltIn::SampleIndex)
                    | Binding::Location {
                        sampling: Some(Sampling::Sample),
                        ..
                    }
            )
        )
    })
}

/// Interpolates the input of argument `argument` at the sample being run, where it is read only through
/// evaluations at samples and shares its register with nothing, so that an evaluation at that sample is
/// the interpolated value itself, as the hardware gives it.
fn interpolate_at_sample(ep: &mut Function, argument: usize) {
    let register = |a: &FunctionArgument| match a.binding {
        Some(Binding::Location { location, .. }) => Some(decode(location).0),
        _ => None,
    };
    let own = register(&ep.arguments[argument]);
    let shared = ep
        .arguments
        .iter()
        .filter(|a| own.is_some() && register(a) == own)
        .count()
        > 1;
    if let Some(Binding::Location {
        interpolation,
        sampling,
        ..
    }) = &mut ep.arguments[argument].binding
        && !shared
        && !matches!(interpolation, Some(Interpolation::Flat))
        && !matches!(sampling, Some(Sampling::Centroid))
    {
        interpolation.get_or_insert(Interpolation::Perspective);
        *sampling = Some(Sampling::Sample);
    }
}

/// How the input of argument `argument` is interpolated: its interpolation (the reader leaves an
/// array's unset: perspective then) and sampling.
fn interpolation(ep: &Function, argument: usize) -> (Interpolation, Option<Sampling>) {
    match ep.arguments[argument].binding {
        Some(Binding::Location {
            interpolation,
            sampling,
            ..
        }) => (
            interpolation.unwrap_or(Interpolation::Perspective),
            sampling,
        ),
        _ => (Interpolation::Perspective, None),
    }
}

fn float(module: &Module, ty: Handle<Type>) -> bool {
    let scalar = match module.types[ty].inner {
        TypeInner::Array { base, .. } => return float(module, base),
        TypeInner::Scalar(s) | TypeInner::Vector { scalar: s, .. } => s,
        _ => return false,
    };
    scalar == Scalar::F32
}

fn private(
    module: &mut Module,
    name: alloc::string::String,
    ty: Handle<Type>,
) -> Handle<GlobalVariable> {
    module.global_variables.append(
        GlobalVariable {
            name: Some(name),
            space: AddressSpace::Private,
            binding: None,
            ty,
            init: None,
            memory_decorations: Default::default(),
        },
        Span::UNDEFINED,
    )
}

fn f32_type(module: &mut Module) -> Handle<Type> {
    module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Scalar(Scalar::F32),
        },
        Span::UNDEFINED,
    )
}

/// Declares the companions of every evaluated input that is interpolated.
fn declare(
    module: &mut Module,
    evaluated: &[(Handle<GlobalVariable>, usize, bool)],
    shared: &mut Shared,
) -> Result<Vec<Input>> {
    let vec2 = vector(module, VectorSize::Bi as u8, Scalar::F32);
    let f32_ty = f32_type(module);
    let mut inputs = Vec::new();
    for (i, &(global, argument, _)) in evaluated.iter().enumerate() {
        let ty = module.global_variables[global].ty;
        let (modes, _) = interpolation(&module.entry_points[0].function, argument);
        let companions = match modes {
            Interpolation::Flat => None,
            _ if !float(module, ty) => {
                return Err(malformed(NOT_FLOATS));
            }
            _ => {
                let perspective = modes == Interpolation::Perspective;
                if perspective && shared.w_slope.is_none() {
                    shared.w_slope = Some(private(module, W_SLOPE.into(), vec2));
                }
                Some(Companions {
                    x: private(module, format!("{SLOPE_X}{i}"), ty),
                    y: private(module, format!("{SLOPE_Y}{i}"), ty),
                    w: perspective.then(|| private(module, format!("{W_AT}{i}"), f32_ty)),
                    place: private(module, format!("{PLACE}{i}"), vec2),
                })
            }
        };
        inputs.push(Input {
            global,
            argument,
            companions,
        });
    }
    Ok(inputs)
}

/// The table of sample positions of the targets' sample count, in pixels from the centre.
fn sample_positions(
    module: &mut Module,
    shared: &mut Shared,
    samples: u8,
) -> Result<Handle<GlobalVariable>> {
    if let Some(g) = shared.samples {
        return Ok(g);
    }
    let positions: &[(f32, f32)] = match samples {
        1 => &[(0.0, 0.0)],
        n if usize::from(n) == FOUR_SAMPLES.len() => &FOUR_SAMPLES,
        _ => return Err(ConvertError::Unsupported(crate::refusal::SAMPLE_COUNT)),
    };
    let vec2 = vector(module, VectorSize::Bi as u8, Scalar::F32);
    let array = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Array {
                base: vec2,
                size: naga::ArraySize::Constant(
                    core::num::NonZeroU32::new(positions.len() as u32).expect("positions"),
                ),
                stride: core::mem::size_of::<[f32; 2]>() as u32,
            },
        },
        Span::UNDEFINED,
    );
    let mut components = Vec::new();
    for &(x, y) in positions {
        let xy = [x, y]
            .map(|v| {
                module.global_expressions.append(
                    Expression::Literal(Literal::F32(v / SAMPLE_POSITION_STEPS)),
                    Span::UNDEFINED,
                )
            })
            .to_vec();
        components.push(module.global_expressions.append(
            Expression::Compose {
                ty: vec2,
                components: xy,
            },
            Span::UNDEFINED,
        ));
    }
    let init = module.global_expressions.append(
        Expression::Compose {
            ty: array,
            components,
        },
        Span::UNDEFINED,
    );
    let g = module.global_variables.append(
        GlobalVariable {
            name: Some(SAMPLE_POSITIONS.into()),
            space: AddressSpace::Private,
            binding: None,
            ty: array,
            init: Some(init),
            memory_decorations: Default::default(),
        },
        Span::UNDEFINED,
    );
    shared.samples = Some(g);
    Ok(g)
}

/// The entry point's argument for `builtin`, added where the program has none (and then listed in
/// `added`).
fn argument(
    f: &mut Function,
    body: &mut Block,
    builtin: BuiltIn,
    ty: Handle<Type>,
    added: &mut Vec<SystemValue>,
) -> Handle<Expression> {
    let same = |b: &Option<Binding>| match (b, builtin) {
        (Some(Binding::BuiltIn(BuiltIn::Position { .. })), BuiltIn::Position { .. }) => true,
        (Some(Binding::BuiltIn(b)), _) => *b == builtin,
        _ => false,
    };
    let index = match f.arguments.iter().position(|a| same(&a.binding)) {
        Some(i) => i,
        None => {
            f.arguments.push(FunctionArgument {
                name: None,
                ty,
                binding: Some(Binding::BuiltIn(builtin)),
            });
            added.push(match builtin {
                BuiltIn::Position { .. } => SystemValue::Position,
                BuiltIn::SampleMask => SystemValue::Coverage,
                _ => SystemValue::SampleIndex,
            });
            f.arguments.len() - 1
        }
    };
    push(f, body, Expression::FunctionArgument(index as u32))
}

fn binary(
    f: &mut Function,
    body: &mut Block,
    op: BinaryOperator,
    left: Handle<Expression>,
    right: Handle<Expression>,
) -> Handle<Expression> {
    push(f, body, Expression::Binary { op, left, right })
}

fn math(
    f: &mut Function,
    body: &mut Block,
    fun: MathFunction,
    arg: Handle<Expression>,
    arg1: Option<Handle<Expression>>,
) -> Handle<Expression> {
    push(
        f,
        body,
        Expression::Math {
            fun,
            arg,
            arg1,
            arg2: None,
            arg3: None,
        },
    )
}

fn load_global(
    f: &mut Function,
    body: &mut Block,
    g: Handle<GlobalVariable>,
) -> Handle<Expression> {
    let pointer = push(f, body, Expression::GlobalVariable(g));
    push(f, body, Expression::Load { pointer })
}

fn store(body: &mut Block, pointer: Handle<Expression>, value: Handle<Expression>) {
    body.push(Statement::Store { pointer, value }, Span::UNDEFINED);
}

fn slopes(f: &mut Function, body: &mut Block, v: Handle<Expression>) -> [Handle<Expression>; 2] {
    [DerivativeAxis::X, DerivativeAxis::Y].map(|axis| {
        push(
            f,
            body,
            Expression::Derivative {
                axis,
                ctrl: DerivativeControl::None,
                expr: v,
            },
        )
    })
}

/// Where the position was taken, from the pixel's centre, its w (one over the clip w) and w's
/// slopes along x and y.
#[derive(Clone, Copy)]
struct Taken {
    taken: Handle<Expression>,
    w: Handle<Expression>,
    w_slopes: Handle<Expression>,
}

/// The position's `Taken`, its w's slopes stored into `w_slope`.
fn position_taken(
    ep: &mut Function,
    body: &mut Block,
    w_slope: Handle<GlobalVariable>,
    [vec2, vec4]: [Handle<Type>; 2],
    added: &mut Vec<SystemValue>,
) -> Taken {
    let position = argument(
        ep,
        body,
        BuiltIn::Position { invariant: false },
        vec4,
        added,
    );
    let xy = push(
        ep,
        body,
        Expression::Swizzle {
            size: VectorSize::Bi,
            vector: position,
            pattern: [
                SwizzleComponent::X,
                SwizzleComponent::Y,
                SwizzleComponent::X,
                SwizzleComponent::X,
            ],
        },
    );
    let floor = math(ep, body, MathFunction::Floor, xy, None);
    let half = push(ep, body, Expression::Literal(Literal::F32(PIXEL_CENTRE)));
    let half = push(
        ep,
        body,
        Expression::Splat {
            size: VectorSize::Bi,
            value: half,
        },
    );
    let centre = binary(ep, body, BinaryOperator::Add, floor, half);
    let taken = binary(ep, body, BinaryOperator::Subtract, xy, centre);
    let w = push(
        ep,
        body,
        Expression::AccessIndex {
            base: position,
            index: VectorSize::Quad as u32 - 1,
        },
    );
    let [wx, wy] = slopes(ep, body, w);
    let w_slopes = push(
        ep,
        body,
        Expression::Compose {
            ty: vec2,
            components: alloc::vec![wx, wy],
        },
    );
    let pointer = push(ep, body, Expression::GlobalVariable(w_slope));
    store(body, pointer, w_slopes);
    Taken { taken, w, w_slopes }
}

/// In the entry point `ep`, once the inputs are stored and before the program runs: the slopes and
/// places the evaluations read.
fn prologue(
    module: &mut Module,
    ep: &mut Function,
    inputs: &[Input],
    stand_ins: &[StandIn],
    samples: u8,
    shared: &mut Shared,
    added: &mut Vec<SystemValue>,
) -> Result<()> {
    let vec2 = vector(module, VectorSize::Bi as u8, Scalar::F32);
    let vec4 = vector(module, VectorSize::Quad as u8, Scalar::F32);
    let u32_ty = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Scalar(Scalar::U32),
        },
        Span::UNDEFINED,
    );
    let mut body = Block::new();
    let zero = push(ep, &mut body, Expression::ZeroValue(vec2));
    if stand_ins.iter().any(|s| s.place == Place::Sample) {
        sample_positions(module, shared, samples)?;
    }
    let taken = shared
        .w_slope
        .map(|g| position_taken(ep, &mut body, g, [vec2, vec4], added));

    // Direct3D's centroid: the centre where every sample is covered, else the first covered one.
    if stand_ins.iter().any(|s| s.place == Place::Centroid) {
        let centroid = if samples > 1 {
            let table = sample_positions(module, shared, samples)?;
            let mask = argument(ep, &mut body, BuiltIn::SampleMask, u32_ty, added);
            let all = push(
                ep,
                &mut body,
                Expression::Literal(Literal::U32(u32::MAX >> (u32::BITS - u32::from(samples)))),
            );
            let full = binary(ep, &mut body, BinaryOperator::Equal, mask, all);
            let first = math(ep, &mut body, MathFunction::FirstTrailingBit, mask, None);
            let first = sample_position(ep, &mut body, table, first);
            push(
                ep,
                &mut body,
                Expression::Select {
                    condition: full,
                    accept: zero,
                    reject: first,
                },
            )
        } else {
            zero
        };
        let g = private(module, CENTROID.into(), vec2);
        shared.centroid = Some(g);
        let pointer = push(ep, &mut body, Expression::GlobalVariable(g));
        store(&mut body, pointer, centroid);
    }

    for input in inputs {
        let Some(c) = input.companions else {
            continue;
        };
        let place = match interpolation(ep, input.argument).1 {
            Some(Sampling::Sample) => {
                let table = sample_positions(module, shared, samples)?;
                let index = argument(ep, &mut body, BuiltIn::SampleIndex, u32_ty, added);
                sample_position(ep, &mut body, table, index)
            }
            // The hardware's centroid is not Direct3D's rule everywhere, and the slopes between lanes
            // interpolated at different places are not the value's.
            Some(Sampling::Centroid) if samples > 1 => {
                return Err(ConvertError::Unsupported(
                    crate::refusal::EVALUATED_CENTROID_INPUT,
                ));
            }
            _ => zero,
        };
        let pointer = push(ep, &mut body, Expression::GlobalVariable(c.place));
        store(&mut body, pointer, place);
        let w_at = match (c.w, taken) {
            (Some(g), Some(Taken { taken, w, w_slopes })) => {
                let moved = binary(ep, &mut body, BinaryOperator::Subtract, place, taken);
                let along = math(ep, &mut body, MathFunction::Dot, w_slopes, Some(moved));
                let w_at = binary(ep, &mut body, BinaryOperator::Add, w, along);
                let pointer = push(ep, &mut body, Expression::GlobalVariable(g));
                store(&mut body, pointer, w_at);
                Some(w_at)
            }
            _ => None,
        };
        let rows = match module.types[module.global_variables[input.global].ty].inner {
            TypeInner::Array {
                size: naga::ArraySize::Constant(n),
                ..
            } => Some(n.get()),
            TypeInner::Array { .. } => return Err(malformed(UNSIZED)),
            _ => None,
        };
        let element =
            |ep: &mut Function, body: &mut Block, g: Handle<GlobalVariable>, row: Option<u32>| {
                let p = push(ep, body, Expression::GlobalVariable(g));
                match row {
                    Some(index) => push(ep, body, Expression::AccessIndex { base: p, index }),
                    None => p,
                }
            };
        let each: Vec<Option<u32>> = match rows {
            Some(n) => (0..n).map(Some).collect(),
            None => alloc::vec![None],
        };
        for row in each {
            let pointer = element(ep, &mut body, input.global, row);
            let mut v = push(ep, &mut body, Expression::Load { pointer });
            if let Some(w_at) = w_at {
                v = binary(ep, &mut body, BinaryOperator::Multiply, v, w_at);
            }
            let [x, y] = slopes(ep, &mut body, v);
            let px = element(ep, &mut body, c.x, row);
            store(&mut body, px, x);
            let py = element(ep, &mut body, c.y, row);
            store(&mut body, py, y);
        }
    }

    let at = ep
        .body
        .iter()
        .position(|s| matches!(s, Statement::Call { .. }))
        .ok_or_else(|| malformed(NO_PROGRAM))?;
    ep.body.splice(at..at, body);
    Ok(())
}

fn sample_position(
    f: &mut Function,
    body: &mut Block,
    table: Handle<GlobalVariable>,
    index: Handle<Expression>,
) -> Handle<Expression> {
    let p = push(f, body, Expression::GlobalVariable(table));
    let p = push(f, body, Expression::Access { base: p, index });
    push(f, body, Expression::Load { pointer: p })
}

/// The function a stand-in becomes. Of floats: the value, its slopes along x and y, w where it was
/// interpolated and w's slopes (1 and zero where it is not perspective-correct), that place and the
/// place asked for, giving the value there (the value itself at its own place, unrounded by the w
/// division). Of other values (flat): the value.
fn evaluation(module: &mut Module, s: &StandIn) -> Function {
    let ty = s.ty;
    let name = module.functions[s.function].name.clone();
    let mut f = Function {
        name,
        result: Some(FunctionResult { ty, binding: None }),
        ..Default::default()
    };
    let parameter = |f: &mut Function, ty| {
        f.arguments.push(FunctionArgument {
            name: None,
            ty,
            binding: None,
        });
        f.expressions.append(
            Expression::FunctionArgument(f.arguments.len() as u32 - 1),
            Span::UNDEFINED,
        )
    };
    let v = parameter(&mut f, ty);
    let mut body = Block::new();
    if !float(module, ty) {
        body.push(Statement::Return { value: Some(v) }, Span::UNDEFINED);
        f.body = body;
        return f;
    }
    let vec2 = vector(module, VectorSize::Bi as u8, Scalar::F32);
    let f32_ty = f32_type(module);
    let sx = parameter(&mut f, ty);
    let sy = parameter(&mut f, ty);
    let w = parameter(&mut f, f32_ty);
    let w_slopes = parameter(&mut f, vec2);
    let place = parameter(&mut f, vec2);
    let target = parameter(&mut f, vec2);
    let b = &mut body;
    let d = binary(&mut f, b, BinaryOperator::Subtract, target, place);
    let dx = push(&mut f, b, Expression::AccessIndex { base: d, index: 0 });
    let dy = push(&mut f, b, Expression::AccessIndex { base: d, index: 1 });
    let along_x = binary(&mut f, b, BinaryOperator::Multiply, sx, dx);
    let along_y = binary(&mut f, b, BinaryOperator::Multiply, sy, dy);
    let along = binary(&mut f, b, BinaryOperator::Add, along_x, along_y);
    let w_along = math(&mut f, b, MathFunction::Dot, w_slopes, Some(d));
    let w_there = binary(&mut f, b, BinaryOperator::Add, w, w_along);
    let scaled = binary(&mut f, b, BinaryOperator::Multiply, v, w);
    let there = binary(&mut f, b, BinaryOperator::Add, scaled, along);
    let there = binary(&mut f, b, BinaryOperator::Divide, there, w_there);
    let zero = push(&mut f, b, Expression::ZeroValue(vec2));
    let same = binary(&mut f, b, BinaryOperator::Equal, d, zero);
    let same = push(
        &mut f,
        b,
        Expression::Relational {
            fun: naga::RelationalFunction::All,
            argument: same,
        },
    );
    let value = push(
        &mut f,
        b,
        Expression::Select {
            condition: same,
            accept: v,
            reject: there,
        },
    );
    body.push(Statement::Return { value: Some(value) }, Span::UNDEFINED);
    f.body = body;
    f
}

/// `pointer`, a place in `from` reached through accesses, as the same place in `to`.
fn rebased(
    f: &mut Function,
    body: &mut Block,
    pointer: Handle<Expression>,
    to: Handle<GlobalVariable>,
) -> Handle<Expression> {
    match f.expressions[pointer] {
        Expression::Access { base, index } => {
            let base = rebased(f, body, base, to);
            push(f, body, Expression::Access { base, index })
        }
        Expression::AccessIndex { base, index } => {
            let base = rebased(f, body, base, to);
            push(f, body, Expression::AccessIndex { base, index })
        }
        _ => push(f, body, Expression::GlobalVariable(to)),
    }
}

/// Gives each call of a stand-in in `f`, a function taken out of `module`, the arguments its
/// evaluation takes.
fn replace(
    module: &mut Module,
    f: &mut Function,
    stand_ins: &[StandIn],
    inputs: &[Input],
    shared: &Shared,
) -> Result<()> {
    let vec2 = vector(module, VectorSize::Bi as u8, Scalar::F32);
    let mut body = core::mem::take(&mut f.body);
    let mut failed = None;
    each_block(&mut body, &mut |block: &mut Block| {
        let mut i = 0;
        while i < block.len() && failed.is_none() {
            let Statement::Call {
                function,
                ref arguments,
                ..
            } = block[i]
            else {
                i += 1;
                continue;
            };
            let Some(&s) = stand_ins.iter().find(|s| s.function == function) else {
                i += 1;
                continue;
            };
            let arguments = arguments.clone();
            let mut made = Block::new();
            match evaluation_arguments(module, f, &mut made, s, &arguments, inputs, shared, vec2) {
                Ok(new) => {
                    if let Statement::Call { arguments, .. } = &mut block[i] {
                        *arguments = new;
                    }
                    let n = made.len();
                    block.splice(i..i, made);
                    i += n + 1;
                }
                Err(e) => failed = Some(e),
            }
        }
    });
    f.body = body;
    failed.map_or(Ok(()), Err)
}

/// The arguments of `s`'s evaluation for a call of it with `arguments`, made in `body`.
#[allow(clippy::too_many_arguments)]
fn evaluation_arguments(
    module: &Module,
    f: &mut Function,
    body: &mut Block,
    s: StandIn,
    arguments: &[Handle<Expression>],
    inputs: &[Input],
    shared: &Shared,
    vec2: Handle<Type>,
) -> Result<Vec<Handle<Expression>>> {
    let &[value, ref place @ ..] = arguments else {
        return Err(malformed(NO_VALUE));
    };
    if !float(module, s.ty) {
        return Ok(alloc::vec![value]);
    }
    let Expression::Load { pointer } = f.expressions[value] else {
        return Err(malformed(NOT_LOADED));
    };
    let g =
        global_of(f, pointer).ok_or(ConvertError::Unsupported(crate::refusal::INTERPOLATE_AT))?;
    let input = inputs
        .iter()
        .find(|i| i.global == g)
        .ok_or_else(|| malformed(NOT_FOUND))?;
    let one = push(f, body, Expression::Literal(Literal::F32(1.0)));
    let zero = push(f, body, Expression::ZeroValue(vec2));
    let Some(c) = input.companions else {
        // Flat: the value itself.
        let none = push(f, body, Expression::ZeroValue(s.ty));
        return Ok(alloc::vec![value, none, none, one, zero, zero, zero]);
    };
    let x = rebased(f, body, pointer, c.x);
    let x = push(f, body, Expression::Load { pointer: x });
    let y = rebased(f, body, pointer, c.y);
    let y = push(f, body, Expression::Load { pointer: y });
    let (w, w_slopes) = match (c.w, shared.w_slope) {
        (Some(w), Some(slopes)) => (load_global(f, body, w), load_global(f, body, slopes)),
        _ => (one, zero),
    };
    let at = load_global(f, body, c.place);
    let target = match (s.place, place) {
        (Place::Centroid, []) => load_global(
            f,
            body,
            shared.centroid.ok_or_else(|| malformed(NO_CENTROID))?,
        ),
        (Place::Sample, &[index]) => {
            let table = shared.samples.ok_or_else(|| malformed(NO_SAMPLES))?;
            sample_position(f, body, table, index)
        }
        (Place::Offset, &[offset]) => offset,
        _ => return Err(malformed(NO_PLACE)),
    };
    Ok(alloc::vec![value, x, y, w, w_slopes, at, target])
}
