//! The rewrites a variant key asks for (plan 6.4), made on the common form after either route, so a
//! DXBC and a DXIL program get the same rewrite.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use naga::{
    AddressSpace, Binding, Block, BuiltIn, Expression, Function, FunctionArgument, FunctionResult,
    GlobalVariable, Handle, ImageClass, ImageQuery, Literal, MathFunction, Module, Range,
    SampleLevel, Scalar, Span, Statement, StorageAccess, StructMember, Type, TypeInner, VectorSize,
};
use wcontainers_gpu::{
    AddedBinding, HELPER_SET, HelperSlot, HelperValue, Reflection, ShaderStage, SlotKind,
    StageExtras, VariantFlags, VariantKey, helper_binding,
};

use crate::ConvertError;

type Result<T> = core::result::Result<T, ConvertError>;

/// Applies every rewrite of `variant` this crate builds to a converted program.
pub(crate) fn apply(
    module: &mut Module,
    stage: ShaderStage,
    variant: &VariantKey,
    reflection: &mut Reflection,
) -> Result<()> {
    constants_as_storage(module, variant.constants_as_storage, reflection)?;
    let mut outlined = Outlined::default();
    if variant.lod_bias != 0 {
        lod_bias(module, stage, variant.lod_bias, reflection, &mut outlined)?;
    }
    if variant.flags.contains(VariantFlags::TEXTURE_INFO) {
        texture_info(module, stage, reflection, &mut outlined)?;
    }
    outlined.finish(module);
    let flags = variant.flags;
    match stage {
        ShaderStage::Vertex => {
            let adjust = flags.contains(VariantFlags::VIEWPORT_ADJUST);
            let flip = flags.contains(VariantFlags::VIEWPORT_DEPTH_FLIP);
            let clip = flags.contains(VariantFlags::CLIP_DEPTH_REWRITE);
            if adjust || flip || clip {
                let viewport = adjust.then(|| {
                    let vec4 = vector(module, 4, Scalar::F32);
                    helper(
                        module,
                        reflection,
                        stage,
                        HelperValue::ViewportAdjust,
                        vec4,
                        16,
                    )
                });
                let mut w = Wrapped::new(module);
                position(&mut w, viewport, flip, clip);
                w.finish(module);
            }
        }
        ShaderStage::Fragment => {
            let counting = flags.contains(VariantFlags::COUNTING);
            let mask = flags.contains(VariantFlags::SAMPLE_MASK);
            let dual = flags.contains(VariantFlags::DUAL_SOURCE);
            if counting || mask || dual {
                let mask = mask.then(|| {
                    let u32_ty = scalar(module, Scalar::U32);
                    helper(
                        module,
                        reflection,
                        stage,
                        HelperValue::SampleMask,
                        u32_ty,
                        4,
                    )
                });
                let mut w = Wrapped::new(module);
                if dual {
                    dual_source(&mut w)?;
                }
                if let Some(mask) = mask {
                    sample_mask(module, &mut w, mask);
                }
                if counting {
                    counted(module, &mut w);
                    without_storage_writes(module);
                    if let StageExtras::Pixel { side_effects, .. } = &mut reflection.extras {
                        *side_effects = false;
                    }
                }
                w.finish(module);
            }
        }
        ShaderStage::Compute => {}
    }
    Ok(())
}

// Types and expressions.

fn scalar(module: &mut Module, s: Scalar) -> Handle<Type> {
    module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Scalar(s),
        },
        Span::UNDEFINED,
    )
}

fn vector(module: &mut Module, n: u8, s: Scalar) -> Handle<Type> {
    let size = match n {
        2 => VectorSize::Bi,
        3 => VectorSize::Tri,
        _ => VectorSize::Quad,
    };
    module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Vector { size, scalar: s },
        },
        Span::UNDEFINED,
    )
}

/// Appends `e` to `f`, emitting it in `body` where naga wants it emitted.
fn push(f: &mut Function, body: &mut Block, e: Expression) -> Handle<Expression> {
    let pre = e.needs_pre_emit() || matches!(e, Expression::CallResult(_));
    let h = f.expressions.append(e, Span::UNDEFINED);
    if !pre {
        body.push(
            Statement::Emit(Range::new_from_bounds(h, h)),
            Span::UNDEFINED,
        );
    }
    h
}

fn f32_lit(f: &mut Function, v: f32) -> Handle<Expression> {
    f.expressions
        .append(Expression::Literal(Literal::F32(v)), Span::UNDEFINED)
}

