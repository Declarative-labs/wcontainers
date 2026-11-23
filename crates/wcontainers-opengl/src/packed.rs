//! Vertex inputs of the packed 10-10-10-2 formats the interface lacks (signed, scaled, or blue
//! first): each fetched as one word and unpacked in the vertex program (`VertexFormatConverted`).

use naga::{
    Binding, Block, Expression, Function, FunctionArgument, Handle, Literal, MathFunction, Module,
    Scalar, ScalarKind, Span, Statement, SwizzleComponent, Type, TypeInner, VectorSize,
};

use crate::gallium::*;
use crate::state::refused;

/// How a packed input's word is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Packed {
    /// Its fields are two's complement.
    pub signed: bool,
    /// Its fields are fractions of their largest value; otherwise they are whole numbers.
    pub normalized: bool,
    /// Its lowest field is blue, not red.
    pub blue_first: bool,
}

/// The packed Gallium formats the interface lacks, read as one word each.
const PACKED: &[(pipe_format, Packed)] = &[
    (PIPE_FORMAT_R10G10B10A2_SNORM, packed(true, true, false)),
    (PIPE_FORMAT_R10G10B10A2_USCALED, packed(false, false, false)),
    (PIPE_FORMAT_R10G10B10A2_SSCALED, packed(true, false, false)),
    (PIPE_FORMAT_B10G10R10A2_UNORM, packed(false, true, true)),
    (PIPE_FORMAT_B10G10R10A2_SNORM, packed(true, true, true)),
    (PIPE_FORMAT_B10G10R10A2_USCALED, packed(false, false, true)),
    (PIPE_FORMAT_B10G10R10A2_SSCALED, packed(true, false, true)),
];

const fn packed(signed: bool, normalized: bool, blue_first: bool) -> Packed {
    Packed {
        signed,
        normalized,
        blue_first,
    }
}

/// How a vertex input of `format` is unpacked, where the interface lacks the format.
pub fn packed_format(format: pipe_format) -> Option<Packed> {
    PACKED
        .iter()
        .find(|(gallium, _)| *gallium == format)
        .map(|(_, p)| *p)
}

/// Bits of each color field and of alpha, lowest field first.
const COLOR_BITS: u32 = 10;
const ALPHA_BITS: u32 = 2;
const FIELD_BITS: [u32; VectorSize::MAX] = [COLOR_BITS, COLOR_BITS, COLOR_BITS, ALPHA_BITS];

/// One function's expressions and body, each expression emitted where naga emits it.
struct Builder {
    function: Function,
    body: Block,
    emitter: naga::proc::Emitter,
}

impl Builder {
    fn add(&mut self, e: Expression) -> Handle<Expression> {
        if !e.needs_pre_emit() {
            return self.function.expressions.append(e, Span::UNDEFINED);
        }
        self.flush();
        let h = self.function.expressions.append(e, Span::UNDEFINED);
        self.emitter.start(&self.function.expressions);
        h
    }

    /// Ends the open emit range.
    fn flush(&mut self) {
        if let Some((s, span)) = self.emitter.finish(&self.function.expressions) {
            self.body.push(s, span);
        }
    }

    fn u32(&mut self, v: u32) -> Handle<Expression> {
        self.add(Expression::Literal(Literal::U32(v)))
    }

    fn vec4(
        &mut self,
        module: &mut Module,
        kind: ScalarKind,
        parts: Vec<Handle<Expression>>,
    ) -> Handle<Expression> {
        let ty = vector(module, kind);
        self.add(Expression::Compose {
            ty,
            components: parts,
        })
    }

    /// The four fields of `word` as floats, as `p` reads them.
    fn unpack(
        &mut self,
        module: &mut Module,
        word: Handle<Expression>,
        p: Packed,
    ) -> Handle<Expression> {
        let kind = if p.signed {
            ScalarKind::Sint
        } else {
            ScalarKind::Uint
        };
        let word = if p.signed {
            self.add(Expression::As {
                expr: word,
                kind,
                convert: None,
            })
        } else {
            word
        };
        let mut offset = 0;
        let mut fields = Vec::with_capacity(VectorSize::MAX);
        for bits in FIELD_BITS {
            let (at, count) = (self.u32(offset), self.u32(bits));
            // A signed field's top bit is extended.
            fields.push(self.add(Expression::Math {
                fun: MathFunction::ExtractBits,
                arg: word,
                arg1: Some(at),
                arg2: Some(count),
                arg3: None,
            }));
            offset += bits;
        }
        let whole = self.vec4(module, kind, fields);
        let mut value = self.add(Expression::As {
            expr: whole,
            kind: ScalarKind::Float,
            convert: Some(size_of::<f32>() as u8),
        });
        if p.normalized {
            // The largest value of each field; a signed field's least is one below its negation,
            // which reads -1 as the least above it does.
            let largest = FIELD_BITS.map(|bits| {
                let magnitude = if p.signed { bits - 1 } else { bits };
                ((1u32 << magnitude) - 1) as f32
            });
            let parts = largest
                .iter()
                .map(|&l| self.add(Expression::Literal(Literal::F32(l))))
                .collect();
            let divisor = self.vec4(module, ScalarKind::Float, parts);
            value = self.add(Expression::Binary {
                op: naga::BinaryOperator::Divide,
                left: value,
                right: divisor,
            });
            if p.signed {
                let least = self.add(Expression::Literal(Literal::F32(-1.0)));
                let least = self.add(Expression::Splat {
                    size: VectorSize::Quad,
                    value: least,
                });
                value = self.add(Expression::Math {
                    fun: MathFunction::Max,
                    arg: value,
                    arg1: Some(least),
                    arg2: None,
                    arg3: None,
                });
            }
        }
        if p.blue_first {
            use SwizzleComponent::{W, X, Y, Z};
            value = self.add(Expression::Swizzle {
                size: VectorSize::Quad,
                vector: value,
                pattern: [Z, Y, X, W],
            });
        }
        value
    }

