//! A small builder of naga functions, for the programs this crate makes itself (the stage helper's
//! entry points and the geometry stage's emit functions): each expression is emitted where it is made.

use alloc::vec::Vec;
use naga::{
    AddressSpace, BinaryOperator, Block, Expression, Function, GlobalVariable, Handle, Literal,
    LocalVariable, Module, Range, ResourceBinding, Scalar, ScalarKind, Span, Statement, Type,
    TypeInner, VectorSize,
};

pub(crate) type Expr = Handle<Expression>;

/// The bytes of a 32-bit word, the unit storage buffers of words are addressed in.
pub(crate) const WORD: u32 = core::mem::size_of::<u32>() as u32;
/// The shift turning a byte address into a word index.
pub(crate) const WORD_SHIFT: u32 = WORD.trailing_zeros();
/// The shift turning a byte count into a bit count.
pub(crate) const BYTE_SHIFT: u32 = u8::BITS.trailing_zeros();
/// The components of a four-component vector.
pub(crate) const VEC4: u32 = VectorSize::Quad as u32;
/// The alpha component of a four-component colour.
pub(crate) const ALPHA: u32 = VEC4 - 1;

/// An f32's significand (its stored fraction and the implicit one), fraction and exponent bias.
pub(crate) const F32_SIGNIFICAND_BITS: u32 = f32::MANTISSA_DIGITS;
pub(crate) const F32_FRACTION_BITS: u32 = F32_SIGNIFICAND_BITS - 1;
pub(crate) const F32_EXPONENT_BIAS: u32 = f32::MAX_EXP as u32 - 1;
/// An f32's bits with every exponent bit set (infinity), and its fraction's bits.
pub(crate) const F32_EXPONENT_MASK: u32 = f32::INFINITY.to_bits();
pub(crate) const F32_FRACTION_MASK: u32 = (1 << F32_FRACTION_BITS) - 1;
/// An f32's sign bit.
pub(crate) const F32_SIGN_MASK: u32 = (-0.0f32).to_bits();
/// Bytes of an f32, as naga's conversions name a width.
pub(crate) const F32_WIDTH: u8 = core::mem::size_of::<f32>() as u8;
/// Exponent bits of Direct3D's half, 11- and 10-bit floats, the exponent of infinity and NaN there,
/// and their bias.
pub(crate) const SMALL_FLOAT_EXPONENT_BITS: u32 = 5;
pub(crate) const SMALL_FLOAT_EXPONENT_MASK: u32 = (1 << SMALL_FLOAT_EXPONENT_BITS) - 1;
pub(crate) const SMALL_FLOAT_EXPONENT_BIAS: u32 = SMALL_FLOAT_EXPONENT_MASK >> 1;
/// Bits of a half float, the only small float with a sign.
pub(crate) const HALF_BITS: u32 = u16::BITS;
/// The name of each program's helper constants and of their block's type.
pub(crate) const HELPERS: &str = "helpers";
pub(crate) const HELPERS_TYPE: &str = "Helpers";

/// The draw's bases at the start of the vertex stage's helper constants
/// (`HelperValue::DrawBase`), as fronts write them.
#[repr(C)]
pub(crate) struct DrawBases {
    pub base_vertex: i32,
    pub first_instance: u32,
    pub draw: u32,
}

/// Where an indirect draw's arguments are, in place of [`DrawBases`] (`HelperValue::DrawArguments`).
#[repr(C)]
pub(crate) struct DrawArguments {
    pub offset: u32,
    pub indexed: u32,
    pub draw: u32,
}

impl DrawArguments {
    /// Its members, which take the place of [`DrawBases`]' as many.
    pub const MEMBERS: usize = core::mem::size_of::<DrawArguments>() / core::mem::size_of::<u32>();
}

/// The member names of [`DrawBases`] in the block.
pub(crate) const BASE_VERTEX: &str = "base_vertex";
pub(crate) const FIRST_INSTANCE: &str = "start_instance";
pub(crate) const DRAW: &str = "draw_number";

/// The member names of [`DrawArguments`] in the block.
pub const ARGUMENTS_OFFSET: &str = "arguments_offset";
pub(crate) const INDEXED: &str = "arguments_indexed";