fn binary(
    f: &mut Function,
    body: &mut Block,
    op: naga::BinaryOperator,
    left: Handle<Expression>,
    right: Handle<Expression>,
) -> Handle<Expression> {
    push(f, body, Expression::Binary { op, left, right })
}

fn index(
    f: &mut Function,
    body: &mut Block,
    base: Handle<Expression>,
    i: u32,
) -> Handle<Expression> {
    push(f, body, Expression::AccessIndex { base, index: i })
}

/// Reads member `member` of the stage's helper block.
fn load_helper(
    f: &mut Function,
    body: &mut Block,
    (var, member): (Handle<GlobalVariable>, u32),
) -> Handle<Expression> {
    let g = push(f, body, Expression::GlobalVariable(var));
    let pointer = index(f, body, g, member);
    push(f, body, Expression::Load { pointer })
}

// The helper block.

/// Gives the stage's helper block (made on first use) a member for `value` of type `ty`, `size` bytes,
/// aligned as uniform blocks align it; returns the block and the member's index.
fn helper(
    module: &mut Module,
    reflection: &mut Reflection,
    stage: ShaderStage,
    value: HelperValue,
    ty: Handle<Type>,
    size: u32,
) -> (Handle<GlobalVariable>, u32) {
    let binding = naga::ResourceBinding {
        group: HELPER_SET,
        binding: helper_binding(stage),
    };
    let align = if size > 8 { 16 } else { size };
    let offset = reflection.helper_size.next_multiple_of(align);
    let found = module
        .global_variables
        .iter()
        .find(|(_, g)| g.binding == Some(binding))
        .map(|(h, _)| h);
    let mut members = match found {
        Some(g) => match &module.types[module.global_variables[g].ty].inner {
            TypeInner::Struct { members, .. } => members.clone(),
            _ => Vec::new(),
        },
        None => Vec::new(),
    };
    members.push(StructMember {
        name: Some(helper_name(value)),
        ty,
        binding: None,
        offset,
    });
    let index = members.len() as u32 - 1;
    let span = (offset + size).next_multiple_of(16);
    let block = module.types.insert(
        Type {
            name: Some("Helpers".into()),
            inner: TypeInner::Struct { members, span },
        },
        Span::UNDEFINED,
    );
    reflection.helpers.push(HelperSlot { value, offset });
    reflection.helper_size = span;
    let var = match found {
        Some(g) => {
            module.global_variables[g].ty = block;
            g
        }
        None => module.global_variables.append(
            GlobalVariable {
                name: Some("helpers".into()),
                space: AddressSpace::Uniform,
                binding: Some(binding),
                ty: block,
                init: None,
                memory_decorations: Default::default(),
            },
            Span::UNDEFINED,
        ),
    };
    (var, index)
}

fn helper_name(value: HelperValue) -> String {
    match value {
        HelperValue::LodBias { sampler } => format!("lod_bias_{sampler}"),
        HelperValue::TextureLevels { texture } => format!("levels_{texture}"),
        HelperValue::TextureSamples { texture } => format!("samples_{texture}"),
        HelperValue::SampleMask => "sample_mask".into(),
        HelperValue::ViewportAdjust => "viewport".into(),
        other => format!("{other:?}").to_lowercase(),
    }
}

// The entry point wrapped.

/// One output of the wrapped entry point.
struct Output {
    name: Option<String>,
    ty: Handle<Type>,
    binding: Binding,
    value: Handle<Expression>,
}

/// The entry point moved into a function and called by a new entry point with the same inputs, so
/// rewrites can read its inputs and replace, add or drop its outputs.
struct Wrapped {
    f: Function,
    body: Block,
    outputs: Vec<Output>,
    /// The original result: one bound value, or a struct of them.
    original: Option<(Handle<Type>, bool)>,
    changed: bool,
}

