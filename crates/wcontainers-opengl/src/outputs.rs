//! A fragment program's outputs fitted to the framebuffer's targets: the host requires each output
//! to hold every component of its target's format, of the target's kind, where OpenGL leaves the
//! components a program does not write, and the values of an output of another kind, undefined.

use crate::state::refused;

use naga::{
    Arena, Block, Expression, Handle, Literal, Module, Scalar, ScalarKind, Span, Statement,
    StructMember, Type, TypeInner, VectorSize,
};

/// The kind of the target at each location.
pub type Targets = Vec<(u32, ScalarKind)>;

/// The bytes of a scalar of every kind the outputs take.
const WIDTH: u8 = Scalar::F32.width;
/// The components of a target's widest format.
const WIDE: u32 = VectorSize::Quad as u32;

/// The value an output fills a component it does not write with: a vertex attribute's, (0, 0, 0,
/// 1).
fn filler(kind: ScalarKind, last: bool) -> Literal {
    match (kind, last) {
        (ScalarKind::Float, false) => Literal::F32(0.0),
        (ScalarKind::Float, true) => Literal::F32(1.0),
        (ScalarKind::Sint, false) => Literal::I32(0),
        (ScalarKind::Sint, true) => Literal::I32(1),
        (_, false) => Literal::U32(0),
        (_, true) => Literal::U32(1),
    }
}

/// The components and kind of a value of type `inner`.
fn shape(inner: &TypeInner) -> Option<(u32, ScalarKind)> {
    match *inner {
        TypeInner::Scalar(s) => Some((1, s.kind)),
        TypeInner::Vector { size, scalar } => Some((size as u32, scalar.kind)),
        _ => None,
    }
}

/// One output to fit: its member (none for a result that is the output itself), its shape, the
/// target's kind and the output's new type.
struct Fit {
    member: Option<u32>,
    components: u32,
    kind: ScalarKind,
    target: ScalarKind,
    ty: Handle<Type>,
}

