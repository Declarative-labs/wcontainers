//! The position's transform applied to a vertex program in the common form: the pass-through that
//! draws a geometry program's output, so that stream output takes the geometry program's depth as
//! written and the host draws it in its own clip space.

use naga::{
    BinaryOperator, Binding, Block, BuiltIn, Expression, Function, Handle, Literal, Module, Span,
    Statement, SwitchCase, Type, TypeInner, UnaryOperator,
};
use wcontainers_nir::mesa::PositionTransform;

/// Half, which takes OpenGL's depth from -w..w to the host's 0..w.
const HALF: f32 = 0.5;

/// Where the returned value holds the position, and the position's type.
#[derive(Clone, Copy)]
struct Place {
    /// The result's struct, its member holding the position and its member count; `None` when the
    /// result is the position.
    member: Option<(Handle<Type>, u32, u32)>,
    position: Handle<Type>,
    transform: PositionTransform,
}

/// Applies `t` (but the point size, which the program's NIR drops) to the position the entry point
/// returns.
pub fn transform(module: &mut Module, t: PositionTransform) -> Result<(), String> {
    if !(t.flip_y || t.half_z || t.reverse_z) {
        return Ok(());
    }
    let entry = module
        .entry_points
        .first_mut()
        .ok_or("a pass-through without an entry point")?;
    let result = entry
        .function
        .result
        .as_ref()
        .ok_or("a pass-through returning nothing")?;
    let is_position =
        |b: &Option<Binding>| matches!(b, Some(Binding::BuiltIn(BuiltIn::Position { .. })));
    let place = if is_position(&result.binding) {
        Place {
            member: None,
            position: result.ty,
            transform: t,
        }
    } else {
        let TypeInner::Struct { members, .. } = &module.types[result.ty].inner else {
            return crate::state::refused(format_args!("a pass-through without a position"));
        };
        let index = members
            .iter()
            .position(|m| is_position(&m.binding))
            .ok_or("a pass-through without a position")?;
        Place {
            member: Some((result.ty, index as u32, members.len() as u32)),
            position: members[index].ty,
            transform: t,
        }
    };
    let body = std::mem::take(&mut entry.function.body);
    entry.function.body = rewritten(body, &mut entry.function, place);
    Ok(())
}

/// `block` with each return's position transformed.
fn rewritten(block: Block, f: &mut Function, place: Place) -> Block {
    let mut out = Block::with_capacity(block.len());
    for (statement, span) in block.span_into_iter() {
        let statement = match statement {
            Statement::Return { value: Some(value) } => Statement::Return {
                value: Some(returned(f, &mut out, value, place)),
            },
            Statement::Block(b) => Statement::Block(rewritten(b, f, place)),
            Statement::If {
                condition,
                accept,
                reject,
            } => Statement::If {
                condition,
                accept: rewritten(accept, f, place),
                reject: rewritten(reject, f, place),
            },
            Statement::Switch { selector, cases } => Statement::Switch {
                selector,
                cases: cases
                    .into_iter()
                    .map(|c| SwitchCase {
                        body: rewritten(c.body, f, place),
                        ..c
                    })
                    .collect(),
            },
            Statement::Loop {
                body,
                continuing,
                break_if,
            } => Statement::Loop {
                body: rewritten(body, f, place),
                continuing: rewritten(continuing, f, place),
                break_if,
            },
            other => other,
        };
        out.push(statement, span);
    }
    out
}

/// The value returned in place of `value`, its position transformed, made in `out` as OpenGL's
/// clip space is taken to the host's: y negated, depth halved onto 0..w, then reversed.
fn returned(
    f: &mut Function,
    out: &mut Block,
    value: Handle<Expression>,
    place: Place,
) -> Handle<Expression> {
    let t = place.transform;
    let half = f
        .expressions
        .append(Expression::Literal(Literal::F32(HALF)), Span::UNDEFINED);
    let start = f.expressions.len();
    let mut add = |e| f.expressions.append(e, Span::UNDEFINED);
    let position = match place.member {
        None => value,
        Some((_, index, _)) => add(Expression::AccessIndex { base: value, index }),
    };
    let [x, mut y, mut z, w] = std::array::from_fn(|index| {
        add(Expression::AccessIndex {
            base: position,
            index: index as u32,
        })
    });
    if t.flip_y {
        y = add(Expression::Unary {
            op: UnaryOperator::Negate,
            expr: y,
        });
    }
    if t.half_z {
        let sum = add(Expression::Binary {
            op: BinaryOperator::Add,
            left: z,
            right: w,
        });
        z = add(Expression::Binary {
            op: BinaryOperator::Multiply,
            left: sum,
            right: half,
        });
    }
    if t.reverse_z {
        z = add(Expression::Binary {
            op: BinaryOperator::Subtract,
            left: w,
            right: z,
        });
    }
    let transformed = add(Expression::Compose {
        ty: place.position,
        components: vec![x, y, z, w],
    });
    let value = match place.member {
        None => transformed,
        Some((ty, index, members)) => {
            let components = (0..members)
                .map(|m| {
                    if m == index {
                        transformed
                    } else {
                        add(Expression::AccessIndex {
                            base: value,
                            index: m,
                        })
                    }
                })
                .collect();
            add(Expression::Compose { ty, components })
        }
    };
    out.push(
        Statement::Emit(f.expressions.range_from(start)),
        Span::UNDEFINED,
    );
    value
}