impl Wrapped {
    fn new(module: &mut Module) -> Wrapped {
        let mut inner = core::mem::take(&mut module.entry_points[0].function);
        let arguments = core::mem::take(&mut inner.arguments);
        inner.arguments = arguments
            .iter()
            .map(|a| FunctionArgument {
                binding: None,
                ..a.clone()
            })
            .collect();
        let result = inner.result.clone();
        if let Some(r) = inner.result.as_mut() {
            r.binding = None;
        }
        inner.name = Some("shader".into());
        let callee = module.functions.append(inner, Span::UNDEFINED);

        let mut f = Function {
            name: Some("main".into()),
            arguments: arguments.clone(),
            ..Default::default()
        };
        let mut body = Block::new();
        let passed = (0..arguments.len() as u32)
            .map(|i| push(&mut f, &mut body, Expression::FunctionArgument(i)))
            .collect();
        let returned = result
            .as_ref()
            .map(|_| push(&mut f, &mut body, Expression::CallResult(callee)));
        body.push(
            Statement::Call {
                function: callee,
                arguments: passed,
                result: returned,
            },
            Span::UNDEFINED,
        );
        let mut outputs = Vec::new();
        let mut original = None;
        if let (Some(r), Some(value)) = (&result, returned) {
            match (&r.binding, &module.types[r.ty].inner) {
                (Some(binding), _) => {
                    original = Some((r.ty, true));
                    outputs.push(Output {
                        name: None,
                        ty: r.ty,
                        binding: binding.clone(),
                        value,
                    });
                }
                (None, TypeInner::Struct { members, .. }) => {
                    original = Some((r.ty, false));
                    for (i, m) in members.clone().into_iter().enumerate() {
                        let v = index(&mut f, &mut body, value, i as u32);
                        outputs.push(Output {
                            name: m.name,
                            ty: m.ty,
                            binding: m.binding.expect("entry point results are bound"),
                            value: v,
                        });
                    }
                }
                _ => {}
            }
        }
        Wrapped {
            f,
            body,
            outputs,
            original,
            changed: false,
        }
    }

    fn output(&self, is: impl Fn(&Binding) -> bool) -> Option<usize> {
        self.outputs.iter().position(|o| is(&o.binding))
    }

    fn finish(mut self, module: &mut Module) {
        let f = &mut self.f;
        let value = match self.original {
            Some((_, true)) if !self.changed => Some(self.outputs[0].value),
            Some((ty, false)) if !self.changed => {
                let components = self.outputs.iter().map(|o| o.value).collect();
                Some(push(
                    f,
                    &mut self.body,
                    Expression::Compose { ty, components },
                ))
            }
            _ if self.outputs.is_empty() => None,
            _ => {
                // A new struct, laid out as naga lays out pipeline values.
                let mut offset = 0;
                let mut members = Vec::new();
                for o in &self.outputs {
                    let (size, align) = match module.types[o.ty].inner {
                        TypeInner::Vector { size, .. } => {
                            let n = size as u32;
                            (4 * n, if n == 2 { 8 } else { 16 })
                        }
                        _ => (4, 4),
                    };
                    offset = u32::next_multiple_of(offset, align);
                    members.push(StructMember {
                        name: o.name.clone(),
                        ty: o.ty,
                        binding: Some(o.binding.clone()),
                        offset,
                    });
                    offset += size;
                }
                let ty = module.types.insert(
                    Type {
                        name: Some("Outputs".into()),
                        inner: TypeInner::Struct {
                            members,
                            span: offset.next_multiple_of(16),
                        },
                    },
                    Span::UNDEFINED,
                );
                let components = self.outputs.iter().map(|o| o.value).collect();
                f.result = Some(FunctionResult { ty, binding: None });
                Some(push(
                    f,
                    &mut self.body,
                    Expression::Compose { ty, components },
                ))
            }
        };
        if !self.changed
            && let Some((ty, single)) = self.original
        {
            let binding = single.then(|| self.outputs[0].binding.clone());
            f.result = Some(FunctionResult { ty, binding });
        }
        if value.is_none() {
            f.result = None;
        }
        self.body.push(Statement::Return { value }, Span::UNDEFINED);
        f.body = self.body;
        module.entry_points[0].function = self.f;
    }
}

/// The last vertex stage's position: x and y mapped by the viewport adjustment, then z flipped for a
/// swapped depth range, then rewritten for a clip depth of -1 to 1.
fn position(
    w: &mut Wrapped,
    viewport: Option<(Handle<GlobalVariable>, u32)>,
    flip: bool,
    clip: bool,
) {
    use naga::BinaryOperator::{Add, Multiply, Subtract};
    let Some(at) = w.output(|b| matches!(b, Binding::BuiltIn(BuiltIn::Position { .. }))) else {
        return;
    };
    let ty = w.outputs[at].ty;
    let (f, body) = (&mut w.f, &mut w.body);
    let p = w.outputs[at].value;
    let mut c: Vec<_> = (0..4).map(|i| index(f, body, p, i)).collect();
    if let Some(helper) = viewport {
        // Clip x and y scaled, and offset by w, so x/w and y/w land as in the fractional viewport.
        let adjust = load_helper(f, body, helper);
        for axis in 0..2 {
            let scale = index(f, body, adjust, axis);
            let offset = index(f, body, adjust, axis + 2);
            let scaled = binary(f, body, Multiply, c[axis as usize], scale);
            let moved = binary(f, body, Multiply, offset, c[3]);
            c[axis as usize] = binary(f, body, Add, scaled, moved);
        }
    }
    if flip {
        c[2] = binary(f, body, Subtract, c[3], c[2]);
    }
    if clip {
        let two = f32_lit(f, 2.0);
        let doubled = binary(f, body, Multiply, two, c[2]);
        c[2] = binary(f, body, Subtract, doubled, c[3]);
    }
    w.outputs[at].value = push(f, body, Expression::Compose { ty, components: c });
}

