//! A pixel program's inputs taken at another place in the pixel than where they were interpolated
//! (`eval_centroid`, `eval_sample_index`, `eval_snapped`), which WGSL lacks, and its position where
//! Direct3D gives it at another place than WGSL does (the pixel's centre in a program run per sample,
//! the centroid where declared so). An interpolated value is affine on the screen (over w where
//! perspective-correct), so the entry point, where every lane of a quad runs, takes its slopes with
//! derivatives (exact for an affine value at any precision of control), and the value anywhere in the pixel is its value plus the slopes times the
//! distance. Places are in pixels from the pixel's centre; samples are at Direct3D's standard
//! positions; the centroid is Direct3D's: the centre where every sample is covered, else the first
//! covered sample.

use super::{Emit, FunctionLowering, Lowering, Result, malformed, unsupported};
use crate::ir::{Op, builtin, code, interp};
use crate::variant::{FOUR_SAMPLES, SAMPLE_POSITION_STEPS};
use alloc::format;
use alloc::vec::Vec;
use naga::{
    AddressSpace, BinaryOperator, Block, DerivativeAxis, DerivativeControl, Expression, Function,
    FunctionArgument, GlobalVariable, Handle, Literal, MathFunction, Scalar, Span, Statement,
    TypeInner,
};

/// What the entry point leaves for the evaluations in the program.
#[derive(Default)]
pub(super) struct Evaluated {
    /// Per input declaration read at another place: its companions.
    inputs: Vec<(u32, Companions)>,
    /// w's slopes (the position's w, one over the clip w), along x and y.
    w_slope: Option<Handle<GlobalVariable>>,
    /// Where the centroid is.
    centroid: Option<Handle<GlobalVariable>>,
    /// The sample positions of the targets' sample count.
    samples: Option<Handle<GlobalVariable>>,
}

#[derive(Clone, Copy)]
struct Companions {
    /// The slopes of the value (times w where perspective-correct) along x and y.
    x: Handle<GlobalVariable>,
    y: Handle<GlobalVariable>,
    /// w where the value was interpolated, and that place.
    w: Handle<GlobalVariable>,
    place: Handle<GlobalVariable>,
}

