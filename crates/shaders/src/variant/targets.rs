//! Color targets written in the program (`VariantKey::targets_in_shader`): an output of another type
//! than its target's format is written as the format's type with its bits kept, as Direct3D writes it,
//! and widened with zeros to the format's components, which WebGPU and GLSL ES require of an output.
//! Into a normalised format, which the front stores as the integer format of the same bits, the
//! output is encoded as Direct3D converts it (`NormAsInteger`). A target combined with its old value
//! (`BlendInShader`, `LogicOpInShader`) reads that value from a copy of the target and blends or applies
//! the logic operation by the helper block's codes (`combine.wgsl`), then writes it by the write mask.

use naga::ScalarKind;
use wcontainers_gpu::{
    FORMAT_FACTS, FormatKind, HELPER_SET, HelperValue, Reflection, ScalarKind as ValueKind,
    ShaderStage, SystemValue, TARGET_COPY_BINDING, TargetInShader,
};

use super::pull::{Kind, layout};
use super::texel::vertex_format;
use super::*;
use crate::build::{ALPHA, VEC4};

/// A normalised format's kind and each component's bits, which an output is encoded by.
type Encoding = (Kind, Vec<u32>);

/// The scalar target `t` takes from the program and, for a normalised format, how the output is
/// encoded into the integers of the same bits. A normalised format no integer format holds is refused
/// by name.
fn target_scalar(t: &TargetInShader) -> Result<(Scalar, Option<Encoding>)> {
    let format = t.format;
    match FORMAT_FACTS[format as usize].kind {
        FormatKind::Uint => Ok((Scalar::U32, None)),
        FormatKind::Sint => Ok((Scalar::I32, None)),
        FormatKind::Float => match vertex_format(format).map(layout) {
            Some((Kind::Float, _, _)) => Ok((Scalar::F32, None)),
            _ => {
                let (kind, bits) = super::decoded::decoding(format)
                    .map_err(|_| ConvertError::Unsupported(crate::refusal::NORM_AS_INTEGER))?;
                let to = match kind {
                    Kind::Snorm => Scalar::I32,
                    _ => Scalar::U32,
                };
                Ok((to, Some((kind, bits))))
            }
        },
        _ => Err(ConvertError::Unsupported(crate::refusal::TARGETS_IN_SHADER)),
    }
}

/// What target `t`'s output of type `ty` becomes: its scalar, its component count and the output's own
/// count; `None` where it stays as it is. An encoded output must be a float one.
fn retyped(
    module: &Module,
    ty: Handle<Type>,
    t: &TargetInShader,
) -> Result<Option<(Scalar, u32, u32)>> {
    let (to, encoding) = target_scalar(t)?;
    let (own_count, own_scalar) = match module.types[ty].inner {
        TypeInner::Scalar(s) => (1, s),
        TypeInner::Vector { size, scalar } => (size as u32, scalar),
        _ => return Err(ConvertError::Unsupported(crate::refusal::TARGETS_IN_SHADER)),
    };
    if encoding.is_some() && own_scalar != Scalar::F32 {
        return Err(ConvertError::Unsupported(crate::refusal::TARGETS_IN_SHADER));
    }
    // Blending takes float values, logic operations integers.
    let float = own_scalar.kind == ScalarKind::Float;
    if t.combine && encoding.is_none() && float != (to.kind == ScalarKind::Float) {
        return Err(ConvertError::Unsupported(crate::refusal::TARGETS_IN_SHADER));
    }
    let format_count = u32::BITS - FORMAT_FACTS[t.format as usize].components.leading_zeros();
    let count = own_count.max(format_count);
    let changed = t.combine || own_scalar != to || own_count != count;
    Ok(changed.then_some((to, count, own_count)))
}

fn at_target(b: &Binding, target: u8) -> bool {
    matches!(b, Binding::Location { location, .. } if *location == u32::from(target))
}