/// Target 0 and 1 become the two sources of dual-source blending at location 0; other targets are
/// dropped, as Direct3D ignores them with dual-source blending.
fn dual_source(w: &mut Wrapped) -> Result<()> {
    let target = |w: &Wrapped, n: u32| {
        w.output(|b| matches!(b, Binding::Location { location, .. } if *location == n))
    };
    let (Some(first), Some(second)) = (target(w, 0), target(w, 1)) else {
        return Err(ConvertError::Unsupported("DualSourceWithoutTwoTargets"));
    };
    for (at, source) in [(first, 0), (second, 1)] {
        if let Binding::Location {
            location,
            blend_src,
            ..
        } = &mut w.outputs[at].binding
        {
            *location = 0;
            *blend_src = Some(source);
        }
    }
    w.outputs.retain(|o| match o.binding {
        Binding::Location { blend_src, .. } => blend_src.is_some(),
        _ => true,
    });
    w.changed = true;
    Ok(())
}

/// The sample mask written from the helper block, combined with the one the program writes.
fn sample_mask(module: &mut Module, w: &mut Wrapped, helper: (Handle<GlobalVariable>, u32)) {
    let mask = load_helper(&mut w.f, &mut w.body, helper);
    let is_mask = |b: &Binding| *b == Binding::BuiltIn(BuiltIn::SampleMask);
    match w.output(is_mask) {
        Some(at) => {
            let own = w.outputs[at].value;
            w.outputs[at].value =
                binary(&mut w.f, &mut w.body, naga::BinaryOperator::And, own, mask);
        }
        None => {
            let ty = scalar(module, Scalar::U32);
            w.outputs.push(Output {
                name: Some("sample_mask".into()),
                ty,
                binding: Binding::BuiltIn(BuiltIn::SampleMask),
                value: mask,
            });
            w.changed = true;
        }
    }
}

/// The counting variant's outputs: 1.0 at location 0 with target 0's alpha (for alpha to coverage),
/// the depth and sample mask the program writes, nothing else.
fn counted(module: &mut Module, w: &mut Wrapped) {
    let target0 = w.output(|b| matches!(b, Binding::Location { location: 0, .. }));
    let float4 = |ty: Handle<Type>| {
        matches!(
            module.types[ty].inner,
            TypeInner::Vector {
                size: VectorSize::Quad,
                scalar: Scalar::F32
            }
        )
    };
    let one = f32_lit(&mut w.f, 1.0);
    let alpha = match target0 {
        Some(at) if float4(w.outputs[at].ty) => {
            index(&mut w.f, &mut w.body, w.outputs[at].value, 3)
        }
        _ => one,
    };
    let ty = vector(module, 4, Scalar::F32);
    let value = push(
        &mut w.f,
        &mut w.body,
        Expression::Compose {
            ty,
            components: alloc::vec![one, one, one, alpha],
        },
    );
    w.outputs
        .retain(|o| matches!(o.binding, Binding::BuiltIn(_)));
    w.outputs.insert(
        0,
        Output {
            name: Some("count".into()),
            ty,
            binding: Binding::Location {
                location: 0,
                interpolation: None,
                sampling: None,
                blend_src: None,
                per_primitive: false,
            },
            value,
        },
    );
    w.changed = true;
}

/// Calls `f` on `block` and on every block nested in it.
pub(crate) fn each_block(block: &mut Block, f: &mut impl FnMut(&mut Block)) {
    f(block);
    for s in block.iter_mut() {
        match s {
            Statement::Block(b) => each_block(b, f),
            Statement::If { accept, reject, .. } => {
                each_block(accept, f);
                each_block(reject, f);
            }
            Statement::Switch { cases, .. } => {
                for c in cases {
                    each_block(&mut c.body, f);
                }
            }
            Statement::Loop {
                body, continuing, ..
            } => {
                each_block(body, f);
                each_block(continuing, f);
            }
            _ => {}
        }
    }
}