/// The helper block's type holding only [`DrawBases`], and its size in bytes: a whole number of
/// four-word slots, as a uniform block is laid out.
pub(crate) fn draw_bases_block(module: &mut Module) -> (Handle<Type>, u32) {
    use core::mem::offset_of;
    let mut scalar = |scalar| {
        module.types.insert(
            Type {
                name: None,
                inner: TypeInner::Scalar(scalar),
            },
            Span::UNDEFINED,
        )
    };
    let (i32_ty, u32_ty) = (scalar(Scalar::I32), scalar(Scalar::U32));
    let member = |name: &str, ty, offset: usize| naga::StructMember {
        name: Some(name.into()),
        ty,
        binding: None,
        offset: offset as u32,
    };
    let span = (core::mem::size_of::<DrawBases>() as u32).next_multiple_of(WORD * VEC4);
    let ty = module.types.insert(
        Type {
            name: Some(HELPERS_TYPE.into()),
            inner: TypeInner::Struct {
                members: alloc::vec![
                    member(BASE_VERTEX, i32_ty, offset_of!(DrawBases, base_vertex)),
                    member(
                        FIRST_INSTANCE,
                        u32_ty,
                        offset_of!(DrawBases, first_instance)
                    ),
                    member(DRAW, u32_ty, offset_of!(DrawBases, draw)),
                ],
                span,
            },
        },
        Span::UNDEFINED,
    );
    (ty, span)
}

/// Builds one function, statement by statement, into the innermost open block.
pub(crate) struct Builder {
    pub f: Function,
    blocks: Vec<Block>,
}

impl Builder {
    pub fn new(name: &str) -> Builder {
        Builder {
            f: Function {
                name: Some(name.into()),
                ..Default::default()
            },
            blocks: alloc::vec![Block::new()],
        }
    }

    fn block(&mut self) -> &mut Block {
        self.blocks.last_mut().expect("a block is open")
    }

    /// Appends an expression, emitting it where naga wants it emitted.
    pub fn expr(&mut self, e: Expression) -> Expr {
        let pre = e.needs_pre_emit()
            || matches!(
                e,
                Expression::CallResult(_) | Expression::AtomicResult { .. }
            );
        let h = self.f.expressions.append(e, Span::UNDEFINED);
        if !pre {
            self.block().push(
                Statement::Emit(Range::new_from_bounds(h, h)),
                Span::UNDEFINED,
            );
        }
        h
    }

    pub fn stmt(&mut self, s: Statement) {
        self.block().push(s, Span::UNDEFINED);
    }

    pub fn u32(&mut self, v: u32) -> Expr {
        self.expr(Expression::Literal(Literal::U32(v)))
    }

    pub fn global(&mut self, var: Handle<GlobalVariable>) -> Expr {
        self.expr(Expression::GlobalVariable(var))
    }

    pub fn local(&mut self, ty: Handle<Type>, init: Option<Expr>) -> Expr {
        let var = self.f.local_variables.append(
            LocalVariable {
                name: None,
                ty,
                init: None,
            },
            Span::UNDEFINED,
        );
        let p = self.expr(Expression::LocalVariable(var));
        if let Some(value) = init {
            self.store(p, value);
        }
        p
    }

    pub fn argument(&mut self, i: u32) -> Expr {
        self.expr(Expression::FunctionArgument(i))
    }

    pub fn load(&mut self, pointer: Expr) -> Expr {
        self.expr(Expression::Load { pointer })
    }

    pub fn store(&mut self, pointer: Expr, value: Expr) {
        self.stmt(Statement::Store { pointer, value });
    }

    pub fn binary(&mut self, op: BinaryOperator, left: Expr, right: Expr) -> Expr {
        self.expr(Expression::Binary { op, left, right })
    }

    pub fn add(&mut self, left: Expr, right: Expr) -> Expr {
        self.binary(BinaryOperator::Add, left, right)
    }

    pub fn mul(&mut self, left: Expr, right: Expr) -> Expr {
        self.binary(BinaryOperator::Multiply, left, right)
    }

    /// Element or member `index` of a value or pointer.
    pub fn field(&mut self, base: Expr, index: u32) -> Expr {
        self.expr(Expression::AccessIndex { base, index })
    }

