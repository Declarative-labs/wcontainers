//! Runs one entry point of a program in the common form on the CPU, for the recording back end's
//! draws. It covers what drawing programs use: arithmetic, vectors and matrices, structs and arrays,
//! local variables, branches and loops, uniform and storage buffers (written by fragments), texture
//! samples and loads. Anything else is an error naming it, which the back end reports as device loss, never a
//! silent wrong answer.

use naga::{
    BinaryOperator, Expression, Function, Handle, Literal, MathFunction, Module,
    RelationalFunction, SampleLevel, ScalarKind, Statement, TypeInner, UnaryOperator,
};

pub type Error = String;

/// A value of the program.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    F32(f32),
    I32(i32),
    U32(u32),
    Bool(bool),
    /// A vector, matrix (its columns), array or struct.
    Composite(Vec<Value>),
    Pointer(Pointer),
    /// A texture or sampler, as its global.
    Resource(Handle<naga::GlobalVariable>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pointer {
    root: Root,
    path: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Root {
    Local(Handle<naga::LocalVariable>),
    Global(Handle<naga::GlobalVariable>),
}

impl Value {
    pub fn f32(&self) -> Result<f32, Error> {
        match *self {
            Value::F32(v) => Ok(v),
            _ => Err(format!("expected a float, found {self:?}")),
        }
    }

    /// The value as an index or a count.
    pub fn int(&self) -> Result<i64, Error> {
        match *self {
            Value::I32(v) => Ok(i64::from(v)),
            Value::U32(v) => Ok(i64::from(v)),
            _ => Err(format!("expected an integer, found {self:?}")),
        }
    }

    pub fn components(&self) -> Vec<Value> {
        match self {
            Value::Composite(c) => c.clone(),
            v => vec![v.clone()],
        }
    }

    /// Numeric components as f64 (integers exact), for writing outputs.
    pub fn numbers(&self) -> Result<Vec<f64>, Error> {
        self.components()
            .iter()
            .map(|c| match *c {
                Value::F32(v) => Ok(f64::from(v)),
                Value::I32(v) => Ok(f64::from(v)),
                Value::U32(v) => Ok(f64::from(v)),
                Value::Bool(v) => Ok(f64::from(u8::from(v))),
                _ => Err(format!("expected numbers, found {c:?}")),
            })
            .collect()
    }
}

/// What a texture read asks for.
#[derive(Clone, Copy, Debug)]
pub enum Level {
    /// Implicit (only exact where the view has one level).
    Auto,
    Exact(f32),
    Bias(f32),
}

/// The resources bound to a draw, as the program sees them by group and binding.
pub trait Resources {
    /// The bytes of the buffer range bound at (group, binding).
    fn buffer(&self, group: u32, binding: u32) -> Result<&[u8], Error>;
    /// Samples the texture at `image` with the sampler at `sampler`; `compare` is a depth reference.
    fn sample(
        &self,
        image: (u32, u32),
        sampler: (u32, u32),
        coord: [f32; 3],
        layer: Option<i64>,
        level: Level,
        compare: Option<f32>,
    ) -> Result<[f64; 4], Error>;
    /// Loads one texel, or one sample of a multisampled texel.
    fn load(
        &self,
        image: (u32, u32),
        coord: [i64; 3],
        layer: Option<i64>,
        level: i64,
        sample: Option<i64>,
    ) -> Result<[f64; 4], Error>;
    /// The size of a level of the texture.
    fn size(&self, image: (u32, u32), level: i64) -> Result<[u32; 3], Error>;
}

/// A program's write into a storage buffer: group, binding, byte offset in the bound range, bytes.
pub type StorageWrite = (u32, u32, u32, Vec<u8>);

/// Where a pointer into a buffer leads.
enum Place {
    Typed(Handle<naga::Type>, u32),
    Scalar(ScalarKind, u32),
    /// A matrix column whose vector type the module does not declare.
    Column(usize, u32),
}

/// How a function ended.
enum Flow {
    Next,
    Break,
    Continue,
    Return(Option<Value>),
    Kill,
}

/// One running invocation of an entry point.
pub struct Invocation<'a, R: Resources> {
    module: &'a Module,
    module_info: &'a naga::valid::ModuleInfo,
    info: &'a naga::valid::FunctionInfo,
    fun: &'a Function,
    args: Vec<Value>,
    locals: Vec<Value>,
    cache: Vec<Option<Value>>,
    resources: &'a R,
    writes: &'a mut Vec<StorageWrite>,
}

/// Runs entry point `index` with its arguments; None when the invocation was discarded. Its writes into
/// storage buffers are added to `writes`, in order; the invocation's own loads see them.
pub fn run<R: Resources>(
    module: &Module,
    info: &naga::valid::ModuleInfo,
    index: usize,
    args: Vec<Value>,
    resources: &R,
    writes: &mut Vec<StorageWrite>,
) -> Result<Option<Value>, Error> {
    let fun = &module.entry_points[index].function;
    let fun_info = info.get_entry_point(index);
    invoke(module, info, fun, fun_info, args, resources, writes)
}

/// Runs one function with its arguments; None when the invocation was discarded.
fn invoke<R: Resources>(
    module: &Module,
    module_info: &naga::valid::ModuleInfo,
    fun: &Function,
    info: &naga::valid::FunctionInfo,
    args: Vec<Value>,
    resources: &R,
    writes: &mut Vec<StorageWrite>,
) -> Result<Option<Value>, Error> {
    let mut inv = Invocation {
        module,
        module_info,
        info,
        fun,
        args,
        locals: Vec::new(),
        cache: vec![None; fun.expressions.len()],
        resources,
        writes,
    };
    for (_, local) in fun.local_variables.iter() {
        let v = match local.init {
            Some(init) => inv.eval(init)?,
            None => zero(module, local.ty)?,
        };
        inv.locals.push(v);
    }
    match inv.block(&fun.body)? {
        Flow::Return(v) => Ok(Some(v.unwrap_or(Value::Composite(Vec::new())))),
        Flow::Kill => Ok(None),
        Flow::Next => Ok(Some(Value::Composite(Vec::new()))),
        Flow::Break | Flow::Continue => Err("break outside a loop".into()),
    }
}

/// The zero value of a type.
pub fn zero(module: &Module, ty: Handle<naga::Type>) -> Result<Value, Error> {
    Ok(match module.types[ty].inner {
        TypeInner::Scalar(s) => scalar_zero(s.kind)?,
        TypeInner::Vector { size, scalar } => {
            Value::Composite(vec![scalar_zero(scalar.kind)?; size as usize])
        }
        TypeInner::Matrix {
            columns,
            rows,
            scalar,
        } => Value::Composite(vec![
            Value::Composite(vec![
                scalar_zero(scalar.kind)?;
                rows as usize
            ]);
            columns as usize
        ]),
        TypeInner::Array { base, size, .. } => {
            let naga::ArraySize::Constant(n) = size else {
                return Err("zero value of a runtime-sized array".into());
            };
            Value::Composite(vec![zero(module, base)?; n.get() as usize])
        }
        TypeInner::Struct { ref members, .. } => Value::Composite(
            members
                .iter()
                .map(|m| zero(module, m.ty))
                .collect::<Result<_, _>>()?,
        ),
        ref other => return Err(format!("zero value of {other:?}")),
    })
}

fn scalar_zero(kind: ScalarKind) -> Result<Value, Error> {
    Ok(match kind {
        ScalarKind::Float => Value::F32(0.0),
        ScalarKind::Sint => Value::I32(0),
        ScalarKind::Uint => Value::U32(0),
        ScalarKind::Bool => Value::Bool(false),
        k => return Err(format!("scalar kind {k:?}")),
    })
}

fn literal(l: Literal) -> Result<Value, Error> {
    Ok(match l {
        Literal::F32(v) => Value::F32(v),
        Literal::I32(v) => Value::I32(v),
        Literal::U32(v) => Value::U32(v),
        Literal::Bool(v) => Value::Bool(v),
        Literal::AbstractInt(v) => Value::I32(v as i32),
        Literal::AbstractFloat(v) => Value::F32(v as f32),
        other => return Err(format!("literal {other:?}")),
    })
}

/// Evaluates a module-scope constant expression.
fn constant(module: &Module, h: Handle<Expression>) -> Result<Value, Error> {
    Ok(match module.global_expressions[h] {
        Expression::Literal(l) => literal(l)?,
        Expression::Constant(c) => constant(module, module.constants[c].init)?,
        Expression::ZeroValue(ty) => zero(module, ty)?,
        Expression::Compose { ty, ref components } => {
            let parts = components
                .iter()
                .map(|&c| constant(module, c))
                .collect::<Result<Vec<_>, _>>()?;
            compose(module, ty, parts)
        }
        Expression::Splat { size, value } => {
            Value::Composite(vec![constant(module, value)?; size as usize])
        }
        ref other => return Err(format!("constant expression {other:?}")),
    })
}

/// Builds a composite; vector components that are vectors are flattened.
fn compose(module: &Module, ty: Handle<naga::Type>, parts: Vec<Value>) -> Value {
    if let TypeInner::Vector { .. } = module.types[ty].inner {
        Value::Composite(parts.iter().flat_map(Value::components).collect())
    } else {
        Value::Composite(parts)
    }
}

/// Applies `f` to each component pair, broadcasting a scalar against a vector.
fn zip(
    a: &Value,
    b: &Value,
    f: &dyn Fn(&Value, &Value) -> Result<Value, Error>,
) -> Result<Value, Error> {
    match (a, b) {
        (Value::Composite(x), Value::Composite(y)) => Ok(Value::Composite(
            x.iter()
                .zip(y)
                .map(|(x, y)| zip(x, y, f))
                .collect::<Result<_, _>>()?,
        )),
        (Value::Composite(x), s) => Ok(Value::Composite(
            x.iter().map(|x| zip(x, s, f)).collect::<Result<_, _>>()?,
        )),
        (s, Value::Composite(y)) => Ok(Value::Composite(
            y.iter().map(|y| zip(s, y, f)).collect::<Result<_, _>>()?,
        )),
        (x, y) => f(x, y),
    }
}

fn map(a: &Value, f: &dyn Fn(&Value) -> Result<Value, Error>) -> Result<Value, Error> {
    match a {
        Value::Composite(x) => Ok(Value::Composite(
            x.iter().map(|x| map(x, f)).collect::<Result<_, _>>()?,
        )),
        x => f(x),
    }
}

fn float_fn(a: &Value, f: impl Fn(f32) -> f32) -> Result<Value, Error> {
    map(a, &|x| match *x {
        Value::F32(v) => Ok(Value::F32(f(v))),
        Value::I32(v) => Ok(Value::I32(f(v as f32) as i32)),
        ref o => Err(format!("float function of {o:?}")),
    })
}

fn binary(op: BinaryOperator, x: &Value, y: &Value) -> Result<Value, Error> {
    use BinaryOperator as B;
    Ok(match (x, y) {
        (&Value::F32(a), &Value::F32(b)) => match op {
            B::Add => Value::F32(a + b),
            B::Subtract => Value::F32(a - b),
            B::Multiply => Value::F32(a * b),
            B::Divide => Value::F32(a / b),
            B::Modulo => Value::F32(a - b * (a / b).trunc()),
            B::Equal => Value::Bool(a == b),
            B::NotEqual => Value::Bool(a != b),
            B::Less => Value::Bool(a < b),
            B::LessEqual => Value::Bool(a <= b),
            B::Greater => Value::Bool(a > b),
            B::GreaterEqual => Value::Bool(a >= b),
            _ => return Err(format!("{op:?} on floats")),
        },
        (&Value::I32(a), &Value::I32(b)) => match op {
            B::Add => Value::I32(a.wrapping_add(b)),
            B::Subtract => Value::I32(a.wrapping_sub(b)),
            B::Multiply => Value::I32(a.wrapping_mul(b)),
            B::Divide => Value::I32(if b == 0 { a } else { a.wrapping_div(b) }),
            B::Modulo => Value::I32(if b == 0 { 0 } else { a.wrapping_rem(b) }),
            B::Equal => Value::Bool(a == b),
            B::NotEqual => Value::Bool(a != b),
            B::Less => Value::Bool(a < b),
            B::LessEqual => Value::Bool(a <= b),
            B::Greater => Value::Bool(a > b),
            B::GreaterEqual => Value::Bool(a >= b),
            B::And => Value::I32(a & b),
            B::InclusiveOr => Value::I32(a | b),
            B::ExclusiveOr => Value::I32(a ^ b),
            _ => return Err(format!("{op:?} on signed integers")),
        },
        (&Value::I32(a), &Value::U32(b)) => match op {
            B::ShiftLeft => Value::I32(a.wrapping_shl(b)),
            B::ShiftRight => Value::I32(a.wrapping_shr(b)),
            _ => return Err(format!("{op:?} on a signed and an unsigned integer")),
        },
        (&Value::U32(a), &Value::U32(b)) => match op {
            B::Add => Value::U32(a.wrapping_add(b)),
            B::Subtract => Value::U32(a.wrapping_sub(b)),
            B::Multiply => Value::U32(a.wrapping_mul(b)),
            B::Divide => Value::U32(a.checked_div(b).unwrap_or(a)),
            B::Modulo => Value::U32(a.checked_rem(b).unwrap_or(0)),
            B::Equal => Value::Bool(a == b),
            B::NotEqual => Value::Bool(a != b),
            B::Less => Value::Bool(a < b),
            B::LessEqual => Value::Bool(a <= b),
            B::Greater => Value::Bool(a > b),
            B::GreaterEqual => Value::Bool(a >= b),
            B::And => Value::U32(a & b),
            B::InclusiveOr => Value::U32(a | b),
            B::ExclusiveOr => Value::U32(a ^ b),
            B::ShiftLeft => Value::U32(a.wrapping_shl(b)),
            B::ShiftRight => Value::U32(a.wrapping_shr(b)),
            _ => return Err(format!("{op:?} on unsigned integers")),
        },
        (&Value::Bool(a), &Value::Bool(b)) => match op {
            B::Equal => Value::Bool(a == b),
            B::NotEqual => Value::Bool(a != b),
            B::LogicalAnd | B::And => Value::Bool(a && b),
            B::LogicalOr | B::InclusiveOr => Value::Bool(a || b),
            _ => return Err(format!("{op:?} on booleans")),
        },
        _ => return Err(format!("{op:?} on {x:?} and {y:?}")),
    })
}

fn dot(a: &Value, b: &Value) -> Result<f32, Error> {
    let (a, b) = (a.components(), b.components());
    a.iter().zip(&b).map(|(x, y)| Ok(x.f32()? * y.f32()?)).sum()
}

impl<R: Resources> Invocation<'_, R> {
    fn kind(&self, h: Handle<Expression>) -> &TypeInner {
        self.info[h].ty.inner_with(&self.module.types)
    }

    fn block(&mut self, block: &naga::Block) -> Result<Flow, Error> {
        for statement in block.iter() {
            let flow = self.statement(statement)?;
            if !matches!(flow, Flow::Next) {
                return Ok(flow);
            }
        }
        Ok(Flow::Next)
    }

    fn statement(&mut self, s: &Statement) -> Result<Flow, Error> {
        match *s {
            Statement::Emit(ref range) => {
                for h in range.clone() {
                    let v = self.compute(h)?;
                    self.cache[h.index()] = Some(v);
                }
            }
            Statement::Block(ref b) => return self.block(b),
            Statement::If {
                condition,
                ref accept,
                ref reject,
            } => {
                let Value::Bool(c) = self.eval(condition)? else {
                    return Err("if on a non-boolean".into());
                };
                return self.block(if c { accept } else { reject });
            }
            Statement::Loop {
                ref body,
                ref continuing,
                break_if,
            } => loop {
                match self.block(body)? {
                    Flow::Break => break,
                    f @ (Flow::Return(_) | Flow::Kill) => return Ok(f),
                    Flow::Next | Flow::Continue => {}
                }
                if let f @ (Flow::Return(_) | Flow::Kill) = self.block(continuing)? {
                    return Ok(f);
                }
                if let Some(b) = break_if
                    && self.eval(b)? == Value::Bool(true)
                {
                    break;
                }
            },
            Statement::Break => return Ok(Flow::Break),
            Statement::Continue => return Ok(Flow::Continue),
            Statement::Return { value } => {
                let v = value.map(|v| self.eval(v)).transpose()?;
                return Ok(Flow::Return(v));
            }
            Statement::Kill => return Ok(Flow::Kill),
            Statement::Call {
                function,
                ref arguments,
                result,
            } => {
                let args = arguments
                    .iter()
                    .map(|&a| self.eval(a))
                    .collect::<Result<Vec<_>, _>>()?;
                // A pointer names a local of the function it was made in.
                if args.iter().any(|a| {
                    matches!(
                        a,
                        Value::Pointer(Pointer {
                            root: Root::Local(_),
                            ..
                        })
                    )
                }) {
                    return Err("a pointer to a local passed to a function".into());
                }
                let (module, module_info, resources) =
                    (self.module, self.module_info, self.resources);
                let returned = invoke(
                    module,
                    module_info,
                    &module.functions[function],
                    &module_info[function],
                    args,
                    resources,
                    self.writes,
                )?;
                let Some(v) = returned else {
                    return Ok(Flow::Kill);
                };
                if let Some(r) = result {
                    self.cache[r.index()] = Some(v);
                }
            }
            Statement::Store { pointer, value } => {
                let value = self.eval(value)?;
                let Value::Pointer(p) = self.eval(pointer)? else {
                    return Err("store through a non-pointer".into());
                };
                let local = match p.root {
                    Root::Local(local) => local,
                    Root::Global(g) => {
                        let var = &self.module.global_variables[g];
                        let naga::AddressSpace::Storage { access } = var.space else {
                            return Err("store into a global that is not a storage buffer".into());
                        };
                        if !access.contains(naga::StorageAccess::STORE) {
                            return Err("store into a read-only storage buffer".into());
                        }
                        let b = var.binding.as_ref().ok_or("a buffer without a binding")?;
                        let (group, binding) = (b.group, b.binding);
                        let mut bytes = Vec::new();
                        let at = match self.place(&p)? {
                            Place::Typed(ty, at) => {
                                write_typed(self.module, ty, &value, &mut bytes)?;
                                at
                            }
                            Place::Scalar(_, at) | Place::Column(_, at) => {
                                for c in value.components() {
                                    bytes.extend_from_slice(&scalar_bits(&c)?.to_le_bytes());
                                }
                                at
                            }
                        };
                        let bound = self.resources.buffer(group, binding)?.len();
                        if at as usize + bytes.len() > bound {
                            return Err("a store past the bound range".into());
                        }
                        self.writes.push((group, binding, at, bytes));
                        return Ok(Flow::Next);
                    }
                };
                let mut slot = &mut self.locals[local.index()];
                for &i in &p.path {
                    let Value::Composite(c) = slot else {
                        return Err("store path into a scalar".into());
                    };
                    slot = &mut c[i as usize];
                }
                *slot = value;
            }
            ref other => return Err(format!("statement {other:?}")),
        }
        Ok(Flow::Next)
    }

    fn eval(&mut self, h: Handle<Expression>) -> Result<Value, Error> {
        match &self.cache[h.index()] {
            Some(v) => Ok(v.clone()),
            None => self.compute(h),
        }
    }

    fn index_of(&mut self, h: Handle<Expression>) -> Result<u32, Error> {
        let i = self.eval(h)?.int()?;
        u32::try_from(i).map_err(|_| format!("negative index {i}"))
    }

    fn compute(&mut self, h: Handle<Expression>) -> Result<Value, Error> {
        let module = self.module;
        Ok(match self.fun.expressions[h] {
            Expression::Literal(l) => literal(l)?,
            Expression::Constant(c) => constant(module, module.constants[c].init)?,
            Expression::ZeroValue(ty) => zero(module, ty)?,
            Expression::Compose { ty, ref components } => {
                let parts = components
                    .iter()
                    .map(|&c| self.eval(c))
                    .collect::<Result<Vec<_>, _>>()?;
                compose(module, ty, parts)
            }
            Expression::Access { base, index } => {
                let i = self.index_of(index)?;
                self.element(base, i)?
            }
            Expression::AccessIndex { base, index } => self.element(base, index)?,
            Expression::Splat { size, value } => {
                Value::Composite(vec![self.eval(value)?; size as usize])
            }
            Expression::Swizzle {
                size,
                vector,
                pattern,
            } => {
                let v = self.eval(vector)?.components();
                Value::Composite(
                    pattern[..size as usize]
                        .iter()
                        .map(|&c| v[c as usize].clone())
                        .collect(),
                )
            }
            Expression::FunctionArgument(i) => self.args[i as usize].clone(),
            Expression::GlobalVariable(g) => match module.global_variables[g].space {
                naga::AddressSpace::Handle => Value::Resource(g),
                _ => Value::Pointer(Pointer {
                    root: Root::Global(g),
                    path: Vec::new(),
                }),
            },
            Expression::LocalVariable(l) => Value::Pointer(Pointer {
                root: Root::Local(l),
                path: Vec::new(),
            }),
            Expression::Load { pointer } => {
                let Value::Pointer(p) = self.eval(pointer)? else {
                    return Err("load through a non-pointer".into());
                };
                self.load(&p)?
            }
            Expression::Unary { op, expr } => {
                let v = self.eval(expr)?;
                map(&v, &|x| {
                    Ok(match (op, x) {
                        (UnaryOperator::Negate, &Value::F32(a)) => Value::F32(-a),
                        (UnaryOperator::Negate, &Value::I32(a)) => Value::I32(a.wrapping_neg()),
                        (UnaryOperator::LogicalNot, &Value::Bool(a)) => Value::Bool(!a),
                        (UnaryOperator::BitwiseNot, &Value::I32(a)) => Value::I32(!a),
                        (UnaryOperator::BitwiseNot, &Value::U32(a)) => Value::U32(!a),
                        _ => return Err(format!("{op:?} on {x:?}")),
                    })
                })?
            }
            Expression::Binary { op, left, right } => {
                let (l, r) = (self.eval(left)?, self.eval(right)?);
                let (lm, rm) = (
                    matches!(self.kind(left), TypeInner::Matrix { .. }),
                    matches!(self.kind(right), TypeInner::Matrix { .. }),
                );
                let rv = matches!(self.kind(right), TypeInner::Vector { .. });
                let lv = matches!(self.kind(left), TypeInner::Vector { .. });
                if op == BinaryOperator::Multiply && (lm || rm) {
                    matrix_multiply(&l, &r, lm, rm, lv, rv)?
                } else {
                    zip(&l, &r, &|x, y| binary(op, x, y))?
                }
            }
            Expression::Select {
                condition,
                accept,
                reject,
            } => {
                let (c, a, r) = (
                    self.eval(condition)?,
                    self.eval(accept)?,
                    self.eval(reject)?,
                );
                match c {
                    Value::Bool(c) => {
                        if c {
                            a
                        } else {
                            r
                        }
                    }
                    Value::Composite(c) => Value::Composite(
                        c.iter()
                            .zip(a.components().into_iter().zip(r.components()))
                            .map(|(c, (a, r))| if *c == Value::Bool(true) { a } else { r })
                            .collect(),
                    ),
                    _ => return Err("select on a non-boolean".into()),
                }
            }
            Expression::Relational { fun, argument } => {
                let v = self.eval(argument)?.components();
                let t = v.iter().filter(|b| **b == Value::Bool(true)).count();
                match fun {
                    RelationalFunction::All => Value::Bool(t == v.len()),
                    RelationalFunction::Any => Value::Bool(t > 0),
                    other => return Err(format!("relational {other:?}")),
                }
            }
            Expression::Math {
                fun,
                arg,
                arg1,
                arg2,
                ..
            } => {
                let a = self.eval(arg)?;
                let b = arg1.map(|e| self.eval(e)).transpose()?;
                let c = arg2.map(|e| self.eval(e)).transpose()?;
                math(fun, &a, b.as_ref(), c.as_ref())?
            }
            Expression::As {
                expr,
                kind,
                convert,
            } => {
                let v = self.eval(expr)?;
                map(&v, &|x| cast(x, kind, convert.is_some()))?
            }
            Expression::ImageSample {
                image,
                sampler,
                gather: None,
                coordinate,
                array_index,
                offset: None,
                level,
                depth_ref,
                ..
            } => {
                let image = self.binding_of(image)?;
                let sampler = self.binding_of(sampler)?;
                let coord = self.eval(coordinate)?.components();
                let mut c = [0.0f32; 3];
                for (i, v) in coord.iter().enumerate().take(3) {
                    c[i] = v.f32()?;
                }
                let layer = array_index.map(|a| self.eval(a)?.int()).transpose()?;
                let level = match level {
                    SampleLevel::Auto => Level::Auto,
                    SampleLevel::Zero => Level::Exact(0.0),
                    SampleLevel::Exact(e) => Level::Exact(self.eval(e)?.f32()?),
                    SampleLevel::Bias(e) => Level::Bias(self.eval(e)?.f32()?),
                    SampleLevel::Gradient { .. } => {
                        return Err("sampling with gradients".into());
                    }
                };
                let compare = depth_ref.map(|d| self.eval(d)?.f32()).transpose()?;
                let texel = self
                    .resources
                    .sample(image, sampler, c, layer, level, compare)?;
                if compare.is_some() {
                    Value::F32(texel[0] as f32)
                } else {
                    Value::Composite(texel.iter().map(|&v| Value::F32(v as f32)).collect())
                }
            }
            Expression::ImageLoad {
                image,
                coordinate,
                array_index,
                sample,
                level,
            } => {
                let kind = match *self.kind(image) {
                    TypeInner::Image {
                        class: naga::ImageClass::Sampled { kind, .. },
                        ..
                    } => kind,
                    TypeInner::Image {
                        class: naga::ImageClass::Depth { .. },
                        ..
                    } => ScalarKind::Float,
                    ref other => return Err(format!("load from {other:?}")),
                };
                let depth = matches!(
                    *self.kind(image),
                    TypeInner::Image {
                        class: naga::ImageClass::Depth { .. },
                        ..
                    }
                );
                let image = self.binding_of(image)?;
                let coord = self.eval(coordinate)?.components();
                let mut c = [0i64; 3];
                for (i, v) in coord.iter().enumerate().take(3) {
                    c[i] = v.int()?;
                }
                let layer = array_index.map(|a| self.eval(a)?.int()).transpose()?;
                let level = level.map(|l| self.eval(l)?.int()).transpose()?.unwrap_or(0);
                let sample = sample.map(|n| self.eval(n)?.int()).transpose()?;
                let texel = self.resources.load(image, c, layer, level, sample)?;
                if depth {
                    Value::F32(texel[0] as f32)
                } else {
                    Value::Composite(
                        texel
                            .iter()
                            .map(|&v| match kind {
                                ScalarKind::Uint => Value::U32(v as u32),
                                ScalarKind::Sint => Value::I32(v as i32),
                                _ => Value::F32(v as f32),
                            })
                            .collect(),
                    )
                }
            }
            Expression::ImageQuery {
                image,
                query: naga::ImageQuery::Size { level },
            } => {
                let dims = match *self.kind(image) {
                    TypeInner::Image { dim, .. } => match dim {
                        naga::ImageDimension::D1 => 1,
                        naga::ImageDimension::D3 => 3,
                        _ => 2,
                    },
                    _ => return Err("size of a non-image".into()),
                };
                let image = self.binding_of(image)?;
                let level = level.map(|l| self.eval(l)?.int()).transpose()?.unwrap_or(0);
                let size = self.resources.size(image, level)?;
                if dims == 1 {
                    Value::U32(size[0])
                } else {
                    Value::Composite(size[..dims].iter().map(|&v| Value::U32(v)).collect())
                }
            }
            ref other => return Err(format!("expression {other:?}")),
        })
    }

    /// The (group, binding) of a texture or sampler expression.
    fn binding_of(&mut self, h: Handle<Expression>) -> Result<(u32, u32), Error> {
        let Value::Resource(g) = self.eval(h)? else {
            return Err("a texture or sampler that is not a global".into());
        };
        let b = self.module.global_variables[g]
            .binding
            .as_ref()
            .ok_or("a resource without a binding")?;
        Ok((b.group, b.binding))
    }

    fn element(&mut self, base: Handle<Expression>, i: u32) -> Result<Value, Error> {
        Ok(match self.eval(base)? {
            Value::Pointer(mut p) => {
                p.path.push(i);
                Value::Pointer(p)
            }
            Value::Composite(c) => c
                .get(i as usize)
                .cloned()
                .ok_or_else(|| format!("index {i} out of range"))?,
            other => return Err(format!("element {i} of {other:?}")),
        })
    }

    fn load(&self, p: &Pointer) -> Result<Value, Error> {
        match p.root {
            Root::Local(l) => {
                let mut v = &self.locals[l.index()];
                for &i in &p.path {
                    let Value::Composite(c) = v else {
                        return Err("load path into a scalar".into());
                    };
                    v = &c[i as usize];
                }
                Ok(v.clone())
            }
            Root::Global(g) => {
                let var = &self.module.global_variables[g];
                let b = var.binding.as_ref().ok_or("a buffer without a binding")?;
                let mut bytes = self.resources.buffer(b.group, b.binding)?;
                // The invocation's own earlier writes, laid over the bound range.
                let written;
                if self
                    .writes
                    .iter()
                    .any(|w| (w.0, w.1) == (b.group, b.binding))
                {
                    let mut copy = bytes.to_vec();
                    for (_, _, at, w) in self
                        .writes
                        .iter()
                        .filter(|w| (w.0, w.1) == (b.group, b.binding))
                    {
                        copy.get_mut(*at as usize..*at as usize + w.len())
                            .ok_or("a write past the bound range")?
                            .copy_from_slice(w);
                    }
                    written = copy;
                    bytes = &written;
                }
                match self.place(p)? {
                    Place::Typed(ty, at) => read_typed(self.module, ty, bytes, at),
                    Place::Scalar(kind, at) => read_scalar(kind, bytes, at),
                    Place::Column(rows, at) => read_vector(rows, bytes, at),
                }
            }
        }
    }

    /// Where a pointer into a buffer leads, from the start of the bound range.
    fn place(&self, p: &Pointer) -> Result<Place, Error> {
        let Root::Global(g) = p.root else {
            return Err("a local is not in a buffer".into());
        };
        let (mut ty, mut offset) = (self.module.global_variables[g].ty, 0u32);
        for &i in &p.path {
            (ty, offset) = match self.module.types[ty].inner {
                TypeInner::Struct { ref members, .. } => {
                    (members[i as usize].ty, offset + members[i as usize].offset)
                }
                TypeInner::Array { base, stride, .. } => (base, offset + stride * i),
                TypeInner::Vector { scalar, .. } => {
                    return Ok(Place::Scalar(scalar.kind, offset + 4 * i));
                }
                TypeInner::Matrix { rows, scalar, .. } => {
                    let column = self.module.types.iter().find_map(|(h, t)| {
                        (t.inner == TypeInner::Vector { size: rows, scalar }).then_some(h)
                    });
                    let stride = if rows == naga::VectorSize::Bi { 8 } else { 16 };
                    match column {
                        Some(c) => (c, offset + stride * i),
                        None => return Ok(Place::Column(rows as usize, offset + stride * i)),
                    }
                }
                ref other => return Err(format!("path into {other:?}")),
            };
        }
        Ok(Place::Typed(ty, offset))
    }
}