/// Every function of the module, entry points included.
fn each_function(module: &mut Module, mut f: impl FnMut(&mut Function)) {
    for (_, function) in module.functions.iter_mut() {
        f(function);
    }
    for ep in &mut module.entry_points {
        f(&mut ep.function);
    }
}

/// The global variable an expression of `f` names, looking through no arguments.
fn global_of(f: &Function, mut e: Handle<Expression>) -> Option<Handle<GlobalVariable>> {
    loop {
        match f.expressions[e] {
            Expression::GlobalVariable(g) => return Some(g),
            Expression::Access { base, .. } | Expression::AccessIndex { base, .. } => e = base,
            _ => return None,
        }
    }
}

/// Removes the program's writes to storage (stores, image stores and atomics), which a replay must not
/// repeat. An atomic whose result is used becomes a load of its old value.
fn without_storage_writes(module: &mut Module) {
    let storage: Vec<Handle<GlobalVariable>> = module
        .global_variables
        .iter()
        .filter(|(_, g)| {
            matches!(g.space, AddressSpace::Storage { .. })
                || matches!(
                    module.types[g.ty].inner,
                    TypeInner::Image {
                        class: ImageClass::Storage { .. },
                        ..
                    }
                )
        })
        .map(|(h, _)| h)
        .collect();
    each_function(module, |f| {
        let mut loads = Vec::new();
        let written = |f: &Function, e| global_of(f, e).is_some_and(|g| storage.contains(&g));
        let mut body = core::mem::take(&mut f.body);
        each_block(&mut body, &mut |block: &mut Block| {
            let old = core::mem::take(block);
            for (s, span) in old.span_into_iter() {
                match s {
                    Statement::Store { pointer, .. } if written(f, pointer) => {}
                    Statement::ImageStore { image, .. } if written(f, image) => {}
                    Statement::ImageAtomic { image, .. } if written(f, image) => {}
                    Statement::Atomic {
                        pointer, result, ..
                    } if written(f, pointer) => {
                        if let Some(r) = result {
                            loads.push((r, pointer));
                            block.push(Statement::Emit(Range::new_from_bounds(r, r)), span);
                        }
                    }
                    s => block.push(s, span),
                }
            }
        });
        f.body = body;
        for (r, pointer) in loads {
            f.expressions[r] = Expression::Load { pointer };
        }
    });
}

// Bindings.

/// Constant buffers read as read-only storage buffers, each moved to the storage range past the
/// highest storage binding the program uses.
fn constants_as_storage(
    module: &mut Module,
    slots: u16,
    reflection: &mut Reflection,
) -> Result<()> {
    if slots == 0 {
        return Ok(());
    }
    let mut next = reflection
        .bindings
        .iter()
        .filter(|b| b.kind == SlotKind::Storage)
        .map(|b| b.binding + b.count)
        .max()
        .unwrap_or(SlotKind::Storage.first_binding());
    for b in &mut reflection.bindings {
        if b.kind != SlotKind::Uniform || b.added.is_some() || slots & (1 << b.register) == 0 {
            continue;
        }
        if next >= SlotKind::Storage.first_binding() + SlotKind::Storage.slots() {
            return Err(ConvertError::Unsupported("BindingsBeyondLimit"));
        }
        let old = naga::ResourceBinding {
            group: b.set,
            binding: b.binding,
        };
        let (_, g) = module
            .global_variables
            .iter_mut()
            .find(|(_, g)| g.binding == Some(old))
            .ok_or(ConvertError::Malformed(
                "a constant buffer without its variable".into(),
            ))?;
        g.space = AddressSpace::Storage {
            access: StorageAccess::LOAD,
        };
        g.binding = Some(naga::ResourceBinding {
            group: b.set,
            binding: next,
        });
        b.kind = SlotKind::Storage;
        b.binding = next;
        b.added = Some(AddedBinding::ConstantsAsStorage(b.register));
        next += 1;
    }
    Ok(())
}

// Expressions replaced by calls.

/// Expressions to be replaced by calls to new functions: the functions are put ahead of every other in
/// the module's arena (callees come before callers), then each expression becomes its call's result.
#[derive(Default)]
struct Outlined {
    callees: Vec<(String, Function)>,
    sites: Vec<Site>,
}