/// Whether any of `targets` changes the entry point's outputs (and so needs it wrapped).
pub(super) fn needed(module: &Module, targets: &[TargetInShader]) -> Result<bool> {
    let Some(result) = &module.entry_points[0].function.result else {
        return Ok(false);
    };
    let outputs: Vec<(Binding, Handle<Type>)> =
        match (&result.binding, &module.types[result.ty].inner) {
            (Some(b), _) => alloc::vec![(b.clone(), result.ty)],
            (None, TypeInner::Struct { members, .. }) => members
                .iter()
                .filter_map(|m| Some((m.binding.clone()?, m.ty)))
                .collect(),
            _ => Vec::new(),
        };
    let mut needed = false;
    for t in targets {
        // Refused by name even where the program writes no such target.
        target_scalar(t)?;
        for (b, ty) in &outputs {
            needed |= at_target(b, t.target) && retyped(module, *ty, t)?.is_some();
        }
    }
    Ok(needed)
}

pub(super) fn targets_in_shader(
    module: &mut Module,
    w: &mut Wrapped,
    targets: &[TargetInShader],
    samples: u8,
    reflection: &mut Reflection,
) -> Result<()> {
    for t in targets {
        let Some(at) = w.output(|b| at_target(b, t.target)) else {
            continue;
        };
        let Some((to, count, own_count)) = retyped(module, w.outputs[at].ty, t)? else {
            continue;
        };
        let (_, encoding) = target_scalar(t)?;
        if t.combine {
            combine(
                module,
                w,
                at,
                t,
                samples,
                reflection,
                (to, count, own_count),
            )?;
            set_kind(reflection, t, to);
            continue;
        }
        let (f, body) = (&mut w.f, &mut w.body);
        let mut value = w.outputs[at].value;
        if let Some((kind, bits)) = &encoding {
            value = encoded(
                module,
                f,
                body,
                value,
                (*kind, bits),
                (to, count, own_count),
            );
        } else if module.types[w.outputs[at].ty].inner.scalar() != Some(to) {
            value = push(
                f,
                body,
                Expression::As {
                    expr: value,
                    kind: to.kind,
                    convert: None,
                },
            );
        }
        let ty = if count == 1 {
            scalar(module, to)
        } else {
            vector(module, count as u8, to)
        };
        if count != own_count && encoding.is_none() {
            let mut components: Vec<_> = match own_count {
                1 => alloc::vec![value],
                n => (0..n).map(|i| index(f, body, value, i)).collect(),
            };
            let zero_ty = scalar(module, to);
            let zero = f
                .expressions
                .append(Expression::ZeroValue(zero_ty), Span::UNDEFINED);
            components.resize(count as usize, zero);
            value = push(f, body, Expression::Compose { ty, components });
        }
        w.outputs[at].ty = ty;
        w.outputs[at].value = value;
        w.changed = true;
        set_kind(reflection, t, to);
    }
    Ok(())
}

/// Reflects target `t`'s output as written in `to`.
fn set_kind(reflection: &mut Reflection, t: &TargetInShader, to: Scalar) {
    let kind = match to.kind {
        ScalarKind::Uint => ValueKind::Uint,
        ScalarKind::Sint => ValueKind::Sint,
        _ => ValueKind::Float,
    };
    for o in reflection
        .outputs
        .iter_mut()
        .filter(|o| o.system == Some(SystemValue::Target(t.target)))
    {
        o.scalar = kind;
    }
}

/// The float output `value` of `own_count` components as `count` components of `to`, each the
/// format's component encoded from it as Direct3D converts (signed ones sign-extended from their
/// bits), zero past the format's or the output's components.
fn encoded(
    module: &mut Module,
    f: &mut Function,
    body: &mut Block,
    value: Handle<Expression>,
    (kind, bits): (Kind, &[u32]),
    (to, count, own_count): (Scalar, u32, u32),
) -> Handle<Expression> {
    let zero_ty = scalar(module, to);
    let mut components = Vec::new();
    for i in 0..count {
        let component = match bits.get(i as usize) {
            Some(&b) if i < own_count => {
                let v = if own_count == 1 {
                    value
                } else {
                    index(f, body, value, i)
                };
                let raw = super::texel::encode(f, body, kind, v, b);
                if kind == Kind::Snorm {
                    let by = super::pull::u(f, u32::BITS - b);
                    let up = binary(f, body, naga::BinaryOperator::ShiftLeft, raw, by);
                    let up = super::pull::cast(f, body, up, ScalarKind::Sint, None);
                    binary(f, body, naga::BinaryOperator::ShiftRight, up, by)
                } else {
                    raw
                }
            }
            _ => f
                .expressions
                .append(Expression::ZeroValue(zero_ty), Span::UNDEFINED),
        };
        components.push(component);
    }
    if count == 1 {
        return components[0];
    }
    let ty = vector(module, count as u8, to);
    push(f, body, Expression::Compose { ty, components })
}