/// The bits of a scalar as a buffer holds them.
fn scalar_bits(v: &Value) -> Result<u32, Error> {
    Ok(match *v {
        Value::F32(x) => x.to_bits(),
        Value::I32(x) => x as u32,
        Value::U32(x) => x,
        Value::Bool(x) => u32::from(x),
        ref other => return Err(format!("writing {other:?} as a scalar")),
    })
}

/// Lays out a scalar or vector value of type `ty` as a buffer holds it.
fn write_typed(
    module: &Module,
    ty: Handle<naga::Type>,
    value: &Value,
    out: &mut Vec<u8>,
) -> Result<(), Error> {
    match module.types[ty].inner {
        TypeInner::Scalar(_) | TypeInner::Vector { .. } => {
            for c in value.components() {
                out.extend_from_slice(&scalar_bits(&c)?.to_le_bytes());
            }
        }
        ref other => return Err(format!("writing {other:?} into a buffer in one store")),
    }
    Ok(())
}

fn read_scalar(kind: ScalarKind, bytes: &[u8], at: u32) -> Result<Value, Error> {
    let at = at as usize;
    let w = bytes
        .get(at..at + 4)
        .ok_or_else(|| format!("read at {at} past the bound range"))?;
    let w = u32::from_le_bytes([w[0], w[1], w[2], w[3]]);
    Ok(match kind {
        ScalarKind::Float => Value::F32(f32::from_bits(w)),
        ScalarKind::Sint => Value::I32(w as i32),
        ScalarKind::Uint => Value::U32(w),
        ScalarKind::Bool => Value::Bool(w != 0),
        k => return Err(format!("scalar kind {k:?} in a buffer")),
    })
}