    /// Element `index` (computed) of an array or vector value or pointer.
    pub fn at(&mut self, base: Expr, index: Expr) -> Expr {
        self.expr(Expression::Access { base, index })
    }

    /// The same bits as another scalar kind.
    pub fn bitcast(&mut self, v: Expr, kind: ScalarKind) -> Expr {
        self.expr(Expression::As {
            expr: v,
            kind,
            convert: None,
        })
    }

    pub fn select(&mut self, condition: Expr, accept: Expr, reject: Expr) -> Expr {
        self.expr(Expression::Select {
            condition,
            accept,
            reject,
        })
    }

    pub fn compose(&mut self, ty: Handle<Type>, components: Vec<Expr>) -> Expr {
        self.expr(Expression::Compose { ty, components })
    }

    pub fn call(
        &mut self,
        function: Handle<Function>,
        arguments: Vec<Expr>,
        returns: bool,
    ) -> Option<Expr> {
        let result = returns.then(|| self.expr(Expression::CallResult(function)));
        self.stmt(Statement::Call {
            function,
            arguments,
            result,
        });
        result
    }

    /// Opens a block for the statements that follow, until [`Builder::close`].
    pub fn open(&mut self) {
        self.blocks.push(Block::new());
    }

    pub fn close(&mut self) -> Block {
        self.blocks.pop().expect("an open block")
    }

    /// `if condition { accept }`, with `accept` built by the closure.
    pub fn when(&mut self, condition: Expr, accept: impl FnOnce(&mut Builder)) {
        self.open();
        accept(self);
        let accept = self.close();
        self.stmt(Statement::If {
            condition,
            accept,
            reject: Block::new(),
        });
    }

    /// `for (i = from; i < to; i++) { body(i) }` over u32 values.
    pub fn each(
        &mut self,
        module: &mut Module,
        from: Expr,
        to: Expr,
        body: impl FnOnce(&mut Builder, Expr),
    ) {
        let u32_ty = scalar(module, Scalar::U32);
        let counter = self.local(u32_ty, Some(from));
        self.open();
        let i = self.load(counter);
        let done = self.binary(BinaryOperator::GreaterEqual, i, to);
        self.when(done, |b| b.stmt(Statement::Break));
        body(self, i);
        let body = self.close();
        self.open();
        let i = self.load(counter);
        let one = self.u32(1);
        let next = self.add(i, one);
        self.store(counter, next);
        let continuing = self.close();
        self.stmt(Statement::Loop {
            body,
            continuing,
            break_if: None,
        });
    }

    /// Runs code written against a function and a block (the variant passes' style) in the open
    /// block.
    pub fn raw<R>(&mut self, g: impl FnOnce(&mut Function, &mut Block) -> R) -> R {
        let block = self.blocks.last_mut().expect("a block is open");
        g(&mut self.f, block)
    }

    pub fn ret(&mut self, value: Option<Expr>) {
        self.stmt(Statement::Return { value });
    }

    /// The finished function.
    pub fn finish(mut self) -> Function {
        self.f.body = self.close();
        self.f
    }
}

pub(crate) fn scalar(module: &mut Module, s: Scalar) -> Handle<Type> {
    module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Scalar(s),
        },
        Span::UNDEFINED,
    )
}

pub(crate) fn vec4(module: &mut Module, s: Scalar) -> Handle<Type> {
    module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Vector {
                size: VectorSize::Quad,
                scalar: s,
            },
        },
        Span::UNDEFINED,
    )
}

/// A runtime-sized array of `base`, `stride` bytes apart.
pub(crate) fn runtime_array(module: &mut Module, base: Handle<Type>, stride: u32) -> Handle<Type> {
    module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Array {
                base,
                size: naga::ArraySize::Dynamic,
                stride,
            },
        },
        Span::UNDEFINED,
    )
}

/// A global of the module, bound where `binding` says (if anywhere).
pub(crate) fn global(
    module: &mut Module,
    name: &str,
    space: AddressSpace,
    binding: Option<ResourceBinding>,
    ty: Handle<Type>,
) -> Handle<GlobalVariable> {
    module.global_variables.append(
        GlobalVariable {
            name: Some(name.into()),
            space,
            binding,
            ty,
            init: None,
            memory_decorations: Default::default(),
        },
        Span::UNDEFINED,
    )
}