/// The name of the copy of a target the program reads its old value from, before its number.
const TARGET_COPY: &str = "target_copy_";

const LIBRARY: &str = include_str!("combine.wgsl");
const LIBRARY_FUNCTIONS: [&str; 3] = ["blended", "logic_operation", "write_masked"];

/// The prefixes `combine.wgsl` gives the codes of blend factors, blend operations and logic
/// operations, each followed by the interface's name for the code in upper snake case.
const FACTOR_PREFIX: &str = "FACTOR_";
const OPERATION_PREFIX: &str = "OP_";
const LOGIC_PREFIX: &str = "LOGIC_";

/// `combine.wgsl` with the codes it names declared ahead of it, as the interface numbers them.
fn library_source() -> String {
    use wcontainers_gpu::{BlendFactor as F, BlendOp as O, LogicOp as L};
    let factors = [
        F::Zero,
        F::One,
        F::Src,
        F::OneMinusSrc,
        F::SrcAlpha,
        F::OneMinusSrcAlpha,
        F::Dst,
        F::OneMinusDst,
        F::DstAlpha,
        F::OneMinusDstAlpha,
        F::SrcAlphaSaturated,
        F::Constant,
        F::OneMinusConstant,
        F::ConstantAlpha,
        F::OneMinusConstantAlpha,
        F::Src1,
        F::OneMinusSrc1,
        F::Src1Alpha,
        F::OneMinusSrc1Alpha,
    ];
    let operations = [O::Subtract, O::ReverseSubtract, O::Min, O::Max];
    let logic = [
        L::Clear,
        L::And,
        L::AndReverse,
        L::AndInverted,
        L::Noop,
        L::Xor,
        L::Or,
        L::Nor,
        L::Equivalent,
        L::Invert,
        L::OrReverse,
        L::CopyInverted,
        L::OrInverted,
        L::Nand,
        L::Set,
    ];
    let codes = (factors
        .iter()
        .map(|f| (FACTOR_PREFIX, format!("{f:?}"), *f as u32)))
    .chain(
        operations
            .iter()
            .map(|o| (OPERATION_PREFIX, format!("{o:?}"), *o as u32)),
    )
    .chain(
        logic
            .iter()
            .map(|l| (LOGIC_PREFIX, format!("{l:?}"), *l as u32)),
    );
    let mut text: String = codes
        .map(|(prefix, name, code)| {
            format!("const {prefix}{}: u32 = {code}u;\n", upper_snake(&name))
        })
        .collect();
    text.push_str(LIBRARY);
    text
}

/// A name in upper camel case (`OneMinusSrc1Alpha`) in upper snake case (`ONE_MINUS_SRC1_ALPHA`).
fn upper_snake(name: &str) -> String {
    const WORD_BREAK: char = '_';
    let mut out = String::new();
    for (i, c) in name.char_indices() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push(WORD_BREAK);
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}

/// The library's [`LIBRARY_FUNCTIONS`] imported into `module` after its own functions, with the
/// functions they call.
fn library_functions(module: &mut Module) -> Result<[Handle<Function>; LIBRARY_FUNCTIONS.len()]> {
    let parsed = naga::front::wgsl::parse_str(&library_source())
        .map_err(|e| ConvertError::Malformed(format!("the combine library does not parse: {e}")))?;
    let mut library = super::filter::Library::default();
    let mut imported = Vec::new();
    for name in LIBRARY_FUNCTIONS {
        let f = parsed
            .functions
            .iter()
            .find(|(_, f)| f.name.as_deref() == Some(name))
            .map(|(h, _)| h)
            .ok_or_else(|| ConvertError::Malformed(format!("the combine library has no {name}")))?;
        imported.push(library.import(module, &parsed, f)?);
    }
    // Callees come before their callers, so each call's new handle is known when its caller moves.
    let mut handles: Vec<Handle<Function>> = Vec::new();
    for mut g in library.ahead.into_inner() {
        for (_, e) in g.expressions.iter_mut() {
            if let Expression::CallResult(callee) = e {
                *callee = handles[callee.index()];
            }
        }
        each_block(&mut g.body, &mut |b: &mut Block| {
            for s in b.iter_mut() {
                if let Statement::Call { function, .. } = s {
                    *function = handles[function.index()];
                }
            }
        });
        handles.push(module.functions.append(g, Span::UNDEFINED));
    }
    let mut found = imported.iter().map(|h| handles[h.index()]);
    Ok(core::array::from_fn(|_| found.next().unwrap()))
}