fn read_vector(n: usize, bytes: &[u8], at: u32) -> Result<Value, Error> {
    Ok(Value::Composite(
        (0..n as u32)
            .map(|i| read_scalar(ScalarKind::Float, bytes, at + 4 * i))
            .collect::<Result<_, _>>()?,
    ))
}

/// Reads a value of type `ty` laid out in a buffer at `at`.
fn read_typed(
    module: &Module,
    ty: Handle<naga::Type>,
    bytes: &[u8],
    at: u32,
) -> Result<Value, Error> {
    Ok(match module.types[ty].inner {
        TypeInner::Scalar(s) => read_scalar(s.kind, bytes, at)?,
        TypeInner::Vector { size, scalar } => Value::Composite(
            (0..size as u32)
                .map(|i| read_scalar(scalar.kind, bytes, at + 4 * i))
                .collect::<Result<_, _>>()?,
        ),
        TypeInner::Matrix { columns, rows, .. } => {
            let stride = if rows == naga::VectorSize::Bi { 8 } else { 16 };
            Value::Composite(
                (0..columns as u32)
                    .map(|c| read_vector(rows as usize, bytes, at + stride * c))
                    .collect::<Result<_, _>>()?,
            )
        }
        TypeInner::Array {
            base,
            size: naga::ArraySize::Constant(n),
            stride,
        } => Value::Composite(
            (0..n.get())
                .map(|i| read_typed(module, base, bytes, at + stride * i))
                .collect::<Result<_, _>>()?,
        ),
        TypeInner::Struct { ref members, .. } => Value::Composite(
            members
                .iter()
                .map(|m| read_typed(module, m.ty, bytes, at + m.offset))
                .collect::<Result<_, _>>()?,
        ),
        ref other => return Err(format!("reading {other:?} from a buffer")),
    })
}