/// An expression to be replaced by a call: which function holds it (an index into the module's
/// functions as they were, or past their count an entry point), the expression, the callee (an index
/// into [`Outlined::callees`]) and its arguments.
type Site = (usize, Handle<Expression>, usize, Vec<Handle<Expression>>);

impl Outlined {
    fn add(
        &mut self,
        owner: usize,
        h: Handle<Expression>,
        callee: Function,
        arguments: Vec<Handle<Expression>>,
    ) {
        let key = format!("{callee:?}");
        let at = match self.callees.iter().position(|(k, _)| *k == key) {
            Some(at) => at,
            None => {
                self.callees.push((key, callee));
                self.callees.len() - 1
            }
        };
        self.sites.push((owner, h, at, arguments));
    }

    fn finish(self, module: &mut Module) {
        if self.callees.is_empty() {
            return;
        }
        let old = core::mem::take(&mut module.functions);
        let count = old.len();
        let mut handles = Vec::new();
        for (_, f) in self.callees {
            handles.push(module.functions.append(f, Span::UNDEFINED));
        }
        let mut moved: Vec<Handle<naga::Function>> = Vec::new();
        for f in old.into_inner() {
            moved.push(module.functions.append(f, Span::UNDEFINED));
        }
        let remap = |h: &mut Handle<naga::Function>| *h = moved[h.index()];
        each_function(module, |f| {
            for (_, e) in f.expressions.iter_mut() {
                if let Expression::CallResult(callee) = e {
                    remap(callee);
                }
            }
            each_block(&mut f.body, &mut |b: &mut Block| {
                for s in b.iter_mut() {
                    if let Statement::Call { function, .. } = s {
                        remap(function);
                    }
                }
            });
        });
        for (owner, h, callee, arguments) in self.sites {
            let f = if owner < count {
                &mut module.functions[moved[owner]]
            } else {
                &mut module.entry_points[owner - count].function
            };
            let function = handles[callee];
            f.expressions[h] = Expression::CallResult(function);
            let mut call = Some(Statement::Call {
                function,
                arguments,
                result: Some(h),
            });
            each_block(&mut f.body, &mut |block: &mut Block| {
                if call.is_none() {
                    return;
                }
                let at = block.iter().position(|s| {
                    matches!(s, Statement::Emit(r) if r.index_range().contains(&(h.index() as u32)))
                });
                let Some(at) = at else { return };
                let Statement::Emit(range) = block[at].clone() else {
                    unreachable!()
                };
                let r = range.index_range();
                let i = h.index() as u32;
                let mut split = Block::new();
                if r.start < i {
                    split.push(
                        Statement::Emit(Range::from_index_range(r.start..i, &f.expressions)),
                        Span::UNDEFINED,
                    );
                }
                split.push(call.take().unwrap(), Span::UNDEFINED);
                if i + 1 < r.end {
                    split.push(
                        Statement::Emit(Range::from_index_range(i + 1..r.end, &f.expressions)),
                        Span::UNDEFINED,
                    );
                }
                block.splice(at..at + 1, split);
            });
        }
    }
}

/// The functions of the module in the order [`Outlined`] numbers them, with their owner number.
fn owners(module: &Module) -> Vec<(usize, &Function)> {
    module
        .functions
        .iter()
        .map(|(_, f)| f)
        .chain(module.entry_points.iter().map(|ep| &ep.function))
        .enumerate()
        .collect()
}

/// The type of each of `exprs` in `f`, added to the module's types.
fn types_of(
    module: &mut Module,
    f: &Function,
    exprs: &[Handle<Expression>],
) -> Result<Vec<Handle<Type>>> {
    let mut typifier = naga::front::Typifier::new();
    let mut resolved = Vec::new();
    {
        let ctx = naga::proc::ResolveContext::with_locals(module, &f.local_variables, &f.arguments);
        for &e in exprs {
            typifier
                .grow(e, &f.expressions, &ctx)
                .map_err(|e| ConvertError::Malformed(format!("{e}")))?;
            resolved.push(typifier[e].clone());
        }
    }
    Ok(resolved
        .into_iter()
        .map(|r| match r {
            naga::proc::TypeResolution::Handle(h) => h,
            naga::proc::TypeResolution::Value(inner) => module
                .types
                .insert(Type { name: None, inner }, Span::UNDEFINED),
        })
        .collect())
}