fn call(
    f: &mut Function,
    body: &mut Block,
    function: Handle<Function>,
    arguments: Vec<Handle<Expression>>,
) -> Handle<Expression> {
    let result = f
        .expressions
        .append(Expression::CallResult(function), Span::UNDEFINED);
    body.push(
        Statement::Call {
            function,
            arguments,
            result: Some(result),
        },
        Span::UNDEFINED,
    );
    result
}

/// `value` of `count` components as four, the others zero.
fn widened(
    module: &mut Module,
    f: &mut Function,
    body: &mut Block,
    value: Handle<Expression>,
    count: u32,
    of: Scalar,
) -> Handle<Expression> {
    if count == VEC4 {
        return value;
    }
    let zero_ty = scalar(module, of);
    let zero = f
        .expressions
        .append(Expression::ZeroValue(zero_ty), Span::UNDEFINED);
    let mut components: Vec<_> = match count {
        1 => alloc::vec![value],
        n => (0..n).map(|i| index(f, body, value, i)).collect(),
    };
    components.resize(VEC4 as usize, zero);
    let ty = vector(module, VEC4 as u8, of);
    push(f, body, Expression::Compose { ty, components })
}

fn bitcast(
    f: &mut Function,
    body: &mut Block,
    value: Handle<Expression>,
    from: Scalar,
    kind: ScalarKind,
) -> Handle<Expression> {
    if from.kind == kind {
        return value;
    }
    push(
        f,
        body,
        Expression::As {
            expr: value,
            kind,
            convert: None,
        },
    )
}

fn u32_vector(
    module: &mut Module,
    f: &mut Function,
    body: &mut Block,
    words: [u32; VEC4 as usize],
) -> Handle<Expression> {
    let ty = vector(module, VEC4 as u8, Scalar::U32);
    let components = words
        .iter()
        .map(|&w| {
            f.expressions
                .append(Expression::Literal(naga::Literal::U32(w)), Span::UNDEFINED)
        })
        .collect();
    push(f, body, Expression::Compose { ty, components })
}