/// Fits the first entry point's outputs to `targets`; outputs at no target are left as they are.
pub fn fit(module: &mut Module, targets: &Targets) {
    let Some(result) = module
        .entry_points
        .first()
        .and_then(|e| e.function.result.as_ref())
    else {
        return;
    };
    let target_at = |binding: &Option<naga::Binding>| match binding {
        Some(naga::Binding::Location {
            location,
            blend_src: None | Some(0),
            ..
        }) => targets.iter().find(|(l, _)| l == location).map(|&(_, k)| k),
        _ => None,
    };
    let result_ty = result.ty;
    let members = match &module.types[result_ty].inner {
        TypeInner::Struct { members, .. } => Some(members.clone()),
        _ => None,
    };
    let outputs: Vec<(Option<u32>, Handle<Type>, Option<ScalarKind>)> = match &members {
        Some(members) => members
            .iter()
            .enumerate()
            .map(|(i, m)| (Some(i as u32), m.ty, target_at(&m.binding)))
            .collect(),
        None => vec![(None, result_ty, target_at(&result.binding))],
    };
    let mut fits = Vec::new();
    for (member, ty, target) in outputs {
        let Some(target) = target else { continue };
        let Some((components, kind)) = shape(&module.types[ty].inner) else {
            continue;
        };
        if components == WIDE && kind == target {
            continue;
        }
        let ty = module.types.insert(
            Type {
                name: None,
                inner: TypeInner::Vector {
                    size: VectorSize::Quad,
                    scalar: Scalar {
                        kind: target,
                        width: WIDTH,
                    },
                },
            },
            Span::UNDEFINED,
        );
        fits.push(Fit {
            member,
            components,
            kind,
            target,
            ty,
        });
    }
    if fits.is_empty() {
        return;
    }
    let (new_ty, count) = match members {
        Some(mut members) => {
            for f in &fits {
                members[f.member.expect("a member") as usize].ty = f.ty;
            }
            let count = members.len() as u32;
            (restruct(module, result_ty, members), Some(count))
        }
        None => (fits[0].ty, None),
    };
    let function = &mut module.entry_points[0].function;
    if let Some(result) = function.result.as_mut() {
        result.ty = new_ty;
    }
    let body = std::mem::take(&mut function.body);
    let make = |value: Handle<Expression>, exprs: &mut Arena<Expression>| {
        // Literals first: they are never emitted.
        let fillers: Vec<Vec<Handle<Expression>>> = fits
            .iter()
            .map(|f| {
                (f.components..WIDE)
                    .map(|c| {
                        let literal = Expression::Literal(filler(f.target, c + 1 == WIDE));
                        exprs.append(literal, Span::UNDEFINED)
                    })
                    .collect()
            })
            .collect();
        let start = exprs.len();
        let mut fitted = Vec::new();
        for (f, fill) in fits.iter().zip(fillers) {
            let mut e = match f.member {
                Some(index) => exprs.append(
                    Expression::AccessIndex { base: value, index },
                    Span::UNDEFINED,
                ),
                None => value,
            };
            if f.kind != f.target {
                let cast = Expression::As {
                    expr: e,
                    kind: f.target,
                    convert: Some(WIDTH),
                };
                e = exprs.append(cast, Span::UNDEFINED);
            }
            if f.components != WIDE {
                let mut components = vec![e];
                components.extend(fill);
                e = exprs.append(
                    Expression::Compose {
                        ty: f.ty,
                        components,
                    },
                    Span::UNDEFINED,
                );
            }
            fitted.push((f.member, e));
        }
        let new = match count {
            Some(count) => {
                let components = (0..count)
                    .map(|index| match fitted.iter().find(|p| p.0 == Some(index)) {
                        Some(&(_, e)) => e,
                        None => exprs.append(
                            Expression::AccessIndex { base: value, index },
                            Span::UNDEFINED,
                        ),
                    })
                    .collect();
                let compose = Expression::Compose {
                    ty: new_ty,
                    components,
                };
                exprs.append(compose, Span::UNDEFINED)
            }
            None => fitted[0].1,
        };
        (exprs.range_from(start), new)
    };
    function.body = rewrite_returns(body, &mut function.expressions, &make);
}

/// A copy of the structure `old` with `members`, laid out anew.
fn restruct(
    module: &mut Module,
    old: Handle<Type>,
    mut members: Vec<StructMember>,
) -> Handle<Type> {
    let mut layouter = naga::proc::Layouter::default();
    // The members' types are in the module; a failed layout keeps the old offsets.
    if layouter.update(module.to_ctx()).is_err() {
        return old;
    }
    let mut offset = 0;
    let mut alignment = naga::proc::Alignment::ONE;
    for m in &mut members {
        let layout = layouter[m.ty];
        offset = layout.alignment.round_up(offset);
        m.offset = offset;
        offset += layout.size;
        alignment = alignment.max(layout.alignment);
    }
    let name = module.types[old].name.clone();
    module.types.insert(
        Type {
            name,
            inner: TypeInner::Struct {
                members,
                span: alignment.round_up(offset),
            },
        },
        Span::UNDEFINED,
    )
}

/// What a return's value is replaced by: the expressions to emit first, and the new value.
type Remake<'a> = dyn Fn(Handle<Expression>, &mut Arena<Expression>) -> (naga::Range<Expression>, Handle<Expression>)
    + 'a;

