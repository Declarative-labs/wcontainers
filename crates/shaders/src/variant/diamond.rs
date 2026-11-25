//! Direct3D's aliased line rule for lines drawn as rectangles (`VariantFlags::LINE_DIAMOND`): the
//! pixel program keeps the pixels the diamond-exit rule picks for the line whose ends the stage
//! helper passes, through a library function written in WGSL (`diamond.wgsl`).

use alloc::format;
use naga::{
    Binding, BuiltIn, Expression, Interpolation, MathFunction, Module, Scalar, Span, Statement,
    UnaryOperator,
};

use super::{Result, Wrapped, binary, f32_lit, index, math, push, vector};
use crate::ConvertError;

const LIBRARY: &str = include_str!("diamond.wgsl");
const DIAMOND_EXIT: &str = "diamond_exit";

/// The window-space offset of a pixel's centre from its corner.
const CENTRE: f32 = 0.5;

/// Discards the fragment unless the diamond-exit rule picks its pixel for the line whose window-space
/// ends (x and y of the first, then of the second) the stage helper's rectangle passes as four flat
/// f32 at `location`.
pub(super) fn line_diamond(module: &mut Module, w: &mut Wrapped, location: u32) -> Result<()> {
    let parsed = naga::front::wgsl::parse_str(LIBRARY).map_err(|e| {
        ConvertError::Malformed(format!("the line rule library does not parse: {e}"))
    })?;
    let (f, _) = parsed
        .functions
        .iter()
        .find(|(_, f)| f.name.as_deref() == Some(DIAMOND_EXIT))
        .ok_or_else(|| ConvertError::Malformed(format!("the library has no {DIAMOND_EXIT}")))?;
    let mut library = super::filter::Library::default();
    library.import(module, &parsed, f)?;
    let mut made = library.ahead.into_inner().into_iter();
    let function = match (made.next(), made.next()) {
        (Some(g), None) => module.functions.append(g, Span::UNDEFINED),
        _ => {
            return Err(ConvertError::Malformed(format!(
                "{DIAMOND_EXIT} was not imported alone"
            )));
        }
    };
    let vec4 = vector(module, crate::build::VEC4 as u8, Scalar::F32);
    let vec2 = vector(module, 2, Scalar::F32);
    let ends = w.input(
        module,
        |b| matches!(b, Binding::Location { location: l, .. } if *l == location),
        Binding::Location {
            location,
            interpolation: Some(Interpolation::Flat),
            sampling: None,
            blend_src: None,
            per_primitive: false,
        },
        vec4,
    );
    let position = w.input(
        module,
        |b| matches!(b, Binding::BuiltIn(BuiltIn::Position { .. })),
        Binding::BuiltIn(BuiltIn::Position { invariant: false }),
        vec4,
    );
    let (f, body) = (&mut w.f, &mut w.prologue);
    // The pixel's centre, also where the program runs per sample.
    let [x, y] = [0, 1].map(|i| index(f, body, position, i));
    let at = push(
        f,
        body,
        Expression::Compose {
            ty: vec2,
            components: alloc::vec![x, y],
        },
    );
    let corner = math(f, body, MathFunction::Floor, [at, at]);
    let half = f32_lit(f, CENTRE);
    let half = push(
        f,
        body,
        Expression::Splat {
            size: naga::VectorSize::Bi,
            value: half,
        },
    );
    let centre = binary(f, body, naga::BinaryOperator::Add, corner, half);
    let kept = push(f, body, Expression::CallResult(function));
    body.push(
        Statement::Call {
            function,
            arguments: alloc::vec![ends, centre],
            result: Some(kept),
        },
        Span::UNDEFINED,
    );
    let left = push(
        f,
        body,
        Expression::Unary {
            op: UnaryOperator::LogicalNot,
            expr: kept,
        },
    );
    let mut accept = naga::Block::new();
    accept.push(Statement::Kill, Span::UNDEFINED);
    body.push(
        Statement::If {
            condition: left,
            accept,
            reject: naga::Block::new(),
        },
        Span::UNDEFINED,
    );
    w.changed = true;
    Ok(())
}