fn matrix_multiply(
    l: &Value,
    r: &Value,
    lm: bool,
    rm: bool,
    lv: bool,
    rv: bool,
) -> Result<Value, Error> {
    let columns = |m: &Value| m.components();
    Ok(match (lm, rm) {
        // Matrix times vector: the sum of the columns weighted by the vector.
        (true, false) if rv => {
            let cols = columns(l);
            let v = r.components();
            let rows = cols[0].components().len();
            let mut out = vec![0.0f32; rows];
            for (c, s) in cols.iter().zip(&v) {
                for (o, x) in out.iter_mut().zip(c.components()) {
                    *o += x.f32()? * s.f32()?;
                }
            }
            Value::Composite(out.into_iter().map(Value::F32).collect())
        }
        // Vector times matrix: the dot product with each column.
        (false, true) if lv => Value::Composite(
            columns(r)
                .iter()
                .map(|c| Ok(Value::F32(dot(l, c)?)))
                .collect::<Result<_, Error>>()?,
        ),
        (true, true) => Value::Composite(
            columns(r)
                .iter()
                .map(|c| matrix_multiply(l, c, true, false, false, true))
                .collect::<Result<_, _>>()?,
        ),
        // Matrix times scalar.
        _ => zip(l, r, &|x, y| binary(BinaryOperator::Multiply, x, y))?,
    })
}