/// Copies a constant expression (a texel offset) from `from` into `into`.
fn copy_constant(
    from: &Function,
    e: Handle<Expression>,
    into: &mut Function,
) -> Handle<Expression> {
    let copied = match &from.expressions[e] {
        Expression::Compose { ty, components } => Expression::Compose {
            ty: *ty,
            components: components
                .iter()
                .map(|&c| copy_constant(from, c, into))
                .collect(),
        },
        Expression::Splat { size, value } => Expression::Splat {
            size: *size,
            value: copy_constant(from, *value, into),
        },
        other => other.clone(),
    };
    into.expressions.append(copied, Span::UNDEFINED)
}

/// The API register of the resource bound to global `g`, if it is of `kind`.
fn register_of(
    module: &Module,
    reflection: &Reflection,
    g: Handle<GlobalVariable>,
    kind: SlotKind,
) -> Option<u32> {
    let rb = module.global_variables[g].binding?;
    reflection
        .bindings
        .iter()
        .find(|b| {
            b.kind == kind && b.added.is_none() && b.set == rb.group && b.binding == rb.binding
        })
        .map(|b| b.register)
}

/// Samplings through the samplers of `slots` add the sampler's level-of-detail bias from the helper
/// block: an implicit level becomes a bias, a bias and an explicit level grow by it, and gradients are
/// scaled by two to its power.
fn lod_bias(
    module: &mut Module,
    stage: ShaderStage,
    slots: u16,
    reflection: &mut Reflection,
    outlined: &mut Outlined,
) -> Result<()> {
    struct Site {
        owner: usize,
        h: Handle<Expression>,
        sampler: u32,
    }
    let mut sites = Vec::new();
    for (owner, f) in owners(module) {
        for (h, e) in f.expressions.iter() {
            let Expression::ImageSample {
                sampler,
                gather: None,
                ..
            } = *e
            else {
                continue;
            };
            let g = global_of(f, sampler).ok_or(ConvertError::Unsupported("SamplerArgument"))?;
            let Some(r) = register_of(module, reflection, g, SlotKind::Sampler) else {
                continue;
            };
            if slots & (1 << r) != 0 {
                sites.push(Site {
                    owner,
                    h,
                    sampler: r,
                });
            }
        }
    }
    let mut helpers: Vec<(u32, (Handle<GlobalVariable>, u32))> = Vec::new();
    for site in sites {
        let helper_at = match helpers.iter().find(|(s, _)| *s == site.sampler) {
            Some((_, h)) => *h,
            None => {
                let f32_ty = scalar(module, Scalar::F32);
                let value = HelperValue::LodBias {
                    sampler: site.sampler,
                };
                let h = helper(module, reflection, stage, value, f32_ty, 4);
                helpers.push((site.sampler, h));
                h
            }
        };
        let f = function(module, site.owner);
        let Expression::ImageSample {
            image,
            sampler,
            coordinate,
            array_index,
            offset,
            level,
            depth_ref,
            clamp_to_edge,
            ..
        } = f.expressions[site.h].clone()
        else {
            unreachable!()
        };
        if depth_ref.is_some() {
            return Err(ConvertError::Unsupported("LodBiasOnComparison"));
        }
        let mut operands = alloc::vec![image, sampler, coordinate];
        operands.extend(array_index);
        match level {
            SampleLevel::Exact(e) | SampleLevel::Bias(e) => operands.push(e),
            SampleLevel::Gradient { x, y } => operands.extend([x, y]),
            SampleLevel::Auto | SampleLevel::Zero => {}
        }
        operands.push(site.h);
        let f = function(module, site.owner).clone();
        let types = types_of(module, &f, &operands)?;
        let result = *types.last().unwrap();
        let mut callee = Function {
            name: Some(format!("sample_biased_{}", site.sampler)),
            result: Some(FunctionResult {
                ty: result,
                binding: None,
            }),
            ..Default::default()
        };
        let mut body = Block::new();
        let n = operands.len() - 1;
        let mut args = Vec::new();
        for (i, &ty) in types[..n].iter().enumerate() {
            callee.arguments.push(FunctionArgument {
                name: None,
                ty,
                binding: None,
            });
            args.push(push(
                &mut callee,
                &mut body,
                Expression::FunctionArgument(i as u32),
            ));
        }
        let arg = |i: usize| args[i];
        let depth = matches!(
            module.types[types[0]].inner,
            TypeInner::Image {
                class: ImageClass::Depth { .. },
                ..
            }
        );
        if depth && matches!(level, SampleLevel::Zero | SampleLevel::Exact(_)) {
            return Err(ConvertError::Unsupported("LodBiasOnDepthLevel"));
        }
        let bias = load_helper(&mut callee, &mut body, helper_at);
        let next = 3 + usize::from(array_index.is_some());
        let level = match level {
            SampleLevel::Auto => SampleLevel::Bias(bias),
            SampleLevel::Zero => SampleLevel::Exact(bias),
            SampleLevel::Exact(_) => SampleLevel::Exact(binary(
                &mut callee,
                &mut body,
                naga::BinaryOperator::Add,
                arg(next),
                bias,
            )),
            SampleLevel::Bias(_) => SampleLevel::Bias(binary(
                &mut callee,
                &mut body,
                naga::BinaryOperator::Add,
                arg(next),
                bias,
            )),
            SampleLevel::Gradient { .. } => {
                let scale = push(
                    &mut callee,
                    &mut body,
                    Expression::Math {
                        fun: MathFunction::Exp2,
                        arg: bias,
                        arg1: None,
                        arg2: None,
                        arg3: None,
                    },
                );
                let mut scaled = |g: Handle<Expression>| {
                    binary(
                        &mut callee,
                        &mut body,
                        naga::BinaryOperator::Multiply,
                        g,
                        scale,
                    )
                };
                let x = scaled(arg(next));
                let y = scaled(arg(next + 1));
                SampleLevel::Gradient { x, y }
            }
        };
        if stage != ShaderStage::Fragment && matches!(level, SampleLevel::Bias(_)) {
            return Err(ConvertError::Malformed(
                "an implicit level outside a pixel shader".into(),
            ));
        }
        let offset = offset.map(|o| copy_constant(&f, o, &mut callee));
        let sample = push(
            &mut callee,
            &mut body,
            Expression::ImageSample {
                image: arg(0),
                sampler: arg(1),
                gather: None,
                coordinate: arg(2),
                array_index: array_index.map(|_| arg(3)),
                offset,
                level,
                depth_ref: None,
                clamp_to_edge,
            },
        );
        body.push(
            Statement::Return {
                value: Some(sample),
            },
            Span::UNDEFINED,
        );
        callee.body = body;
        outlined.add(site.owner, site.h, callee, operands[..n].to_vec());
    }
    Ok(())
}

