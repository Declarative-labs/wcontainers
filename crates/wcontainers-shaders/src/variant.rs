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
    AddedBinding, CLAMPED_SAMPLER_BINDING, DRAW_ARGUMENTS_BINDING, DrawBaseSource, HELPER_SET,
    HelperSlot, HelperValue, LinkedValues, Reflection, ShaderStage, SlotKind, StageExtras,
    VariantFlags, VariantKey, helper_binding,
};

use crate::ConvertError;

mod constants;
mod filter;
pub(crate) mod pull;
pub(crate) mod texel;

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
    constants::constants_as_texture(
        module,
        variant.constants_as_texture,
        reflection,
        &mut outlined,
    )?;
    if variant.draw_base == DrawBaseSource::Arguments {
        draw_arguments(module, reflection, &mut outlined)?;
    }
    let mut clamps = Vec::new();
    if variant.lod_bias != 0
        || variant.min_lod_clamped != 0
        || variant.mirror_once_in_shader != 0
        || variant.filter_in_shader != 0
        || variant.border_in_shader != 0
    {
        samples(
            module,
            stage,
            variant,
            reflection,
            &mut outlined,
            &mut clamps,
        )?;
    }
    if variant.min_lod_clamped != 0 {
        loads(
            module,
            stage,
            variant,
            reflection,
            &mut outlined,
            &mut clamps,
        )?;
    }
    if variant.flags.contains(VariantFlags::TEXTURE_INFO) {
        texture_info(module, stage, reflection, &mut outlined)?;
    }
    outlined.finish(module);
    if variant.constants_as_texture != 0 {
        // Drops the pointers into the buffers' old variables, and the variables.
        naga::compact::compact(module, naga::compact::KeepUnused::No);
    }
    let flags = variant.flags;
    match stage {
        ShaderStage::Vertex => {
            let adjust = flags.contains(VariantFlags::VIEWPORT_ADJUST);
            let flip = flags.contains(VariantFlags::VIEWPORT_DEPTH_FLIP);
            let clip = flags.contains(VariantFlags::CLIP_DEPTH_REWRITE);
            let distances = flags.contains(VariantFlags::CLIP_DISTANCE);
            let clamp = flags.contains(VariantFlags::DEPTH_CLAMP);
            let pulls = !variant.pulled.is_empty();
            if adjust || flip || clip || distances || clamp || pulls {
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
                pull::pulled(module, &mut w, &variant.pulled, reflection)?;
                let z = position(&mut w, viewport, flip, clip, clamp);
                if distances || clamp {
                    linked_out(module, &mut w, variant.linked, distances, z)?;
                }
                w.finish(module);
            }
        }
        ShaderStage::Fragment => {
            let counting = flags.contains(VariantFlags::COUNTING);
            let mask = flags.contains(VariantFlags::SAMPLE_MASK);
            let dual = flags.contains(VariantFlags::DUAL_SOURCE);
            let bias = flags.contains(VariantFlags::DEPTH_BIAS);
            let distances =
                flags.contains(VariantFlags::CLIP_DISTANCE) && variant.linked.clip_distances > 0;
            let clamp = flags.contains(VariantFlags::DEPTH_CLAMP);
            if counting || mask || dual || bias || distances || clamp {
                let bias = bias.then(|| {
                    let vec4 = vector(module, 4, Scalar::F32);
                    helper(module, reflection, stage, HelperValue::DepthBias, vec4, 16)
                });
                let range = clamp.then(|| {
                    let vec2 = vector(module, 2, Scalar::F32);
                    helper(module, reflection, stage, HelperValue::DepthRange, vec2, 8)
                });
                let per_sample = matches!(
                    reflection.extras,
                    StageExtras::Pixel {
                        per_sample: true,
                        ..
                    }
                );
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
                let linked = if distances || clamp {
                    linked_in(module, &mut w, variant.linked, clamp)
                } else {
                    Vec::new()
                };
                let count = variant.linked.clip_distances as usize;
                if distances {
                    clipped(
                        module,
                        &mut w,
                        &linked[..count],
                        variant.samples,
                        per_sample,
                    )?;
                    if let StageExtras::Pixel { discards, .. } = &mut reflection.extras {
                        *discards = true;
                    }
                }
                if dual {
                    dual_source(&mut w)?;
                }
                if let Some(mask) = mask {
                    let mask = load_helper(&mut w.f, &mut w.body, mask);
                    sample_mask(module, &mut w, mask);
                }
                if bias.is_some() || clamp {
                    let clamp = range.map(|range| (linked[linked.len() - 1], range));
                    depth(module, &mut w, bias, clamp);
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

/// A member of the stage's helper block: the block and the member's index.
type HelperMember = (Handle<GlobalVariable>, u32);

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
    // Packed after the last member's end, not the block's padded size, as std140 packs a uniform
    // block (GLSL ES reads the block by those rules, not by the offsets the module gives).
    let end = members
        .iter()
        .map(|m| m.offset + module.types[m.ty].inner.size(module.to_ctx()))
        .max()
        .unwrap_or(0);
    let align = if size > 8 { 16 } else { size };
    let offset = end.next_multiple_of(align);
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

/// The member of the stage's helper block for `value`, made by [`helper`] when it is not there yet.
fn helper_once(
    module: &mut Module,
    reflection: &mut Reflection,
    stage: ShaderStage,
    value: HelperValue,
    ty: Handle<Type>,
    size: u32,
) -> HelperMember {
    if reflection.helpers.iter().any(|s| s.value == value) {
        let binding = naga::ResourceBinding {
            group: HELPER_SET,
            binding: helper_binding(stage),
        };
        let name = helper_name(value);
        let found = module
            .global_variables
            .iter()
            .find(|(_, g)| g.binding == Some(binding))
            .and_then(|(g, var)| match &module.types[var.ty].inner {
                TypeInner::Struct { members, .. } => members
                    .iter()
                    .position(|m| m.name.as_deref() == Some(name.as_str()))
                    .map(|i| (g, i as u32)),
                _ => None,
            });
        if let Some(at) = found {
            return at;
        }
    }
    helper(module, reflection, stage, value, ty, size)
}

fn helper_name(value: HelperValue) -> String {
    match value {
        HelperValue::LodBias { sampler } => format!("lod_bias_{sampler}"),
        HelperValue::BorderColor { sampler } => format!("border_{sampler}"),
        HelperValue::SamplerState { sampler } => format!("sampler_{sampler}"),
        HelperValue::TextureLevels { texture } => format!("levels_{texture}"),
        HelperValue::TextureSamples { texture } => format!("samples_{texture}"),
        HelperValue::MinLod { texture } => format!("min_lod_{texture}"),
        HelperValue::PulledStream { input } => format!("stream_{input}"),
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
    /// Statements run before the wrapped program, which compute from its inputs alone.
    prologue: Block,
    /// Where the call of the wrapped program is in `body`.
    call_at: usize,
    outputs: Vec<Output>,
    /// The original result: one bound value, or a struct of them.
    original: Option<(Handle<Type>, bool)>,
    changed: bool,
    /// Inputs the prologue computes instead: the argument's index and its value.
    replaced: Vec<(u32, Handle<Expression>)>,
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
        let call_at = body.len();
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
            prologue: Block::new(),
            call_at,
            outputs,
            original,
            changed: false,
            replaced: Vec::new(),
        }
    }

    /// The entry point's argument bound at `location` and its type. An input inside a struct is
    /// refused: neither route makes one for a vertex program.
    fn input_at(&self, module: &Module, location: u32) -> Result<Option<(u32, Handle<Type>)>> {
        let at = |b: &Binding| matches!(b, Binding::Location { location: l, .. } if *l == location);
        for (i, a) in self.f.arguments.iter().enumerate() {
            if a.binding.as_ref().is_some_and(at) {
                return Ok(Some((i as u32, a.ty)));
            }
            if let TypeInner::Struct { members, .. } = &module.types[a.ty].inner
                && members.iter().any(|m| m.binding.as_ref().is_some_and(at))
            {
                return Err(ConvertError::Unsupported("PulledInputInStruct"));
            }
        }
        Ok(None)
    }

    /// Passes the wrapped program `value` for argument `argument`, which the entry point then no longer
    /// takes.
    fn replace(&mut self, argument: u32, value: Handle<Expression>) {
        self.replaced.push((argument, value));
    }

    /// Makes the replaced inputs' values the call's arguments and drops them from the entry point.
    fn drop_replaced(&mut self) {
        if self.replaced.is_empty() {
            return;
        }
        if let Statement::Call { arguments, .. } = &mut self.body[self.call_at] {
            for &(i, value) in &self.replaced {
                arguments[i as usize] = value;
            }
        }
        let mut removed: Vec<u32> = self.replaced.iter().map(|r| r.0).collect();
        removed.sort_unstable();
        let types: Vec<_> = removed
            .iter()
            .map(|&i| self.f.arguments[i as usize].ty)
            .collect();
        for &i in removed.iter().rev() {
            self.f.arguments.remove(i as usize);
        }
        for (_, e) in self.f.expressions.iter_mut() {
            if let Expression::FunctionArgument(j) = *e {
                *e = match removed.binary_search(&j) {
                    // No longer read: the call takes the computed value.
                    Ok(k) => Expression::ZeroValue(types[k]),
                    Err(k) => Expression::FunctionArgument(j - k as u32),
                };
            }
        }
    }

    fn output(&self, is: impl Fn(&Binding) -> bool) -> Option<usize> {
        self.outputs.iter().position(|o| is(&o.binding))
    }

    /// The entry point's input bound as `is` says, read in the prologue; made an argument of type
    /// `ty` bound by `binding` where the program has none.
    fn input(
        &mut self,
        module: &Module,
        is: impl Fn(&Binding) -> bool,
        binding: Binding,
        ty: Handle<Type>,
    ) -> Handle<Expression> {
        let (f, prologue) = (&mut self.f, &mut self.prologue);
        for i in 0..f.arguments.len() {
            let a = &f.arguments[i];
            if a.binding.as_ref().is_some_and(&is) {
                return push(f, prologue, Expression::FunctionArgument(i as u32));
            }
            if let TypeInner::Struct { members, .. } = &module.types[a.ty].inner
                && let Some(m) = members
                    .iter()
                    .position(|m| m.binding.as_ref().is_some_and(&is))
            {
                let s = push(f, prologue, Expression::FunctionArgument(i as u32));
                return index(f, prologue, s, m as u32);
            }
        }
        f.arguments.push(FunctionArgument {
            name: None,
            ty,
            binding: Some(binding),
        });
        let i = f.arguments.len() as u32 - 1;
        push(f, prologue, Expression::FunctionArgument(i))
    }

    fn finish(mut self, module: &mut Module) {
        self.drop_replaced();
        let prologue = core::mem::take(&mut self.prologue);
        self.body.splice(self.call_at..self.call_at, prologue);
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
        // The wrapped program's own result no longer leaves the stage: its members lose their
        // bindings, so an output a rewrite replaced (clip distances) asks nothing of the language.
        if self.changed
            && let Some((ty, false)) = self.original
            && let TypeInner::Struct { members, span } = &module.types[ty].inner
        {
            let unbound = Type {
                name: module.types[ty].name.clone(),
                inner: TypeInner::Struct {
                    members: members
                        .iter()
                        .map(|m| StructMember {
                            binding: None,
                            ..m.clone()
                        })
                        .collect(),
                    span: *span,
                },
            };
            if module.types.get(&unbound).is_none() {
                module.types.replace(ty, unbound);
            }
        }
        if value.is_none() {
            f.result = None;
        }
        self.body.push(Statement::Return { value }, Span::UNDEFINED);
        f.body = self.body;
        module.entry_points[0].function = self.f;
    }
}

/// The last vertex stage's position: x and y mapped by the viewport adjustment, z kept inside the
/// clip volume when `clamp` (the unclamped z is returned), then flipped for a swapped depth range,
/// then rewritten for a clip depth of -1 to 1.
fn position(
    w: &mut Wrapped,
    viewport: Option<(Handle<GlobalVariable>, u32)>,
    flip: bool,
    clip: bool,
    clamp: bool,
) -> Option<Handle<Expression>> {
    use naga::BinaryOperator::{Add, Multiply, Subtract};
    let at = w.output(|b| matches!(b, Binding::BuiltIn(BuiltIn::Position { .. })))?;
    let ty = w.outputs[at].ty;
    let (f, body) = (&mut w.f, &mut w.body);
    let p = w.outputs[at].value;
    let mut c: Vec<_> = (0..4).map(|i| index(f, body, p, i)).collect();
    let unclamped = c[2];
    if clamp {
        // Direct3D's clip volume holds 0 <= z <= w; a point with w below 0 is outside it anyway.
        let below = math(f, body, MathFunction::Min, [c[2], c[3]]);
        let zero = f32_lit(f, 0.0);
        c[2] = math(f, body, MathFunction::Max, [below, zero]);
    }
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
    clamp.then_some(unclamped)
}

/// A perspective-correct four-component f32 value at `location`.
fn linked_binding(location: u32) -> Binding {
    Binding::Location {
        location,
        interpolation: Some(naga::Interpolation::Perspective),
        sampling: Some(naga::Sampling::Center),
        blend_src: None,
        per_primitive: false,
    }
}

/// The vertex program's added values (`VariantKey::linked`): its clip distances, taken from the
/// built-in when `distances`, then the unclamped clip z.
fn linked_out(
    module: &mut Module,
    w: &mut Wrapped,
    linked: LinkedValues,
    distances: bool,
    z: Option<Handle<Expression>>,
) -> Result<()> {
    let mut values = Vec::new();
    if distances && let Some(at) = w.output(|b| *b == Binding::BuiltIn(BuiltIn::ClipDistances)) {
        let o = w.outputs.remove(at);
        let TypeInner::Array {
            size: naga::ArraySize::Constant(n),
            ..
        } = module.types[o.ty].inner
        else {
            return Err(ConvertError::Malformed(
                "clip distances not an array".into(),
            ));
        };
        for i in 0..n.get() {
            values.push(index(&mut w.f, &mut w.body, o.value, i));
        }
        w.changed = true;
    }
    if values.len() != usize::from(if distances { linked.clip_distances } else { 0 }) {
        return Err(ConvertError::Unsupported("LinkedClipDistanceCount"));
    }
    let taken = w.outputs.iter().any(
        |o| matches!(o.binding, Binding::Location { location, .. } if location >= linked.location),
    );
    if taken {
        return Err(ConvertError::Unsupported("LinkedLocationTaken"));
    }
    values.extend(z);
    let vec4 = vector(module, 4, Scalar::F32);
    let zero = f32_lit(&mut w.f, 0.0);
    for (k, chunk) in values.chunks(4).enumerate() {
        let mut components = chunk.to_vec();
        components.resize(4, zero);
        let value = push(
            &mut w.f,
            &mut w.body,
            Expression::Compose {
                ty: vec4,
                components,
            },
        );
        w.outputs.push(Output {
            name: Some(format!("linked{k}")),
            ty: vec4,
            binding: linked_binding(linked.location + k as u32),
            value,
        });
        w.changed = true;
    }
    Ok(())
}

/// The pixel program's added values, read as the vertex program writes them: its clip distances
/// (`linked.clip_distances` of them), then the unclamped clip z when `clamp`, as scalars.
fn linked_in(
    module: &mut Module,
    w: &mut Wrapped,
    linked: LinkedValues,
    clamp: bool,
) -> Vec<Handle<Expression>> {
    let count = usize::from(linked.clip_distances) + usize::from(clamp);
    let vec4 = vector(module, 4, Scalar::F32);
    let mut values = Vec::new();
    for k in 0..count.div_ceil(4) as u32 {
        let location = linked.location + k;
        let v = w.input(
            module,
            |b| matches!(b, Binding::Location { location: l, .. } if *l == location),
            linked_binding(location),
            vec4,
        );
        for i in 0..4 {
            if values.len() < count {
                values.push(index(&mut w.f, &mut w.prologue, v, i));
            }
        }
    }
    values
}

/// Direct3D's standard positions of four samples, in sixteenths of a pixel from its centre.
const FOUR_SAMPLES: [(f32, f32); 4] = [(-2.0, -6.0), (6.0, -2.0), (-6.0, 2.0), (2.0, 6.0)];

/// Clipping by the clip distances `d`, before the program runs: one sample discards where any is
/// negative at the pixel's centre; four take each distance over w (affine on the screen) to each
/// sample from its derivatives, keep the samples where none is negative in the sample mask, and
/// discard where none is kept.
fn clipped(
    module: &mut Module,
    w: &mut Wrapped,
    d: &[Handle<Expression>],
    samples: u8,
    per_sample: bool,
) -> Result<()> {
    use naga::BinaryOperator::{Add, Equal, GreaterEqual, InclusiveOr, Less, LogicalAnd, Multiply};
    let mask = match samples {
        0 => return Err(ConvertError::Unsupported("SampleCount")),
        1 => None,
        4 if per_sample => return Err(ConvertError::Unsupported("ClipDistancePerSample")),
        4 => {
            let vec4 = vector(module, 4, Scalar::F32);
            let position = w.input(
                module,
                |b| matches!(b, Binding::BuiltIn(BuiltIn::Position { .. })),
                Binding::BuiltIn(BuiltIn::Position { invariant: false }),
                vec4,
            );
            let (f, body) = (&mut w.f, &mut w.prologue);
            let inverse_w = index(f, body, position, 3);
            let sloped: Vec<_> = d
                .iter()
                .map(|&d| {
                    let q = binary(f, body, Multiply, d, inverse_w);
                    let [dx, dy] = [naga::DerivativeAxis::X, naga::DerivativeAxis::Y].map(|axis| {
                        push(
                            f,
                            body,
                            Expression::Derivative {
                                axis,
                                ctrl: naga::DerivativeControl::None,
                                expr: q,
                            },
                        )
                    });
                    (q, dx, dy)
                })
                .collect();
            let zero = f32_lit(f, 0.0);
            let mut mask = f
                .expressions
                .append(Expression::Literal(Literal::U32(0)), Span::UNDEFINED);
            for (s, (x, y)) in FOUR_SAMPLES.iter().enumerate() {
                let (x, y) = (f32_lit(f, x / 16.0), f32_lit(f, y / 16.0));
                let mut kept = None;
                for &(q, dx, dy) in &sloped {
                    let along_x = binary(f, body, Multiply, dx, x);
                    let along_y = binary(f, body, Multiply, dy, y);
                    let moved = binary(f, body, Add, q, along_x);
                    let at = binary(f, body, Add, moved, along_y);
                    let inside = binary(f, body, GreaterEqual, at, zero);
                    kept = Some(match kept {
                        Some(k) => binary(f, body, LogicalAnd, k, inside),
                        None => inside,
                    });
                }
                let bit = f
                    .expressions
                    .append(Expression::Literal(Literal::U32(1 << s)), Span::UNDEFINED);
                let none = f
                    .expressions
                    .append(Expression::Literal(Literal::U32(0)), Span::UNDEFINED);
                let chosen = push(
                    f,
                    body,
                    Expression::Select {
                        condition: kept.expect("clip distances"),
                        accept: bit,
                        reject: none,
                    },
                );
                mask = binary(f, body, InclusiveOr, mask, chosen);
            }
            Some(mask)
        }
        _ => return Err(ConvertError::Unsupported("ClipDistanceSamples")),
    };
    let (f, body) = (&mut w.f, &mut w.prologue);
    let outside = match mask {
        Some(mask) => {
            let none = f
                .expressions
                .append(Expression::Literal(Literal::U32(0)), Span::UNDEFINED);
            binary(f, body, Equal, mask, none)
        }
        None => {
            let zero = f32_lit(f, 0.0);
            let mut outside = None;
            for &d in d {
                let negative = binary(f, body, Less, d, zero);
                outside = Some(match outside {
                    Some(o) => binary(f, body, naga::BinaryOperator::LogicalOr, o, negative),
                    None => negative,
                });
            }
            outside.expect("clip distances")
        }
    };
    let mut accept = Block::new();
    accept.push(Statement::Kill, Span::UNDEFINED);
    body.push(
        Statement::If {
            condition: outside,
            accept,
            reject: Block::new(),
        },
        Span::UNDEFINED,
    );
    if let Some(mask) = mask {
        sample_mask(module, w, mask);
    }
    Ok(())
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

/// The sample mask `mask` written, combined with the one the program writes.
fn sample_mask(module: &mut Module, w: &mut Wrapped, mask: Handle<Expression>) {
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

/// The fragment's depth written by the program: with `clamp` (the unclamped clip z the vertex program
/// passed, and the depth range in the helper block), the window depth made from it and clamped to the
/// range; with `bias`, Direct3D's depth bias added before the clamp. A depth the program writes
/// itself is only clamped, as the fixed function biases only the rasterised depth.
fn depth(
    module: &mut Module,
    w: &mut Wrapped,
    bias: Option<(Handle<GlobalVariable>, u32)>,
    clamp: Option<(Handle<Expression>, HelperMember)>,
) {
    use naga::BinaryOperator::{Add, Multiply, Subtract};
    let within = |f: &mut Function, body: &mut Block, z, range| {
        let range = load_helper(f, body, range);
        let [near, far] = [0, 1].map(|i| index(f, body, range, i));
        let low = math(f, body, MathFunction::Min, [near, far]);
        let high = math(f, body, MathFunction::Max, [near, far]);
        let z = math(f, body, MathFunction::Max, [z, low]);
        math(f, body, MathFunction::Min, [z, high])
    };
    if let Some(at) = w.output(|b| *b == Binding::BuiltIn(BuiltIn::FragDepth)) {
        if let Some((_, range)) = clamp {
            let own = w.outputs[at].value;
            w.outputs[at].value = within(&mut w.f, &mut w.body, own, range);
        }
        return;
    }
    let vec4 = vector(module, 4, Scalar::F32);
    let position = w.input(
        module,
        |b| matches!(b, Binding::BuiltIn(BuiltIn::Position { .. })),
        Binding::BuiltIn(BuiltIn::Position { invariant: false }),
        vec4,
    );
    let (f, body) = (&mut w.f, &mut w.prologue);
    let mut z = match clamp {
        Some((unclamped, range)) => {
            // z over w, affine on the screen, then the viewport's depth mapping.
            let inverse_w = index(f, body, position, 3);
            let z = binary(f, body, Multiply, unclamped, inverse_w);
            let range = load_helper(f, body, range);
            let [near, far] = [0, 1].map(|i| index(f, body, range, i));
            let span = binary(f, body, Subtract, far, near);
            let scaled = binary(f, body, Multiply, span, z);
            binary(f, body, Add, near, scaled)
        }
        None => index(f, body, position, 2),
    };
    if let Some(bias) = bias {
        z = biased(f, body, z, bias);
    }
    if let Some((_, range)) = clamp {
        z = within(f, body, z, range);
    }
    let ty = scalar(module, Scalar::F32);
    w.outputs.push(Output {
        name: Some("depth".into()),
        ty,
        binding: Binding::BuiltIn(BuiltIn::FragDepth),
        value: z,
    });
    w.changed = true;
}

/// The window depth `z` plus Direct3D's depth bias: constant units times the format's step plus the
/// slope scale times the larger window depth slope, clamped as the bias clamp's sign says.
fn biased(
    f: &mut Function,
    body: &mut Block,
    z: Handle<Expression>,
    helper: (Handle<GlobalVariable>, u32),
) -> Handle<Expression> {
    use naga::BinaryOperator::{Add, Greater, Less, Multiply};
    let bias = load_helper(f, body, helper);
    let [units, scale, clamp, step] = [0, 1, 2, 3].map(|i| index(f, body, bias, i));
    let slopes = [naga::DerivativeAxis::X, naga::DerivativeAxis::Y].map(|axis| {
        let d = push(
            f,
            body,
            Expression::Derivative {
                axis,
                ctrl: naga::DerivativeControl::None,
                expr: z,
            },
        );
        math(f, body, MathFunction::Abs, [d, d])
    });
    let slope = math(f, body, MathFunction::Max, slopes);
    let constant = binary(f, body, Multiply, units, step);
    let sloped = binary(f, body, Multiply, scale, slope);
    let raw = binary(f, body, Add, constant, sloped);
    let zero = f32_lit(f, 0.0);
    let bounded = [
        math(f, body, MathFunction::Min, [raw, clamp]),
        math(f, body, MathFunction::Max, [raw, clamp]),
    ];
    let positive = binary(f, body, Greater, clamp, zero);
    let negative = binary(f, body, Less, clamp, zero);
    let clamped = push(
        f,
        body,
        Expression::Select {
            condition: negative,
            accept: bounded[1],
            reject: raw,
        },
    );
    let clamped = push(
        f,
        body,
        Expression::Select {
            condition: positive,
            accept: bounded[0],
            reject: clamped,
        },
    );
    binary(f, body, Add, z, clamped)
}

/// A math function of one or two arguments (the second ignored for one-argument functions).
fn math(
    f: &mut Function,
    body: &mut Block,
    fun: MathFunction,
    [arg, arg1]: [Handle<Expression>; 2],
) -> Handle<Expression> {
    let arg1 = (fun.argument_count() > 1).then_some(arg1);
    push(
        f,
        body,
        Expression::Math {
            fun,
            arg,
            arg1,
            arg2: None,
            arg3: None,
        },
    )
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

/// Bases read from an indirect draw's arguments: the draw bases' block (16 bytes at offset 0, made by
/// either route) becomes [`HelperValue::DrawArguments`], and each read of the base vertex or first
/// instance becomes a read of the argument buffer at [`DRAW_ARGUMENTS_BINDING`]. In words from the
/// draw's offset, the base vertex is at 2 and the first instance at 3, one later when indexed.
fn draw_arguments(
    module: &mut Module,
    reflection: &mut Reflection,
    outlined: &mut Outlined,
) -> Result<()> {
    let Some(slot) = reflection
        .helpers
        .iter_mut()
        .find(|s| s.value == HelperValue::DrawBase)
    else {
        return Ok(());
    };
    slot.value = HelperValue::DrawArguments;
    let block = naga::ResourceBinding {
        group: HELPER_SET,
        binding: helper_binding(ShaderStage::Vertex),
    };
    let helpers = module
        .global_variables
        .iter()
        .find(|(_, g)| g.binding == Some(block))
        .map(|(h, _)| h)
        .ok_or(ConvertError::Malformed(
            "draw bases without their block".into(),
        ))?;
    let u32_ty = scalar(module, Scalar::U32);
    let i32_ty = scalar(module, Scalar::I32);
    let member = |name: &str, offset| StructMember {
        name: Some(name.into()),
        ty: u32_ty,
        binding: None,
        offset,
    };
    let ty = module.types.insert(
        Type {
            name: Some("Helpers".into()),
            inner: TypeInner::Struct {
                members: alloc::vec![
                    member("arguments_offset", 0),
                    member("indexed", 4),
                    member("draw", 8),
                ],
                span: 16,
            },
        },
        Span::UNDEFINED,
    );
    module.global_variables[helpers].ty = ty;
    let words = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Array {
                base: u32_ty,
                size: naga::ArraySize::Dynamic,
                stride: 4,
            },
        },
        Span::UNDEFINED,
    );
    let arguments = module.global_variables.append(
        GlobalVariable {
            name: Some("draw_arguments".into()),
            space: AddressSpace::Storage {
                access: StorageAccess::LOAD,
            },
            binding: Some(naga::ResourceBinding {
                group: HELPER_SET,
                binding: DRAW_ARGUMENTS_BINDING,
            }),
            ty: words,
            init: None,
            memory_decorations: Default::default(),
        },
        Span::UNDEFINED,
    );
    // The reader of word `first` (plus one when indexed), as the type the block's member had.
    let reader = |first: u32, signed: bool| {
        use naga::BinaryOperator::Add;
        let mut f = Function {
            name: Some(
                if signed {
                    "base_vertex"
                } else {
                    "first_instance"
                }
                .into(),
            ),
            result: Some(FunctionResult {
                ty: if signed { i32_ty } else { u32_ty },
                binding: None,
            }),
            ..Default::default()
        };
        let mut body = Block::new();
        let at = load_helper(&mut f, &mut body, (helpers, 0));
        let indexed = load_helper(&mut f, &mut body, (helpers, 1));
        let first = f
            .expressions
            .append(Expression::Literal(Literal::U32(first)), Span::UNDEFINED);
        let word = binary(&mut f, &mut body, Add, at, first);
        let word = binary(&mut f, &mut body, Add, word, indexed);
        let base = push(&mut f, &mut body, Expression::GlobalVariable(arguments));
        let pointer = push(&mut f, &mut body, Expression::Access { base, index: word });
        let mut value = push(&mut f, &mut body, Expression::Load { pointer });
        if signed {
            value = push(
                &mut f,
                &mut body,
                Expression::As {
                    expr: value,
                    kind: naga::ScalarKind::Sint,
                    convert: None,
                },
            );
        }
        body.push(Statement::Return { value: Some(value) }, Span::UNDEFINED);
        f.body = body;
        f
    };
    let mut sites = Vec::new();
    for (owner, f) in owners(module) {
        for (h, e) in f.expressions.iter() {
            let Expression::Load { pointer } = *e else {
                continue;
            };
            if let Expression::AccessIndex { base, index } = f.expressions[pointer]
                && index < 2
                && f.expressions[base] == Expression::GlobalVariable(helpers)
            {
                sites.push((owner, h, index));
            }
        }
    }
    for (owner, h, index) in sites {
        outlined.add(owner, h, reader(2 + index, index == 0), Vec::new());
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
    /// Library functions the callees call, put ahead of them.
    library: filter::Library,
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
        // The library first, at the handles its callers were given.
        for f in self.library.ahead.into_inner() {
            module.functions.append(f, Span::UNDEFINED);
        }
        let mut handles = Vec::new();
        for (_, f) in self.callees {
            handles.push(module.functions.append(f, Span::UNDEFINED));
        }
        let mut moved: Vec<Handle<naga::Function>> = Vec::new();
        for f in old.into_inner() {
            moved.push(module.functions.append(f, Span::UNDEFINED));
        }
        let remap = |f: &mut Function| {
            for (_, e) in f.expressions.iter_mut() {
                if let Expression::CallResult(callee) = e {
                    *callee = moved[callee.index()];
                }
            }
            each_block(&mut f.body, &mut |b: &mut Block| {
                for s in b.iter_mut() {
                    if let Statement::Call { function, .. } = s {
                        *function = moved[function.index()];
                    }
                }
            });
        };
        for &h in &moved {
            remap(&mut module.functions[h]);
        }
        for ep in &mut module.entry_points {
            remap(&mut ep.function);
        }
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

/// Samplings the variant changes, each replaced by one callee that applies every change it asks for.
/// Through a sampler in `lod_bias`, the sampler's level-of-detail bias from the helper block is added:
/// an implicit level becomes a bias, a bias and an explicit level grow by it, and gradients are scaled
/// by two to its power. Of a texture in `min_lod_clamped`, samples go through the merged sampler of
/// their pair (`CLAMPED_SAMPLER_BINDING` plus the pair's index, which carries the clamp), and samples
/// and gathers read zero where the clamp leaves the access no level (`min_lod_zero`).
fn samples(
    module: &mut Module,
    stage: ShaderStage,
    variant: &VariantKey,
    reflection: &mut Reflection,
    outlined: &mut Outlined,
    clamps: &mut Vec<(u32, (Handle<GlobalVariable>, u32))>,
) -> Result<()> {
    struct Site {
        owner: usize,
        h: Handle<Expression>,
        sampler: u32,
        image: Handle<GlobalVariable>,
        texture: Option<u32>,
        biased: bool,
        clamped: Option<(u32, Handle<Type>)>,
        mirrored: bool,
        /// Filtered in the program: by its texture (`filter_in_shader`) or its sampler
        /// (`border_in_shader`).
        filtered: Option<bool>,
    }
    let mut sites = Vec::new();
    // Pairs sampled natively and pairs only filtered in the program, which leave the reflection.
    let mut native = Vec::new();
    let mut unpaired = Vec::new();
    for (owner, f) in owners(module) {
        for (h, e) in f.expressions.iter() {
            let Expression::ImageSample {
                image,
                sampler,
                gather,
                ..
            } = *e
            else {
                continue;
            };
            let g = global_of(f, sampler).ok_or(ConvertError::Unsupported("SamplerArgument"))?;
            let Some(s) = register_of(module, reflection, g, SlotKind::Sampler) else {
                continue;
            };
            let t = global_of(f, image).ok_or(ConvertError::Unsupported("TextureArgument"))?;
            let texture = register_of(module, reflection, t, SlotKind::Texture);
            let clamped = texture
                .filter(|&r| r < 128 && variant.min_lod_clamped & (1 << r) != 0)
                .map(|r| (r, module.global_variables[g].ty));
            let biased = gather.is_none() && s < 16 && variant.lod_bias & (1 << s) != 0;
            let mirrored = s < 16 && variant.mirror_once_in_shader & (1 << s) != 0;
            let by_texture =
                texture.is_some_and(|r| r < 128 && variant.filter_in_shader & (1 << r) != 0);
            let by_sampler = s < 16 && variant.border_in_shader & (1 << s) != 0;
            let filtered = (by_texture || by_sampler).then_some(by_texture);
            if let Some(r) = texture {
                if filtered.is_some() {
                    unpaired.push((r, s));
                } else {
                    native.push((r, s));
                }
            }
            if biased || clamped.is_some() || mirrored || filtered.is_some() {
                sites.push(Site {
                    owner,
                    h,
                    sampler: s,
                    image: t,
                    texture,
                    biased,
                    clamped,
                    mirrored,
                    filtered,
                });
            }
        }
    }
    reflection
        .sampled_pairs
        .retain(|p| native.contains(p) || !unpaired.contains(p));
    let mut biases: Vec<(u32, (Handle<GlobalVariable>, u32))> = Vec::new();
    let mut states: Vec<(u32, HelperMember)> = Vec::new();
    let mut merged: Vec<(usize, Handle<GlobalVariable>)> = Vec::new();
    for site in sites {
        let bias_at = site
            .biased
            .then(|| match biases.iter().find(|(s, _)| *s == site.sampler) {
                Some((_, h)) => *h,
                None => {
                    let f32_ty = scalar(module, Scalar::F32);
                    let value = HelperValue::LodBias {
                        sampler: site.sampler,
                    };
                    let h = helper(module, reflection, stage, value, f32_ty, 4);
                    biases.push((site.sampler, h));
                    h
                }
            });
        if let Some(by_texture) = site.filtered {
            let state = sampler_state_helper(module, reflection, stage, site.sampler, &mut states);
            let clamp = site
                .clamped
                .map(|(texture, _)| min_lod_helper(module, reflection, stage, texture, clamps));
            filter::sample(
                module,
                stage,
                variant.flags,
                reflection,
                outlined,
                (site.owner, site.h),
                filter::Sources {
                    image: site.image,
                    texture: site.texture,
                    sampler: site.sampler,
                    by_texture,
                    state,
                    bias: bias_at,
                    clamp,
                },
            )?;
            continue;
        }
        let f = function(module, site.owner).clone();
        let Expression::ImageSample {
            image,
            sampler,
            gather,
            coordinate,
            array_index,
            offset,
            level,
            depth_ref,
            clamp_to_edge,
        } = f.expressions[site.h].clone()
        else {
            unreachable!()
        };
        if site.biased && depth_ref.is_some() {
            return Err(ConvertError::Unsupported("LodBiasOnComparison"));
        }
        if site.clamped.is_some() && depth_ref.is_some() {
            return Err(ConvertError::Unsupported("MinLodOnComparison"));
        }
        if site.mirrored {
            // A comparison or a depth sample has no gradient form, a gather's texels would come in
            // mirrored order, and a texel offset applies before the mirror.
            for (refused, name) in [
                (depth_ref.is_some(), "MirrorOnceOnComparison"),
                (gather.is_some(), "MirrorOnceOnGather"),
                (offset.is_some(), "MirrorOnceWithOffset"),
            ] {
                if refused {
                    return Err(ConvertError::Unsupported(name));
                }
            }
        }
        let state_at = site
            .mirrored
            .then(|| sampler_state_helper(module, reflection, stage, site.sampler, &mut states));
        // The merged sampler of the pair, made on first use; gathers read level 0 whatever the
        // sampler's range, so they keep their own.
        let merged_sampler = match site.clamped {
            Some((texture, ty)) if gather.is_none() => {
                let pair = reflection
                    .sampled_pairs
                    .iter()
                    .position(|&p| p == (texture, site.sampler))
                    .ok_or_else(|| {
                        ConvertError::Malformed("a sampled pair missing from the reflection".into())
                    })?;
                Some(match merged.iter().find(|(p, _)| *p == pair) {
                    Some((_, g)) => *g,
                    None => {
                        let g = module.global_variables.append(
                            GlobalVariable {
                                name: Some(format!("clamped_sampler_{pair}")),
                                space: AddressSpace::Handle,
                                binding: Some(naga::ResourceBinding {
                                    group: HELPER_SET,
                                    binding: CLAMPED_SAMPLER_BINDING + pair as u32,
                                }),
                                ty,
                                init: None,
                                memory_decorations: Default::default(),
                            },
                            Span::UNDEFINED,
                        );
                        merged.push((pair, g));
                        g
                    }
                })
            }
            _ => None,
        };
        let clamp_at = site
            .clamped
            .map(|(texture, _)| min_lod_helper(module, reflection, stage, texture, clamps));
        // The callee's arguments: the image, the sampler unless the merged one replaces it, the
        // coordinate, the array index and the level's operands; the last operand only types the result.
        let mut operands = alloc::vec![image];
        if merged_sampler.is_none() {
            operands.push(sampler);
        }
        operands.push(coordinate);
        operands.extend(array_index);
        let next = operands.len();
        match level {
            SampleLevel::Exact(e) | SampleLevel::Bias(e) => operands.push(e),
            SampleLevel::Gradient { x, y } => operands.extend([x, y]),
            SampleLevel::Auto | SampleLevel::Zero => {}
        }
        operands.push(site.h);
        let types = types_of(module, &f, &operands)?;
        let result = *types.last().unwrap();
        let mut callee = Function {
            name: Some(format!("sample_{}", site.sampler)),
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
        let sampler = match merged_sampler {
            Some(g) => push(&mut callee, &mut body, Expression::GlobalVariable(g)),
            None => arg(1),
        };
        let coordinate = arg(if merged_sampler.is_some() { 1 } else { 2 });
        let array_index = array_index.map(|_| arg(next - 1));
        let level = match bias_at {
            Some(bias_at) => {
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
                let bias = load_helper(&mut callee, &mut body, bias_at);
                let add = |callee: &mut Function, body: &mut Block, e| {
                    binary(callee, body, naga::BinaryOperator::Add, e, bias)
                };
                match level {
                    SampleLevel::Auto => SampleLevel::Bias(bias),
                    SampleLevel::Zero => SampleLevel::Exact(bias),
                    SampleLevel::Exact(_) => {
                        SampleLevel::Exact(add(&mut callee, &mut body, arg(next)))
                    }
                    SampleLevel::Bias(_) => {
                        SampleLevel::Bias(add(&mut callee, &mut body, arg(next)))
                    }
                    SampleLevel::Gradient { .. } => {
                        let scale = math(&mut callee, &mut body, MathFunction::Exp2, [bias; 2]);
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
                }
            }
            None => match level {
                SampleLevel::Exact(_) => SampleLevel::Exact(arg(next)),
                SampleLevel::Bias(_) => SampleLevel::Bias(arg(next)),
                SampleLevel::Gradient { .. } => SampleLevel::Gradient {
                    x: arg(next),
                    y: arg(next + 1),
                },
                other => other,
            },
        };
        let (coordinate, level) = match state_at {
            Some(state_at) => mirror_once(
                module,
                &mut callee,
                &mut body,
                state_at,
                (
                    coordinate,
                    types[if merged_sampler.is_some() { 1 } else { 2 }],
                ),
                (types[0], level),
            )?,
            None => (coordinate, level),
        };
        if stage != ShaderStage::Fragment && matches!(level, SampleLevel::Bias(_)) {
            return Err(ConvertError::Malformed(
                "an implicit level outside a pixel shader".into(),
            ));
        }
        let offset = offset.map(|o| copy_constant(&f, o, &mut callee));
        let mut value = push(
            &mut callee,
            &mut body,
            Expression::ImageSample {
                image: arg(0),
                sampler,
                gather,
                coordinate,
                array_index,
                offset,
                level,
                depth_ref: None,
                clamp_to_edge,
            },
        );
        if let Some(clamp_at) = clamp_at {
            // A gather reads level 0 only, which a clamp of one or more leaves out.
            let read = gather.is_some().then(|| f32_lit(&mut callee, 0.0));
            value = min_lod_zero(&mut callee, &mut body, clamp_at, read, result, value);
        }
        body.push(Statement::Return { value: Some(value) }, Span::UNDEFINED);
        callee.body = body;
        outlined.add(site.owner, site.h, callee, operands[..n].to_vec());
    }
    Ok(())
}

/// The helper member holding sampler `sampler`'s state (`HelperValue::SamplerState`, nine words, read as
/// three four-word vectors), made on first use.
fn sampler_state_helper(
    module: &mut Module,
    reflection: &mut Reflection,
    stage: ShaderStage,
    sampler: u32,
    states: &mut Vec<(u32, HelperMember)>,
) -> HelperMember {
    if let Some((_, h)) = states.iter().find(|(s, _)| *s == sampler) {
        return *h;
    }
    let uvec4 = vector(module, 4, Scalar::U32);
    let ty = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Array {
                base: uvec4,
                size: naga::ArraySize::Constant(core::num::NonZeroU32::new(3).unwrap()),
                stride: 16,
            },
        },
        Span::UNDEFINED,
    );
    let value = HelperValue::SamplerState { sampler };
    let h = helper(module, reflection, stage, value, ty, 48);
    states.push((sampler, h));
    h
}

/// `AddressMode::MirrorOnce`'s code in `HelperValue::SamplerState`.
const MIRROR_ONCE_CODE: u32 = wcontainers_gpu::AddressMode::MirrorOnce as u32;

/// Mirror-once addressing on a clamping sampler: each coordinate whose address mode in the sampler's
/// state is mirror-once is taken as its absolute value, and an implicit level becomes the original
/// coordinate's gradients (scaled by two to a bias), so the footprint and level of detail stay those of
/// the unmirrored coordinate. Cube coordinates are directions, which no address mode touches.
fn mirror_once(
    module: &mut Module,
    callee: &mut Function,
    body: &mut Block,
    state_at: HelperMember,
    (coordinate, ty): (Handle<Expression>, Handle<Type>),
    (image, level): (Handle<Type>, SampleLevel),
) -> Result<(Handle<Expression>, SampleLevel)> {
    use naga::BinaryOperator::{Equal, Multiply};
    if let TypeInner::Image {
        dim: naga::ImageDimension::Cube,
        ..
    } = module.types[image].inner
    {
        return Ok((coordinate, level));
    }
    let depth = matches!(
        module.types[image].inner,
        TypeInner::Image {
            class: ImageClass::Depth { .. },
            ..
        }
    );
    let level = match level {
        SampleLevel::Auto | SampleLevel::Bias(_) if depth => {
            return Err(ConvertError::Unsupported("MirrorOnceOnDepth"));
        }
        SampleLevel::Auto | SampleLevel::Bias(_) => {
            let [mut x, mut y] = [naga::DerivativeAxis::X, naga::DerivativeAxis::Y].map(|axis| {
                push(
                    callee,
                    body,
                    Expression::Derivative {
                        axis,
                        ctrl: naga::DerivativeControl::None,
                        expr: coordinate,
                    },
                )
            });
            if let SampleLevel::Bias(b) = level {
                let scale = math(callee, body, MathFunction::Exp2, [b; 2]);
                x = binary(callee, body, Multiply, x, scale);
                y = binary(callee, body, Multiply, y, scale);
            }
            SampleLevel::Gradient { x, y }
        }
        other => other,
    };
    let count = match module.types[ty].inner {
        TypeInner::Vector { size, .. } => size as u32,
        _ => 1,
    };
    let state = load_helper(callee, body, state_at);
    let code = callee.expressions.append(
        Expression::Literal(Literal::U32(MIRROR_ONCE_CODE)),
        Span::UNDEFINED,
    );
    let mut components = Vec::new();
    for axis in 0..count {
        // The address modes are words 3, 4 and 5.
        let word = 3 + axis;
        let row = index(callee, body, state, word / 4);
        let mode = index(callee, body, row, word % 4);
        let mirrored = binary(callee, body, Equal, mode, code);
        let c = if count == 1 {
            coordinate
        } else {
            index(callee, body, coordinate, axis)
        };
        let folded = math(callee, body, MathFunction::Abs, [c; 2]);
        components.push(push(
            callee,
            body,
            Expression::Select {
                condition: mirrored,
                accept: folded,
                reject: c,
            },
        ));
    }
    let coordinate = if count == 1 {
        components[0]
    } else {
        push(callee, body, Expression::Compose { ty, components })
    };
    Ok((coordinate, level))
}

/// The helper member holding texture `texture`'s clamp and level count (two u32, the clamp's f32 bits
/// first), made on first use.
fn min_lod_helper(
    module: &mut Module,
    reflection: &mut Reflection,
    stage: ShaderStage,
    texture: u32,
    made: &mut Vec<(u32, (Handle<GlobalVariable>, u32))>,
) -> (Handle<GlobalVariable>, u32) {
    if let Some((_, at)) = made.iter().find(|(t, _)| *t == texture) {
        return *at;
    }
    let vec2 = vector(module, 2, Scalar::U32);
    let at = helper(
        module,
        reflection,
        stage,
        HelperValue::MinLod { texture },
        vec2,
        8,
    );
    made.push((texture, at));
    at
}

/// `value`, or zero (Direct3D's out-of-bounds result) where the per-resource clamp leaves the access no
/// level: when the clamp passes the view's last level, or when the level the access reads (`level`, an
/// f32 expression or a constant) is below the clamp's whole part.
fn min_lod_zero(
    f: &mut Function,
    body: &mut Block,
    clamp_at: (Handle<GlobalVariable>, u32),
    level: Option<Handle<Expression>>,
    ty: Handle<Type>,
    value: Handle<Expression>,
) -> Handle<Expression> {
    use naga::BinaryOperator as Op;
    let m = load_helper(f, body, clamp_at);
    let bits = index(f, body, m, 0);
    let clamp = push(
        f,
        body,
        Expression::As {
            expr: bits,
            kind: naga::ScalarKind::Float,
            convert: None,
        },
    );
    let count = index(f, body, m, 1);
    let count = push(
        f,
        body,
        Expression::As {
            expr: count,
            kind: naga::ScalarKind::Float,
            convert: Some(4),
        },
    );
    let one = f32_lit(f, 1.0);
    let last = binary(f, body, Op::Subtract, count, one);
    let mut zero_when = binary(f, body, Op::Greater, clamp, last);
    if let Some(level) = level {
        let whole = math(f, body, MathFunction::Floor, [clamp; 2]);
        let below = binary(f, body, Op::Less, level, whole);
        zero_when = binary(f, body, Op::LogicalOr, zero_when, below);
    }
    let zero = push(f, body, Expression::ZeroValue(ty));
    push(
        f,
        body,
        Expression::Select {
            condition: zero_when,
            accept: zero,
            reject: value,
        },
    )
}

/// Loads of textures in `min_lod_clamped` read zero where the clamp leaves their level out.
fn loads(
    module: &mut Module,
    stage: ShaderStage,
    variant: &VariantKey,
    reflection: &mut Reflection,
    outlined: &mut Outlined,
    clamps: &mut Vec<(u32, (Handle<GlobalVariable>, u32))>,
) -> Result<()> {
    let mut sites = Vec::new();
    for (owner, f) in owners(module) {
        for (h, e) in f.expressions.iter() {
            let Expression::ImageLoad { image, .. } = *e else {
                continue;
            };
            let g = global_of(f, image).ok_or(ConvertError::Unsupported("TextureArgument"))?;
            let Some(r) = register_of(module, reflection, g, SlotKind::Texture) else {
                continue;
            };
            if r < 128 && variant.min_lod_clamped & (1 << r) != 0 {
                sites.push((owner, h, r));
            }
        }
    }
    for (owner, h, texture) in sites {
        let clamp_at = min_lod_helper(module, reflection, stage, texture, clamps);
        let f = function(module, owner).clone();
        let Expression::ImageLoad {
            image,
            coordinate,
            array_index,
            sample,
            level,
        } = f.expressions[h].clone()
        else {
            unreachable!()
        };
        let mut operands = alloc::vec![image, coordinate];
        operands.extend(array_index);
        operands.extend(sample);
        operands.extend(level);
        operands.push(h);
        let types = types_of(module, &f, &operands)?;
        let result = *types.last().unwrap();
        let mut callee = Function {
            name: Some(format!("load_clamped_{texture}")),
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
        let mut next = 2;
        let mut take = |o: Option<Handle<Expression>>| {
            o.map(|_| {
                next += 1;
                args[next - 1]
            })
        };
        let array_index = take(array_index);
        let sample = take(sample);
        let level = take(level);
        let value = push(
            &mut callee,
            &mut body,
            Expression::ImageLoad {
                image: args[0],
                coordinate: args[1],
                array_index,
                sample,
                level,
            },
        );
        let read = match level {
            Some(l) => push(
                &mut callee,
                &mut body,
                Expression::As {
                    expr: l,
                    kind: naga::ScalarKind::Float,
                    convert: Some(4),
                },
            ),
            None => f32_lit(&mut callee, 0.0),
        };
        let value = min_lod_zero(&mut callee, &mut body, clamp_at, Some(read), result, value);
        body.push(Statement::Return { value: Some(value) }, Span::UNDEFINED);
        callee.body = body;
        outlined.add(owner, h, callee, operands[..n].to_vec());
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
    for (owner, h, texture, samples) in sites {
        let value = if samples {
            HelperValue::TextureSamples { texture }
        } else {
            HelperValue::TextureLevels { texture }
        };
        let u32_ty = scalar(module, Scalar::U32);
        let at = helper_once(module, reflection, stage, value, u32_ty, 4);
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