fn cast(x: &Value, kind: ScalarKind, convert: bool) -> Result<Value, Error> {
    let bits = |v: &Value| -> Result<u32, Error> {
        Ok(match *v {
            Value::F32(f) => f.to_bits(),
            Value::I32(i) => i as u32,
            Value::U32(u) => u,
            Value::Bool(b) => u32::from(b),
            _ => return Err("cast of a composite".into()),
        })
    };
    Ok(if convert {
        match (kind, x) {
            (ScalarKind::Float, &Value::F32(v)) => Value::F32(v),
            (ScalarKind::Float, &Value::I32(v)) => Value::F32(v as f32),
            (ScalarKind::Float, &Value::U32(v)) => Value::F32(v as f32),
            (ScalarKind::Float, &Value::Bool(v)) => Value::F32(f32::from(u8::from(v))),
            // Float to integer saturates, as WGSL says.
            (ScalarKind::Sint, &Value::F32(v)) => Value::I32(v as i32),
            (ScalarKind::Uint, &Value::F32(v)) => Value::U32(v as u32),
            (ScalarKind::Sint, v) => Value::I32(bits(v)? as i32),
            (ScalarKind::Uint, v) => Value::U32(bits(v)?),
            (ScalarKind::Bool, v) => Value::Bool(bits(v)? != 0 && *v != Value::F32(-0.0)),
            (k, _) => return Err(format!("conversion to {k:?}")),
        }
    } else {
        let b = bits(x)?;
        match kind {
            ScalarKind::Float => Value::F32(f32::from_bits(b)),
            ScalarKind::Sint => Value::I32(b as i32),
            ScalarKind::Uint => Value::U32(b),
            k => return Err(format!("bitcast to {k:?}")),
        }
    })
}

