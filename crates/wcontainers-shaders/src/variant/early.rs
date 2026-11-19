//! Early depth and stencil tests run by the program (`VariantFlags::EARLY_TESTS`): before the pixel
//! shader's side effects, each covered sample's depth and stencil are read from the pass's read-only
//! attachment and tested as the draw's tests say; a fragment no sample of which passes is discarded.

use super::{
    FOUR_SAMPLES, Result, SAMPLE_POSITION_STEPS, Wrapped, biased, binary, f32_lit, helper, index,
    load_helper, push, scalar, vector,
};
use crate::ConvertError;
use naga::{
    AddressSpace, Binding, Block, BuiltIn, Expression, Function, GlobalVariable, Handle,
    ImageClass, ImageDimension, Literal, Module, Scalar, ScalarKind, Span, Statement,
    SwizzleComponent, Type, TypeInner, VectorSize,
};
use wcontainers_gpu::{
    EARLY_DEPTH_BINDING, EARLY_STENCIL_BINDING, HELPER_SET, HelperValue, Reflection, ShaderStage,
};

/// Adds the early tests to the wrapped pixel program's prologue. `samples` is the targets' sample
/// count; `bias` the helper member of the raster block's depth bias, where the variant adds it.
pub(super) fn early_tests(
    module: &mut Module,
    w: &mut Wrapped,
    reflection: &mut Reflection,
    samples: u8,
    per_sample: bool,
    bias: Option<(Handle<GlobalVariable>, u32)>,
) -> Result<()> {
    use naga::BinaryOperator::{Add, Equal, InclusiveOr, LogicalAnd, Multiply};
    let multi = match (samples, per_sample) {
        (0, _) => return Err(ConvertError::Unsupported("SampleCount")),
        (1, _) => false,
        (4, false) => true,
        (4, true) => return Err(ConvertError::Unsupported("EarlyTestsPerSample")),
        _ => return Err(ConvertError::Unsupported("EarlyTestsSamples")),
    };
    let image = |module: &mut Module, class, binding, name: &str| {
        let ty = module.types.insert(
            Type {
                name: None,
                inner: TypeInner::Image {
                    dim: ImageDimension::D2,
                    arrayed: false,
                    class,
                },
            },
            Span::UNDEFINED,
        );
        module.global_variables.append(
            GlobalVariable {
                name: Some(name.into()),
                space: AddressSpace::Handle,
                binding: Some(naga::ResourceBinding {
                    group: HELPER_SET,
                    binding,
                }),
                ty,
                init: None,
                memory_decorations: Default::default(),
            },
            Span::UNDEFINED,
        )
    };
    let depth = image(
        module,
        ImageClass::Depth { multi },
        EARLY_DEPTH_BINDING,
        "early_depth",
    );
    let stencil = image(
        module,
        ImageClass::Sampled {
            kind: ScalarKind::Uint,
            multi,
        },
        EARLY_STENCIL_BINDING,
        "early_stencil",
    );
    let u32_ty = scalar(module, Scalar::U32);
    let rows = vector(module, 4, Scalar::U32);
    let rows = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Array {
                base: rows,
                size: naga::ArraySize::Constant(core::num::NonZeroU32::new(2).unwrap()),
                stride: 16,
            },
        },
        Span::UNDEFINED,
    );
    let tests = helper(
        module,
        reflection,
        ShaderStage::Fragment,
        HelperValue::EarlyTests,
        rows,
    );
    let vec4 = vector(module, 4, Scalar::F32);
    let position = w.input(
        module,
        |b| matches!(b, Binding::BuiltIn(BuiltIn::Position { .. })),
        Binding::BuiltIn(BuiltIn::Position { invariant: false }),
        vec4,
    );
    let bool_ty = scalar(module, Scalar::BOOL);
    let front = w.input(
        module,
        |b| *b == Binding::BuiltIn(BuiltIn::FrontFacing),
        Binding::BuiltIn(BuiltIn::FrontFacing),
        bool_ty,
    );
    let coverage = multi.then(|| {
        w.input(
            module,
            |b| *b == Binding::BuiltIn(BuiltIn::SampleMask),
            Binding::BuiltIn(BuiltIn::SampleMask),
            u32_ty,
        )
    });
    let (f, body) = (&mut w.f, &mut w.prologue);
    let mut z = index(f, body, position, 2);
    if let Some(bias) = bias {
        z = biased(f, body, z, bias);
    }
    let xy = push(
        f,
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
    let pixel = push(
        f,
        body,
        Expression::As {
            expr: xy,
            kind: ScalarKind::Uint,
            convert: Some(4),
        },
    );
    let codes = load_helper(f, body, tests);
    let [first, second] = [0, 1].map(|i| index(f, body, codes, i));
    let [depth_code, front_code, back_code, reference] =
        [0, 1, 2, 3].map(|i| index(f, body, first, i));
    let mask = index(f, body, second, 0);
    let stencil_code = push(
        f,
        body,
        Expression::Select {
            condition: front,
            accept: front_code,
            reject: back_code,
        },
    );
    let reference = binary(f, body, naga::BinaryOperator::And, reference, mask);
    // One sample's tests: the stored values at `sample`, against the fragment's depth there.
    let passes = |f: &mut Function, body: &mut Block, z, sample| {
        let load = |f: &mut Function, body: &mut Block, image| {
            let image = push(f, body, Expression::GlobalVariable(image));
            let level = (!multi).then(|| u32_lit(f, 0));
            push(
                f,
                body,
                Expression::ImageLoad {
                    image,
                    coordinate: pixel,
                    array_index: None,
                    sample,
                    level,
                },
            )
        };
        let stored_depth = load(f, body, depth);
        let texel = load(f, body, stencil);
        let stored = index(f, body, texel, 0);
        let stored = binary(f, body, naga::BinaryOperator::And, stored, mask);
        let depth_passes = compare(f, body, depth_code, z, stored_depth);
        let stencil_passes = compare(f, body, stencil_code, reference, stored);
        binary(f, body, LogicalAnd, depth_passes, stencil_passes)
    };
    let passed = match coverage {
        None => passes(f, body, z, None),
        Some(coverage) => {
            // z is affine on the screen: each sample's from its derivatives.
            let [dx, dy] = [naga::DerivativeAxis::X, naga::DerivativeAxis::Y].map(|axis| {
                push(
                    f,
                    body,
                    Expression::Derivative {
                        axis,
                        ctrl: naga::DerivativeControl::None,
                        expr: z,
                    },
                )
            });
            let mut kept = u32_lit(f, 0);
            for (s, (x, y)) in FOUR_SAMPLES.iter().enumerate() {
                let (x, y) = (
                    f32_lit(f, x / SAMPLE_POSITION_STEPS),
                    f32_lit(f, y / SAMPLE_POSITION_STEPS),
                );
                let along_x = binary(f, body, Multiply, dx, x);
                let along_y = binary(f, body, Multiply, dy, y);
                let moved = binary(f, body, Add, z, along_x);
                let at = binary(f, body, Add, moved, along_y);
                let sample = f
                    .expressions
                    .append(Expression::Literal(Literal::I32(s as i32)), Span::UNDEFINED);
                let ok = passes(f, body, at, Some(sample));
                let bit = u32_lit(f, 1 << s);
                let none = u32_lit(f, 0);
                let chosen = push(
                    f,
                    body,
                    Expression::Select {
                        condition: ok,
                        accept: bit,
                        reject: none,
                    },
                );
                kept = binary(f, body, InclusiveOr, kept, chosen);
            }
            let covered = binary(f, body, naga::BinaryOperator::And, kept, coverage);
            let none = u32_lit(f, 0);
            let empty = binary(f, body, Equal, covered, none);
            push(
                f,
                body,
                Expression::Unary {
                    op: naga::UnaryOperator::LogicalNot,
                    expr: empty,
                },
            )
        }
    };
    let failed = push(
        f,
        body,
        Expression::Unary {
            op: naga::UnaryOperator::LogicalNot,
            expr: passed,
        },
    );
    let mut accept = Block::new();
    accept.push(Statement::Kill, Span::UNDEFINED);
    body.push(
        Statement::If {
            condition: failed,
            accept,
            reject: Block::new(),
        },
        Span::UNDEFINED,
    );
    Ok(())
}