impl Lowering<'_> {
    /// Declares the companions of every input a pixel program evaluates elsewhere.
    pub(super) fn declare_evaluated(&mut self) -> Result<()> {
        let evaluating = |c: u16| {
            matches!(
                c,
                code::INTERPOLATE_AT_CENTROID
                    | code::INTERPOLATE_AT_SAMPLE
                    | code::INTERPOLATE_AT_OFFSET
            )
        };
        let mut dcls: Vec<u32> = Vec::new();
        for op in self.ir.ops.iter().filter(|op| evaluating(op.code)) {
            let dcl = *op
                .refs
                .first()
                .ok_or_else(|| malformed("an evaluation without its input".into()))?;
            if !dcls.contains(&dcl) {
                dcls.push(dcl);
            }
        }
        if dcls.is_empty() && !self.position_moved() {
            return Ok(());
        }
        if self.per_sample() {
            for &dcl in &dcls {
                self.interpolate_at_sample(dcl, evaluating);
            }
        }
        let vec2 = self.vec_ty(2, Scalar::F32)?;
        let f32_ty = self.scalar_ty(Scalar::F32);
        self.evaluated.w_slope =
            Some(self.global("w_slope".into(), AddressSpace::Private, None, vec2, None));
        self.evaluated.centroid =
            Some(self.global("centroid".into(), AddressSpace::Private, None, vec2, None));
        for dcl in dcls {
            let i = *self
                .io_index
                .get(&dcl)
                .ok_or_else(|| malformed(format!("evaluated input {dcl}")))?;
            let ty = self.module.global_variables[self.io[i].1.var].ty;
            let mut companion = |what: &str, ty| {
                self.global(
                    format!("{what}{dcl}"),
                    AddressSpace::Private,
                    None,
                    ty,
                    None,
                )
            };
            let companions = Companions {
                x: companion("slope_x", ty),
                y: companion("slope_y", ty),
                w: companion("w_at", f32_ty),
                place: companion("place", vec2),
            };
            self.evaluated.inputs.push((dcl, companions));
        }
        Ok(())
    }

    /// Interpolates input `dcl` at the sample being run where the program runs per sample, reads it
    /// only through evaluations at samples and shares its location with nothing, so that an
    /// evaluation at that sample is the interpolated value itself, as the hardware gives it.
    fn interpolate_at_sample(&mut self, dcl: u32, evaluating: impl Fn(u16) -> bool) {
        let used_otherwise = self.ir.ops.iter().any(|op| {
            op.refs.first() == Some(&dcl)
                && (op.code == code::INPUT_LOAD
                    || evaluating(op.code) && op.code != code::INTERPOLATE_AT_SAMPLE)
        });
        let Some(&i) = self.io_index.get(&dcl) else {
            return;
        };
        let location = self.io[i].1.location.map(|(l, _)| l);
        let shared = self
            .io
            .iter()
            .filter(|(_, io)| location.is_some() && io.location.map(|(l, _)| l) == location)
            .count()
            > 1;
        let io = &mut self.io[i].1;
        if !used_otherwise
            && !shared
            && location.is_some()
            && io.interpolation & (interp::FLAT | interp::CENTROID) == 0
        {
            io.interpolation |= interp::SAMPLE;
        }
    }

    /// The position is declared at a place WGSL does not give it: the centre in a program run per
    /// sample, or the centroid, of several samples.
    fn position_moved(&self) -> bool {
        self.io.iter().any(|(_, io)| {
            io.builtin == Some(builtin::POSITION)
                && io.location.is_none()
                && (io.interpolation & interp::CENTROID != 0
                    || (self.per_sample() && io.interpolation & interp::SAMPLE == 0))
        }) && self.stage == wcontainers_gpu::SourceStage::Pixel
            // With one sample every place is the centre.
            && self.variant.samples > 1
    }

    fn per_sample(&self) -> bool {
        self.io.iter().any(|(_, io)| {
            io.builtin == Some(builtin::SAMPLE_ID) || io.interpolation & interp::SAMPLE != 0
        })
    }

    /// The table of sample positions of the targets' sample count, in pixels from the centre.
    fn sample_positions(&mut self) -> Result<Handle<GlobalVariable>> {
        if let Some(g) = self.evaluated.samples {
            return Ok(g);
        }
        let positions: &[(f32, f32)] = match self.variant.samples {
            1 => &[(0.0, 0.0)],
            n if usize::from(n) == FOUR_SAMPLES.len() => &FOUR_SAMPLES,
            _ => return Err(unsupported(crate::refusal::SAMPLE_COUNT)),
        };
        let vec2 = self.vec_ty(2, Scalar::F32)?;
        let array = self.ty(TypeInner::Array {
            base: vec2,
            size: naga::ArraySize::Constant(
                core::num::NonZeroU32::new(positions.len() as u32).unwrap(),
            ),
            stride: core::mem::size_of::<[f32; 2]>() as u32,
        });
        let mut components = Vec::new();
        for &(x, y) in positions {
            let mut at = |v: f32| {
                self.module.global_expressions.append(
                    Expression::Literal(Literal::F32(v / SAMPLE_POSITION_STEPS)),
                    Span::UNDEFINED,
                )
            };
            let xy = [at(x), at(y)].to_vec();
            components.push(self.module.global_expressions.append(
                Expression::Compose {
                    ty: vec2,
                    components: xy,
                },
                Span::UNDEFINED,
            ));
        }
        let init = self.module.global_expressions.append(
            Expression::Compose {
                ty: array,
                components,
            },
            Span::UNDEFINED,
        );
        let g = self.global(
            "sample_positions".into(),
            AddressSpace::Private,
            None,
            array,
            Some(init),
        );
        self.evaluated.samples = Some(g);
        Ok(g)
    }

    /// In the entry point, after the inputs are stored: the slopes and places the evaluations read,
    /// and the position moved to its declared place.
    pub(super) fn evaluate_prologue(
        &mut self,
        f: &mut Function,
        body: &mut Block,
        e: &mut Emit,
    ) -> Result<()> {
        let Some(w_slope) = self.evaluated.w_slope else {
            return Ok(());
        };
        let vec2 = self.vec_ty(2, Scalar::F32)?;
        let vec4 = self.vec_ty(4, Scalar::F32)?;
        let u32_ty = self.scalar_ty(Scalar::U32);
        let position = argument(
            f,
            body,
            e,
            naga::BuiltIn::Position { invariant: false },
            vec4,
        );
        let xy = e.swizzle(f, body, position, 0, 2)?;
        let floor = math(f, body, e, MathFunction::Floor, &[xy]);
        let half = e.add(f, body, Expression::Literal(Literal::F32(0.5)));
        let half = e.add(
            f,
            body,
            Expression::Splat {
                size: naga::VectorSize::Bi,
                value: half,
            },
        );
        let centre = bin(f, body, e, BinaryOperator::Add, floor, half);
        // Where the position was taken, from the centre.
        let taken = bin(f, body, e, BinaryOperator::Subtract, xy, centre);
        let w_index = (naga::VectorSize::Quad as u32) - 1;
        let w = e.add(
            f,
            body,
            Expression::AccessIndex {
                base: position,
                index: w_index,
            },
        );
        let slopes = |f: &mut Function, body: &mut Block, e: &mut Emit, v| {
            [DerivativeAxis::X, DerivativeAxis::Y].map(|axis| {
                e.add(
                    f,
                    body,
                    Expression::Derivative {
                        axis,
                        ctrl: DerivativeControl::None,
                        expr: v,
                    },
                )
            })
        };
        let [wx, wy] = slopes(f, body, e, w);
        let w_slopes = e.add(
            f,
            body,
            Expression::Compose {
                ty: vec2,
                components: [wx, wy].to_vec(),
            },
        );
        store(f, body, e, w_slope, w_slopes);
        let zero = e.add(f, body, Expression::ZeroValue(vec2));

        // Direct3D's centroid: the centre where every sample is covered, else the first covered one.
        let samples = self.variant.samples;
        let centroid = if samples > 1 {
            let table = self.sample_positions()?;
            let mask = argument(f, body, e, naga::BuiltIn::SampleMask, u32_ty);
            let all = e.add(
                f,
                body,
                Expression::Literal(Literal::U32(u32::MAX >> (u32::BITS - u32::from(samples)))),
            );
            let full = bin(f, body, e, BinaryOperator::Equal, mask, all);
            let first = math(f, body, e, MathFunction::FirstTrailingBit, &[mask]);
            let first = sample_position(f, body, e, table, first);
            e.add(
                f,
                body,
                Expression::Select {
                    condition: full,
                    accept: zero,
                    reject: first,
                },
            )
        } else {
            zero
        };
        let centroid_global = self.evaluated.centroid.expect("declared with the slopes");
        store(f, body, e, centroid_global, centroid);
        for (dcl, c) in self.evaluated.inputs.clone() {
            let io = &self.io[self.io_index[&dcl]].1;
            let (var, modes) = (io.var, io.interpolation);
            if modes & interp::FLAT != 0 {
                continue;
            }
            let place = if modes & interp::SAMPLE != 0 {
                let table = self.sample_positions()?;
                let index = argument(f, body, e, naga::BuiltIn::SampleIndex, u32_ty);
                sample_position(f, body, e, table, index)
            } else if modes & interp::CENTROID != 0 {
                // The hardware's centroid is not Direct3D's rule everywhere, and the slopes between
                // lanes interpolated at different places are not the value's.
                if samples > 1 {
                    return Err(unsupported(crate::refusal::EVALUATED_CENTROID_INPUT));
                }
                zero
            } else {
                zero
            };
            store(f, body, e, c.place, place);
            let moved = bin(f, body, e, BinaryOperator::Subtract, place, taken);
            let along = dot(f, body, e, w_slopes, moved);
            let w_at = bin(f, body, e, BinaryOperator::Add, w, along);
            store(f, body, e, c.w, w_at);
            let p = e.add(f, body, Expression::GlobalVariable(var));
            let mut v = e.add(f, body, Expression::Load { pointer: p });
            if modes & interp::NO_PERSPECTIVE == 0 {
                v = bin(f, body, e, BinaryOperator::Multiply, v, w_at);
            }
            let [x, y] = slopes(f, body, e, v);
            store(f, body, e, c.x, x);
            store(f, body, e, c.y, y);
        }

        // The position at its declared place.
        if self.position_moved() {
            let io = self
                .io
                .iter()
                .find(|(_, io)| io.builtin == Some(builtin::POSITION))
                .map(|(_, io)| (io.var, io.interpolation, io.ty.basic()));
            let (var, modes, Some(member)) = io.expect("a moved position is declared") else {
                return Err(malformed("the position is not a vector".into()));
            };
            let target = if modes & interp::CENTROID != 0 {
                centroid
            } else {
                zero
            };
            let moved = bin(f, body, e, BinaryOperator::Subtract, target, taken);
            let [px, py] = slopes(f, body, e, position);
            let (mx, my) = (
                e.add(
                    f,
                    body,
                    Expression::AccessIndex {
                        base: moved,
                        index: 0,
                    },
                ),
                e.add(
                    f,
                    body,
                    Expression::AccessIndex {
                        base: moved,
                        index: 1,
                    },
                ),
            );
            let along_x = bin(f, body, e, BinaryOperator::Multiply, px, mx);
            let along_y = bin(f, body, e, BinaryOperator::Multiply, py, my);
            let at = bin(f, body, e, BinaryOperator::Add, position, along_x);
            let at = bin(f, body, e, BinaryOperator::Add, at, along_y);
            let at = if usize::from(member.size) < naga::VectorSize::Quad as usize {
                e.swizzle(f, body, at, 0, member.size)?
            } else {
                at
            };
            store(f, body, e, var, at);
        }
        Ok(())
    }
}