/// Target `t`'s output combined with the target's old value, read at the fragment's pixel from the
/// copy at [`TARGET_COPY_BINDING`] plus the target, by the codes of [`HelperValue::TargetCombine`]:
/// a float or normalised output blended (the normalised one's values clamped to its range, as
/// Direct3D clamps them, and encoded), a normalised or integer one's bits put through the logic
/// operation, then each channel the write mask leaves out kept as it was.
fn combine(
    module: &mut Module,
    w: &mut Wrapped,
    at: usize,
    t: &TargetInShader,
    samples: u8,
    reflection: &mut Reflection,
    (to, count, own_count): (Scalar, u32, u32),
) -> Result<()> {
    use naga::{MathFunction, VectorSize};
    use wcontainers_gpu::{ColorWrites, TargetCombineValue};
    match samples {
        0 => return Err(ConvertError::Unsupported(crate::refusal::SAMPLE_COUNT)),
        1 => {}
        _ => {
            return Err(ConvertError::Unsupported(
                crate::refusal::BLEND_IN_SHADER_MULTISAMPLED,
            ));
        }
    }
    let (_, encoding) = target_scalar(t)?;
    let bits: Vec<u32> = match (&encoding, vertex_format(t.format).map(layout)) {
        (Some((_, bits)), _) => bits.clone(),
        (None, Some((_, components, _))) => components.iter().map(|c| c.2).collect(),
        (None, None) => return Err(ConvertError::Unsupported(crate::refusal::TARGETS_IN_SHADER)),
    };
    let blends = encoding.is_some() || to.kind == ScalarKind::Float;
    let logic = to.kind != ScalarKind::Float;
    let vec4f = vector(module, VEC4 as u8, Scalar::F32);
    let vec3u = vector(module, ALPHA as u8, Scalar::U32);
    let image = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Image {
                dim: naga::ImageDimension::D2,
                arrayed: false,
                class: naga::ImageClass::Sampled {
                    kind: to.kind,
                    multi: false,
                },
            },
        },
        Span::UNDEFINED,
    );
    let copy = module.global_variables.append(
        naga::GlobalVariable {
            name: Some(format!("{TARGET_COPY}{}", t.target)),
            space: naga::AddressSpace::Handle,
            binding: Some(naga::ResourceBinding {
                group: HELPER_SET,
                binding: TARGET_COPY_BINDING + u32::from(t.target),
            }),
            ty: image,
            init: None,
            memory_decorations: Default::default(),
        },
        Span::UNDEFINED,
    );
    let row = vector(module, VEC4 as u8, Scalar::U32);
    let stride = module.types[row].inner.size(module.to_ctx());
    let rows = core::mem::size_of::<TargetCombineValue>() as u32 / stride;
    let rows = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Array {
                base: row,
                size: naga::ArraySize::Constant(core::num::NonZeroU32::new(rows).unwrap()),
                stride,
            },
        },
        Span::UNDEFINED,
    );
    let codes = helper(
        module,
        reflection,
        ShaderStage::Fragment,
        HelperValue::TargetCombine { target: t.target },
        rows,
    );
    let position = w.input(
        module,
        |b| matches!(b, Binding::BuiltIn(naga::BuiltIn::Position { .. })),
        Binding::BuiltIn(naga::BuiltIn::Position { invariant: false }),
        vec4f,
    );
    // Direct3D's second source of target 0 is the program's output at target 1.
    let second = match t.target {
        0 => w
            .output(|b| at_target(b, 1))
            .map(|o| (w.outputs[o].value, w.outputs[o].ty)),
        _ => None,
    };
    let [blended, logic_operation, write_masked] = library_functions(module)?;
    let own = w.outputs[at].ty;
    let own_scalar = module.types[own].inner.scalar().unwrap_or(to);
    let value = w.outputs[at].value;
    let (f, body) = (&mut w.f, &mut w.body);
    let word_of = |name: usize| (name / core::mem::size_of::<u32>()) as u32;
    let codes = load_helper(f, body, codes);
    let word = |f: &mut Function, body: &mut Block, at: u32| {
        let row = index(f, body, codes, at / VEC4);
        index(f, body, row, at % VEC4)
    };
    let xy = push(
        f,
        body,
        Expression::Swizzle {
            size: VectorSize::Bi,
            vector: position,
            pattern: [
                naga::SwizzleComponent::X,
                naga::SwizzleComponent::Y,
                naga::SwizzleComponent::X,
                naga::SwizzleComponent::X,
            ],
        },
    );
    let pixel = push(
        f,
        body,
        Expression::As {
            expr: xy,
            kind: ScalarKind::Uint,
            convert: Some(Scalar::U32.width),
        },
    );
    let copy = push(f, body, Expression::GlobalVariable(copy));
    let level = f
        .expressions
        .append(Expression::Literal(naga::Literal::U32(0)), Span::UNDEFINED);
    let old = push(
        f,
        body,
        Expression::ImageLoad {
            image: copy,
            coordinate: pixel,
            array_index: None,
            sample: None,
            level: Some(level),
        },
    );
    let old_bits = bitcast(f, body, old, to, ScalarKind::Uint);
    let mut new_bits = if blends {
        let range = encoding.as_ref().map(|(kind, _)| match kind {
            Kind::Snorm => -1.0,
            _ => 0.0,
        });
        let clamped = |f: &mut Function, body: &mut Block, v| match range {
            None => v,
            Some(low) => {
                let [low, high] = [low, 1.0].map(|x| {
                    let x = f32_lit(f, x);
                    push(
                        f,
                        body,
                        Expression::Splat {
                            size: VectorSize::Quad,
                            value: x,
                        },
                    )
                });
                push(
                    f,
                    body,
                    Expression::Math {
                        fun: MathFunction::Clamp,
                        arg: v,
                        arg1: Some(low),
                        arg2: Some(high),
                        arg3: None,
                    },
                )
            }
        };
        let s = widened(module, f, body, value, own_count, Scalar::F32);
        let s = clamped(f, body, s);
        let s1 = match second {
            Some((v, ty)) if module.types[ty].inner.scalar() == Some(Scalar::F32) => {
                let n = match module.types[ty].inner {
                    TypeInner::Vector { size, .. } => size as u32,
                    _ => 1,
                };
                widened(module, f, body, v, n, Scalar::F32)
            }
            _ => f
                .expressions
                .append(Expression::ZeroValue(vec4f), Span::UNDEFINED),
        };
        let s1 = clamped(f, body, s1);
        let constant_row = index(
            f,
            body,
            codes,
            word_of(core::mem::offset_of!(TargetCombineValue, constant)) / VEC4,
        );
        let k = bitcast(f, body, constant_row, Scalar::U32, ScalarKind::Float);
        let k = clamped(f, body, k);
        let d = match &encoding {
            Some(_) => super::decoded::texel(f, body, old, t.format, vec4f)?,
            None => old,
        };
        let half = |f: &mut Function, body: &mut Block, first: u32| {
            let components = (first..first + ALPHA).map(|i| word(f, body, i)).collect();
            push(
                f,
                body,
                Expression::Compose {
                    ty: vec3u,
                    components,
                },
            )
        };
        let color = half(
            f,
            body,
            word_of(core::mem::offset_of!(TargetCombineValue, color)),
        );
        let alpha = half(
            f,
            body,
            word_of(core::mem::offset_of!(TargetCombineValue, alpha)),
        );
        let b = call(f, body, blended, alloc::vec![s, s1, d, k, color, alpha]);
        match &encoding {
            Some((kind, bits)) => {
                let b = clamped(f, body, b);
                let e = encoded(module, f, body, b, (*kind, bits), (to, VEC4, VEC4));
                bitcast(f, body, e, to, ScalarKind::Uint)
            }
            None => bitcast(f, body, b, Scalar::F32, ScalarKind::Uint),
        }
    } else {
        let v = bitcast(f, body, value, own_scalar, to.kind);
        let v = widened(module, f, body, v, own_count, to);
        bitcast(f, body, v, to, ScalarKind::Uint)
    };
    if logic {
        let past: [u32; VEC4 as usize] =
            core::array::from_fn(|i| u32::BITS - bits.get(i).copied().unwrap_or(u32::BITS));
        let past = u32_vector(module, f, body, past);
        let signed = f.expressions.append(
            Expression::Literal(naga::Literal::Bool(to.kind == ScalarKind::Sint)),
            Span::UNDEFINED,
        );
        let code = word(
            f,
            body,
            word_of(core::mem::offset_of!(TargetCombineValue, logic_op)),
        );
        new_bits = call(
            f,
            body,
            logic_operation,
            alloc::vec![code, new_bits, old_bits, past, signed],
        );
    }
    let channels = [
        ColorWrites::R,
        ColorWrites::G,
        ColorWrites::B,
        ColorWrites::A,
    ]
    .map(|c| u32::from(c.bits()));
    let channels = u32_vector(module, f, body, channels);
    let writes = word(
        f,
        body,
        word_of(core::mem::offset_of!(TargetCombineValue, writes)),
    );
    let written = call(
        f,
        body,
        write_masked,
        alloc::vec![new_bits, old_bits, writes, channels],
    );
    let written = bitcast(f, body, written, Scalar::U32, to.kind);
    let (ty, value) = match count {
        1 => (scalar(module, to), index(f, body, written, 0)),
        VEC4 => (vector(module, VEC4 as u8, to), written),
        n => {
            let components = (0..n).map(|i| index(f, body, written, i)).collect();
            let ty = vector(module, n as u8, to);
            (ty, push(f, body, Expression::Compose { ty, components }))
        }
    };
    w.outputs[at].ty = ty;
    w.outputs[at].value = value;
    w.changed = true;
    Ok(())
}