/// `block` with every return of a value replaced by what `make` builds from it.
fn rewrite_returns(block: Block, exprs: &mut Arena<Expression>, make: &Remake<'_>) -> Block {
    let mut out = Block::new();
    for (statement, span) in block.span_into_iter() {
        let statement = match statement {
            Statement::Return { value: Some(value) } => {
                let (emitted, new) = make(value, exprs);
                out.push(Statement::Emit(emitted), span);
                Statement::Return { value: Some(new) }
            }
            Statement::Block(b) => Statement::Block(rewrite_returns(b, exprs, make)),
            Statement::If {
                condition,
                accept,
                reject,
            } => Statement::If {
                condition,
                accept: rewrite_returns(accept, exprs, make),
                reject: rewrite_returns(reject, exprs, make),
            },
            Statement::Switch { selector, cases } => Statement::Switch {
                selector,
                cases: cases
                    .into_iter()
                    .map(|c| naga::SwitchCase {
                        body: rewrite_returns(c.body, exprs, make),
                        ..c
                    })
                    .collect(),
            },
            Statement::Loop {
                body,
                continuing,
                break_if,
            } => Statement::Loop {
                body: rewrite_returns(body, exprs, make),
                continuing: rewrite_returns(continuing, exprs, make),
                break_if,
            },
            other => other,
        };
        out.push(statement, span);
    }
    out
}

/// The blend's second source: the colour a fragment program gives beside its first at location 0.
const SECOND_SOURCE: u32 = 1;

/// Adds to the first entry point a second source for blending at location 0 holding `alpha` (f32
/// bits) in every channel, for a blend block reading the blend constant's colour and its alpha on
/// a back end with only the first as a factor (`BlendInShader`): the alpha is read as the second
/// source's. Refused where the program writes any target but the first, which the second source
/// excludes.
pub fn second_source(module: &mut Module, alpha: u32) -> Result<(), String> {
    let Some(result) = module
        .entry_points
        .first()
        .and_then(|e| e.function.result.as_ref())
    else {
        return refused(format_args!(
            "a second source for a program without outputs"
        ));
    };
    let result_ty = result.ty;
    let (mut members, single) = match &module.types[result_ty].inner {
        TypeInner::Struct { members, .. } => (members.clone(), false),
        _ => (
            vec![StructMember {
                name: None,
                ty: result_ty,
                binding: result.binding.clone(),
                offset: 0,
            }],
            true,
        ),
    };
    let mut first = false;
    for m in &mut members {
        if let Some(naga::Binding::Location {
            location,
            blend_src,
            ..
        }) = &mut m.binding
        {
            if *location != 0 {
                return refused(format_args!(
                    "a second source for a program writing a target past the first"
                ));
            }
            *blend_src = Some(0);
            first = true;
        }
    }
    if !first {
        return refused(format_args!(
            "a second source for a program writing no colour"
        ));
    }
    let ty = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Vector {
                size: VectorSize::Quad,
                scalar: Scalar::F32,
            },
        },
        Span::UNDEFINED,
    );
    members.push(StructMember {
        name: None,
        ty,
        binding: Some(naga::Binding::Location {
            location: 0,
            interpolation: None,
            sampling: None,
            blend_src: Some(SECOND_SOURCE),
            per_primitive: false,
        }),
        offset: 0,
    });
    let count = members.len() as u32;
    let new_ty = restruct(module, result_ty, members);
    let function = &mut module.entry_points[0].function;
    if let Some(result) = function.result.as_mut() {
        result.ty = new_ty;
        result.binding = None;
    }
    let body = std::mem::take(&mut function.body);
    let make = |value: Handle<Expression>, exprs: &mut Arena<Expression>| {
        let literal = exprs.append(
            Expression::Literal(Literal::F32(f32::from_bits(alpha))),
            Span::UNDEFINED,
        );
        let start = exprs.len();
        let mut components: Vec<Handle<Expression>> = if single {
            vec![value]
        } else {
            (0..count - 1)
                .map(|index| {
                    exprs.append(
                        Expression::AccessIndex { base: value, index },
                        Span::UNDEFINED,
                    )
                })
                .collect()
        };
        components.push(exprs.append(
            Expression::Splat {
                size: VectorSize::Quad,
                value: literal,
            },
            Span::UNDEFINED,
        ));
        let compose = Expression::Compose {
            ty: new_ty,
            components,
        };
        let new = exprs.append(compose, Span::UNDEFINED);
        (exprs.range_from(start), new)
    };
    function.body = rewrite_returns(body, &mut function.expressions, &make);
    Ok(())
}