/// Texture level and sample counts read from the helper block instead of asked of the texture.
fn texture_info(
    module: &mut Module,
    stage: ShaderStage,
    reflection: &mut Reflection,
    outlined: &mut Outlined,
) -> Result<()> {
    let mut sites = Vec::new();
    for (owner, f) in owners(module) {
        for (h, e) in f.expressions.iter() {
            let Expression::ImageQuery { image, query } = *e else {
                continue;
            };
            let samples = match query {
                ImageQuery::NumLevels => false,
                ImageQuery::NumSamples => true,
                _ => continue,
            };
            let g = global_of(f, image).ok_or(ConvertError::Unsupported("TextureArgument"))?;
            let r = register_of(module, reflection, g, SlotKind::Texture)
                .ok_or(ConvertError::Unsupported("TextureInfoOfStorage"))?;
            sites.push((owner, h, r, samples));
        }
    }
    let mut helpers: Vec<(HelperValue, (Handle<GlobalVariable>, u32))> = Vec::new();
    for (owner, h, texture, samples) in sites {
        let value = if samples {
            HelperValue::TextureSamples { texture }
        } else {
            HelperValue::TextureLevels { texture }
        };
        let u32_ty = scalar(module, Scalar::U32);
        let at = match helpers.iter().find(|(v, _)| *v == value) {
            Some((_, at)) => *at,
            None => {
                let at = helper(module, reflection, stage, value, u32_ty, 4);
                helpers.push((value, at));
                at
            }
        };
        let mut callee = Function {
            name: Some(helper_name(value)),
            result: Some(FunctionResult {
                ty: u32_ty,
                binding: None,
            }),
            ..Default::default()
        };
        let mut body = Block::new();
        let v = load_helper(&mut callee, &mut body, at);
        body.push(Statement::Return { value: Some(v) }, Span::UNDEFINED);
        callee.body = body;
        outlined.add(owner, h, callee, Vec::new());
    }
    Ok(())
}

fn function(module: &Module, owner: usize) -> &Function {
    let count = module.functions.len();
    if owner < count {
        module.functions.iter().nth(owner).unwrap().1
    } else {
        &module.entry_points[owner - count].function
    }
}