fn math(
    fun: MathFunction,
    a: &Value,
    b: Option<&Value>,
    c: Option<&Value>,
) -> Result<Value, Error> {
    use MathFunction as M;
    let b = || b.ok_or_else(|| format!("{fun:?} needs a second argument"));
    let c = || c.ok_or_else(|| format!("{fun:?} needs a third argument"));
    let pair = |f: fn(f32, f32) -> f32| -> Result<Value, Error> {
        zip(a, b()?, &|x, y| Ok(Value::F32(f(x.f32()?, y.f32()?))))
    };
    Ok(match fun {
        M::Abs => map(a, &|x| {
            Ok(match *x {
                Value::F32(v) => Value::F32(v.abs()),
                Value::I32(v) => Value::I32(v.wrapping_abs()),
                ref o => o.clone(),
            })
        })?,
        M::Min | M::Max => zip(a, b()?, &|x, y| {
            let less = binary(BinaryOperator::Less, x, y)? == Value::Bool(true);
            Ok(if less == (fun == M::Min) {
                x.clone()
            } else {
                y.clone()
            })
        })?,
        M::Clamp => {
            let lo = math(M::Max, a, Some(b()?), None)?;
            math(M::Min, &lo, Some(c()?), None)?
        }
        M::Saturate => float_fn(a, |v| v.clamp(0.0, 1.0))?,
        M::Floor => float_fn(a, f32::floor)?,
        M::Ceil => float_fn(a, f32::ceil)?,
        M::Round => float_fn(a, f32::round_ties_even)?,
        M::Fract => float_fn(a, |v| v - v.floor())?,
        M::Trunc => float_fn(a, f32::trunc)?,
        M::Sqrt => float_fn(a, f32::sqrt)?,
        M::InverseSqrt => float_fn(a, |v| 1.0 / v.sqrt())?,
        M::Sin => float_fn(a, f32::sin)?,
        M::Cos => float_fn(a, f32::cos)?,
        M::Tan => float_fn(a, f32::tan)?,
        M::Exp => float_fn(a, f32::exp)?,
        M::Exp2 => float_fn(a, f32::exp2)?,
        M::Log => float_fn(a, f32::ln)?,
        M::Log2 => float_fn(a, f32::log2)?,
        M::Sign => float_fn(a, |v| if v == 0.0 { 0.0 } else { v.signum() })?,
        M::Pow => pair(f32::powf)?,
        M::Step => pair(|edge, x| if x < edge { 0.0 } else { 1.0 })?,
        M::Dot => Value::F32(dot(a, b()?)?),
        M::Length => Value::F32(dot(a, a)?.sqrt()),
        M::Distance => {
            let d = zip(a, b()?, &|x, y| binary(BinaryOperator::Subtract, x, y))?;
            Value::F32(dot(&d, &d)?.sqrt())
        }
        M::Normalize => {
            let l = dot(a, a)?.sqrt();
            float_fn(a, |v| v / l)?
        }
        M::Cross => {
            let (x, y) = (a.numbers()?, b()?.numbers()?);
            let f = |i: usize, j: usize| (x[i] * y[j] - x[j] * y[i]) as f32;
            Value::Composite(vec![
                Value::F32(f(1, 2)),
                Value::F32(f(2, 0)),
                Value::F32(f(0, 1)),
            ])
        }
        M::Mix => {
            let t = c()?;
            let d = zip(b()?, a, &|y, x| binary(BinaryOperator::Subtract, y, x))?;
            let s = zip(&d, t, &|d, t| binary(BinaryOperator::Multiply, d, t))?;
            zip(a, &s, &|x, s| binary(BinaryOperator::Add, x, s))?
        }
        M::Fma => {
            let p = zip(a, b()?, &|x, y| binary(BinaryOperator::Multiply, x, y))?;
            zip(&p, c()?, &|p, z| binary(BinaryOperator::Add, p, z))?
        }
        M::SmoothStep => {
            let (e0, e1) = (a, b()?);
            let x = c()?;
            let t = zip(
                &zip(x, e0, &|x, e| binary(BinaryOperator::Subtract, x, e))?,
                &zip(e1, e0, &|x, e| binary(BinaryOperator::Subtract, x, e))?,
                &|n, d| binary(BinaryOperator::Divide, n, d),
            )?;
            float_fn(&t, |t| {
                let t = t.clamp(0.0, 1.0);
                t * t * (3.0 - 2.0 * t)
            })?
        }
        other => return Err(format!("math function {other:?}")),
    })
}