impl FunctionLowering<'_, '_> {
    /// An input evaluated at the centroid, a sample, or an offset from the centre.
    pub(super) fn evaluate(&mut self, b: &mut Block, op: &Op) -> Result<Handle<Expression>> {
        let r = &op.refs;
        let (dcl, address) = (r[0], r.get(1).copied().unwrap_or(0));
        let (var, ty) = self.memory_var(code::INPUT_LOAD, dcl)?;
        let v = self.load(b, var, &ty, address, &op.ty)?;
        let modes = self.l.io[self.l.io_index[&dcl]].1.interpolation;
        if modes & interp::FLAT != 0 {
            return Ok(v);
        }
        let c = self
            .l
            .evaluated
            .inputs
            .iter()
            .find(|(d, _)| *d == dcl)
            .map(|(_, c)| *c)
            .ok_or_else(|| malformed(format!("evaluated input {dcl} undeclared")))?;
        let target = match op.code {
            code::INTERPOLATE_AT_CENTROID => {
                let g = self
                    .l
                    .evaluated
                    .centroid
                    .expect("declared with the companions");
                let p = self.add(b, Expression::GlobalVariable(g));
                self.add(b, Expression::Load { pointer: p })
            }
            code::INTERPOLATE_AT_SAMPLE => {
                let table = self.l.sample_positions()?;
                let index = self.get_as(b, r[2], Scalar::U32)?;
                let p = self.add(b, Expression::GlobalVariable(table));
                let p = self.add(b, Expression::Access { base: p, index });
                self.add(b, Expression::Load { pointer: p })
            }
            _ => self.get_as(b, r[2], Scalar::F32)?,
        };
        let p = self.add(b, Expression::GlobalVariable(c.place));
        let place = self.add(b, Expression::Load { pointer: p });
        let d = self.bin(b, BinaryOperator::Subtract, target, place);
        let dx = self.add(b, Expression::AccessIndex { base: d, index: 0 });
        let dy = self.add(b, Expression::AccessIndex { base: d, index: 1 });
        let sx = self.load(b, c.x, &ty, address, &op.ty)?;
        let sy = self.load(b, c.y, &ty, address, &op.ty)?;
        let along_x = self.bin(b, BinaryOperator::Multiply, sx, dx);
        let along_y = self.bin(b, BinaryOperator::Multiply, sy, dy);
        let along = self.bin(b, BinaryOperator::Add, along_x, along_y);
        if modes & interp::NO_PERSPECTIVE != 0 {
            return Ok(self.bin(b, BinaryOperator::Add, v, along));
        }
        let p = self.add(b, Expression::GlobalVariable(c.w));
        let w = self.add(b, Expression::Load { pointer: p });
        let g = self
            .l
            .evaluated
            .w_slope
            .expect("declared with the companions");
        let p = self.add(b, Expression::GlobalVariable(g));
        let slopes = self.add(b, Expression::Load { pointer: p });
        let w_along = self.math(b, MathFunction::Dot, &[slopes, d]);
        let w_there = self.bin(b, BinaryOperator::Add, w, w_along);
        let scaled = self.bin(b, BinaryOperator::Multiply, v, w);
        let there = self.bin(b, BinaryOperator::Add, scaled, along);
        let there = self.bin(b, BinaryOperator::Divide, there, w_there);
        // At the place the value was interpolated, the value itself, unrounded by the w division.
        let vec2 = self.l.vec_ty(2, Scalar::F32)?;
        let zero = self.add(b, Expression::ZeroValue(vec2));
        let same = self.bin(b, BinaryOperator::Equal, d, zero);
        let same = self.add(
            b,
            Expression::Relational {
                fun: naga::RelationalFunction::All,
                argument: same,
            },
        );
        Ok(self.add(
            b,
            Expression::Select {
                condition: same,
                accept: v,
                reject: there,
            },
        ))
    }
}