    /// `value`, four floats, as the program's input of type `ty` holds it: its first components,
    /// their bits where the input is of integers.
    fn fit(
        &mut self,
        module: &Module,
        value: Handle<Expression>,
        ty: Handle<Type>,
    ) -> Result<Handle<Expression>, String> {
        use SwizzleComponent::{W, X, Y, Z};
        let (fitted, kind) = match module.types[ty].inner {
            TypeInner::Scalar(Scalar { kind, .. }) => (
                self.add(Expression::AccessIndex {
                    base: value,
                    index: 0,
                }),
                kind,
            ),
            TypeInner::Vector {
                size: VectorSize::Quad,
                scalar,
            } => (value, scalar.kind),
            TypeInner::Vector { size, scalar } => (
                self.add(Expression::Swizzle {
                    size,
                    vector: value,
                    pattern: [X, Y, Z, W],
                }),
                scalar.kind,
            ),
            _ => return refused(format_args!("a packed vertex input of another type")),
        };
        Ok(if kind == ScalarKind::Float {
            fitted
        } else {
            self.add(Expression::As {
                expr: fitted,
                kind,
                convert: None,
            })
        })
    }
}

/// The vector of four scalars of `kind`.
fn vector(module: &mut Module, kind: ScalarKind) -> Handle<Type> {
    module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Vector {
                size: VectorSize::Quad,
                scalar: Scalar {
                    kind,
                    width: size_of::<u32>() as u8,
                },
            },
        },
        Span::UNDEFINED,
    )
}

/// Makes the vertex program take the inputs at the locations of `packed` as words and unpack
/// them: its entry point becomes a function the new entry point calls with the unpacked values.
pub fn unpack_inputs(module: &mut Module, packed: &[(u32, Packed)]) -> Result<(), String> {
    if packed.is_empty() {
        return Ok(());
    }
    let word = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Scalar(Scalar::U32),
        },
        Span::UNDEFINED,
    );
    let Some(entry) = module.entry_points.first_mut() else {
        return refused(format_args!("a vertex program without an entry point"));
    };
    let mut inner = std::mem::take(&mut entry.function);
    let arguments = inner.arguments.clone();
    let result = inner.result.clone();
    for a in &mut inner.arguments {
        a.binding = None;
    }
    if let Some(r) = &mut inner.result {
        r.binding = None;
    }
    let name = inner.name.take();
    let inner = module.functions.append(inner, Span::UNDEFINED);
    let mut b = Builder {
        function: Function {
            name,
            result,
            ..Function::default()
        },
        body: Block::new(),
        emitter: naga::proc::Emitter::default(),
    };
    b.emitter.start(&b.function.expressions);
    let mut values = Vec::with_capacity(arguments.len());
    for (index, a) in arguments.iter().enumerate() {
        let unpacked = match a.binding {
            Some(Binding::Location { location, .. }) => {
                packed.iter().find(|(l, _)| *l == location).map(|(_, p)| *p)
            }
            _ => None,
        };
        b.function.arguments.push(FunctionArgument {
            name: a.name.clone(),
            ty: if unpacked.is_some() { word } else { a.ty },
            binding: a.binding.clone(),
        });
        let argument = b.add(Expression::FunctionArgument(index as u32));
        values.push(match unpacked {
            Some(p) => {
                let value = b.unpack(module, argument, p);
                b.fit(module, value, a.ty)?
            }
            None => argument,
        });
    }
    b.flush();
    let returned = b.function.result.is_some().then(|| {
        b.function
            .expressions
            .append(Expression::CallResult(inner), Span::UNDEFINED)
    });
    b.body.push(
        Statement::Call {
            function: inner,
            arguments: values,
            result: returned,
        },
        Span::UNDEFINED,
    );
    b.body
        .push(Statement::Return { value: returned }, Span::UNDEFINED);
    b.function.body = b.body;
    module.entry_points[0].function = b.function;
    Ok(())
}