fn u32_lit(f: &mut Function, v: u32) -> Handle<Expression> {
    f.expressions
        .append(Expression::Literal(Literal::U32(v)), Span::UNDEFINED)
}

/// Whether `a` passes against `b` under a [`wcontainers_gpu::CompareFn`] code (zero: off). Code
/// minus one is the set of relations that pass: less 1, equal 2, greater 4.
fn compare(
    f: &mut Function,
    body: &mut Block,
    code: Handle<Expression>,
    a: Handle<Expression>,
    b: Handle<Expression>,
) -> Handle<Expression> {
    use naga::BinaryOperator::{And, Equal, Greater, Less, NotEqual, Subtract};
    let relation = |f: &mut Function, body: &mut Block, op, bit, rest| {
        let holds = binary(f, body, op, a, b);
        let bit = u32_lit(f, bit);
        push(
            f,
            body,
            Expression::Select {
                condition: holds,
                accept: bit,
                reject: rest,
            },
        )
    };
    let none = u32_lit(f, 0);
    let greater = relation(f, body, Greater, 4, none);
    let equal = relation(f, body, Equal, 2, greater);
    let relation = relation(f, body, Less, 1, equal);
    let one = u32_lit(f, 1);
    let passing = binary(f, body, Subtract, code, one);
    let passed = binary(f, body, And, passing, relation);
    let zero = u32_lit(f, 0);
    binary(f, body, NotEqual, passed, zero)
}