/// The entry point's argument for `builtin`, added where the program has none.
fn argument(
    f: &mut Function,
    body: &mut Block,
    e: &mut Emit,
    builtin: naga::BuiltIn,
    ty: Handle<naga::Type>,
) -> Handle<Expression> {
    let binding = Some(naga::Binding::BuiltIn(builtin));
    let index = match f.arguments.iter().position(|a| a.binding == binding) {
        Some(i) => i,
        None => {
            f.arguments.push(FunctionArgument {
                name: None,
                ty,
                binding,
            });
            f.arguments.len() - 1
        }
    };
    e.add(f, body, Expression::FunctionArgument(index as u32))
}

fn sample_position(
    f: &mut Function,
    body: &mut Block,
    e: &mut Emit,
    table: Handle<GlobalVariable>,
    index: Handle<Expression>,
) -> Handle<Expression> {
    let p = e.add(f, body, Expression::GlobalVariable(table));
    let p = e.add(f, body, Expression::Access { base: p, index });
    e.add(f, body, Expression::Load { pointer: p })
}

fn store(
    f: &mut Function,
    body: &mut Block,
    e: &mut Emit,
    g: Handle<GlobalVariable>,
    value: Handle<Expression>,
) {
    let pointer = e.add(f, body, Expression::GlobalVariable(g));
    e.stmt(f, body, Statement::Store { pointer, value });
}

fn bin(
    f: &mut Function,
    body: &mut Block,
    e: &mut Emit,
    op: BinaryOperator,
    left: Handle<Expression>,
    right: Handle<Expression>,
) -> Handle<Expression> {
    e.add(f, body, Expression::Binary { op, left, right })
}

fn math(
    f: &mut Function,
    body: &mut Block,
    e: &mut Emit,
    fun: MathFunction,
    args: &[Handle<Expression>],
) -> Handle<Expression> {
    e.add(
        f,
        body,
        Expression::Math {
            fun,
            arg: args[0],
            arg1: args.get(1).copied(),
            arg2: None,
            arg3: None,
        },
    )
}

fn dot(
    f: &mut Function,
    body: &mut Block,
    e: &mut Emit,
    a: Handle<Expression>,
    b: Handle<Expression>,
) -> Handle<Expression> {
    math(f, body, e, MathFunction::Dot, &[a, b])
}
