//! Typed buffers and storage images without a format from dxil-spirv's SPIR-V. naga's reader takes
//! neither, so their image types are rewritten before reading into ones it takes (buffers as 1D
//! images, a missing format as a 32-bit one of the texel's kind), and the module it gives is then
//! brought to the DXBC route's forms: storage images of the bound view's format, typed buffers read
//! and written by the routes of `variant::texel`.

use super::{Placed, each_block, functions_first, place_calls};
use crate::ConvertError;
use crate::lower::{storage_format, storage_kind};
use crate::variant::texel::{TexelBuffer, TexelPlace};
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use naga::{
    AddressSpace, Expression, Function, GlobalVariable, Handle, ImageClass, ImageDimension,
    ImageQuery, Literal, Module, Range, Scalar, ScalarKind, Span, Statement, Type, TypeInner,
    VectorSize,
};
use spirv::{Decoration, Dim, ImageFormat, Op, StorageClass};
use wcontainers_gpu::{Reflection, ShaderStage, TexelBufferRoute, VariantKey};

type Result<T> = core::result::Result<T, ConvertError>;

/// A typed buffer or a storage image without a format, as dxil-spirv declared it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Declared {
    space: u32,
    register: u32,
    /// A view a program writes (`RWBuffer`, `RWTexture*`), not a read-only one.
    storage: bool,
    /// A typed buffer, not a storage image.
    buffer: bool,
}

/// Finds the typed buffers and formatless storage images the SPIR-V declares, and rewrites their image
/// types in place into ones naga's reader takes.
pub(super) fn prepare(words: &mut [u32]) -> Vec<Declared> {
    let mut sets = BTreeMap::new();
    let mut registers = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    let mut images = BTreeMap::new();
    // A pointer's or array's element type.
    let mut elements = BTreeMap::new();
    let mut variables = Vec::new();
    let mut at = 5;
    while at < words.len() {
        let count = (words[at] >> 16) as usize;
        let Some(w) = words.get(at..at + count).filter(|_| count > 0) else {
            break;
        };
        match Op::from_u32(w[0] & 0xffff) {
            Some(Op::Decorate) if count >= 4 => {
                if w[2] == Decoration::DescriptorSet as u32 {
                    sets.insert(w[1], w[3]);
                } else if w[2] == Decoration::Binding as u32 {
                    registers.insert(w[1], w[3]);
                }
            }
            Some(Op::TypeInt) if count >= 4 => {
                let kind = if w[3] != 0 {
                    ScalarKind::Sint
                } else {
                    ScalarKind::Uint
                };
                kinds.insert(w[1], kind);
            }
            Some(Op::TypeFloat) if count >= 3 => {
                kinds.insert(w[1], ScalarKind::Float);
            }
            Some(Op::TypeImage) if count >= 9 => {
                images.insert(w[1], at);
            }
            Some(Op::TypePointer) if count >= 4 => {
                elements.insert(w[1], w[3]);
            }
            Some(Op::TypeArray | Op::TypeRuntimeArray) if count >= 3 => {
                elements.insert(w[1], w[2]);
            }
            Some(Op::Variable) if count >= 4 && w[3] == StorageClass::UniformConstant as u32 => {
                variables.push((w[1], w[2]));
            }
            _ => {}
        }
        at += count;
    }
    let buffer = |words: &[u32], i: usize| words[i + 3] == Dim::DimBuffer as u32;
    let formatless =
        |words: &[u32], i: usize| words[i + 7] == 2 && words[i + 8] == ImageFormat::Unknown as u32;
    let mut declared = Vec::new();
    for (ty, id) in variables {
        let mut element = ty;
        for _ in 0..4 {
            match elements.get(&element) {
                Some(&e) => element = e,
                None => break,
            }
        }
        let (Some(&i), Some(&register)) = (images.get(&element), registers.get(&id)) else {
            continue;
        };
        if buffer(words, i) || formatless(words, i) {
            declared.push(Declared {
                space: sets.get(&id).copied().unwrap_or(0),
                register,
                storage: words[i + 7] == 2,
                buffer: buffer(words, i),
            });
        }
    }
    for &i in images.values() {
        if formatless(words, i) {
            let placeholder = match kinds.get(&words[i + 2]) {
                Some(ScalarKind::Float) => ImageFormat::R32f,
                Some(ScalarKind::Sint) => ImageFormat::R32i,
                Some(ScalarKind::Uint) => ImageFormat::R32ui,
                _ => continue,
            };
            words[i + 8] = placeholder as u32;
        }
        if buffer(words, i) {
            words[i + 3] = Dim::Dim1D as u32;
        }
    }
    declared
}

/// Each declared image's global variable, matched by space, register and kind while the bindings
/// are still dxil-spirv's.
pub(super) fn globals(
    module: &Module,
    declared: &[Declared],
) -> Vec<(Handle<GlobalVariable>, Declared)> {
    let mut found = Vec::new();
    for (h, v) in module.global_variables.iter() {
        let (Some(b), TypeInner::Image { class, .. }) = (v.binding, &module.types[v.ty].inner)
        else {
            continue;
        };
        let storage = matches!(class, ImageClass::Storage { .. });
        if let Some(d) = declared
            .iter()
            .find(|d| (d.space, d.register, d.storage) == (b.group, b.binding, storage))
        {
            found.push((h, *d));
        }
    }
    found
}

/// How a function uses one image.
#[derive(Clone, Copy, Default)]
struct Uses {
    load: bool,
    store: bool,
    size: bool,
    /// Any other use: queries other than the size, samples, atomics, passing it to a function.
    other: bool,
}

/// The image global `e` names in `f`, if it is one of `globals`.
fn image_of(
    f: &Function,
    e: Handle<Expression>,
    globals: &[(Handle<GlobalVariable>, Declared)],
) -> Option<usize> {
    match f.expressions[e] {
        Expression::GlobalVariable(g) => globals.iter().position(|(h, _)| *h == g),
        _ => None,
    }
}

/// How the program uses each of `globals`.
fn uses(module: &Module, globals: &[(Handle<GlobalVariable>, Declared)]) -> Vec<Uses> {
    let mut uses = vec![Uses::default(); globals.len()];
    let functions = module
        .functions
        .iter()
        .map(|(_, f)| f)
        .chain(module.entry_points.iter().map(|ep| &ep.function));
    for f in functions {
        for (_, e) in f.expressions.iter() {
            let (image, used) = match *e {
                Expression::ImageLoad { image, .. } => (image, 0),
                Expression::ImageQuery {
                    image,
                    query: ImageQuery::Size { .. },
                } => (image, 1),
                Expression::ImageQuery { image, .. } | Expression::ImageSample { image, .. } => {
                    (image, 2)
                }
                _ => continue,
            };
            if let Some(i) = image_of(f, image, globals) {
                match used {
                    0 => uses[i].load = true,
                    1 => uses[i].size = true,
                    _ => uses[i].other = true,
                }
            }
        }
        let mut stack = vec![&f.body];
        while let Some(block) = stack.pop() {
            for s in block.iter() {
                match s {
                    Statement::ImageStore { image, .. } => {
                        if let Some(i) = image_of(f, *image, globals) {
                            uses[i].store = true;
                        }
                    }
                    Statement::ImageAtomic { image, .. } => {
                        if let Some(i) = image_of(f, *image, globals) {
                            uses[i].other = true;
                        }
                    }
                    Statement::Call { arguments, .. } => {
                        for &a in arguments {
                            if let Some(i) = image_of(f, a, globals) {
                                uses[i].other = true;
                            }
                        }
                    }
                    Statement::Block(b) => stack.push(b),
                    Statement::If { accept, reject, .. } => stack.extend([accept, reject]),
                    Statement::Loop {
                        body, continuing, ..
                    } => stack.extend([body, continuing]),
                    Statement::Switch { cases, .. } => stack.extend(cases.iter().map(|c| &c.body)),
                    _ => {}
                }
            }
        }
    }
    uses
}

/// The texel's scalar kind of an image type as the reader gave it.
fn texel_kind(inner: &TypeInner) -> Option<ScalarKind> {
    match *inner {
        TypeInner::Image {
            class: ImageClass::Sampled { kind, .. },
            ..
        } => Some(kind),
        TypeInner::Image {
            class: ImageClass::Storage { format, .. },
            ..
        } => Some(storage_kind(format)),
        _ => None,
    }
}

/// A storage image of the bound view's format (`VariantKey::storage_formats`), with the access the
/// program's uses need. Read-write access exists in WebGPU for 32-bit single-channel formats only.
fn storage_image(
    module: &mut Module,
    variant: &VariantKey,
    var: Handle<GlobalVariable>,
    d: &Declared,
    uses: Uses,
) -> Result<()> {
    let TypeInner::Image { dim, arrayed, .. } = module.types[module.global_variables[var].ty].inner
    else {
        return Err(ConvertError::Malformed("a storage image's type".into()));
    };
    let declared = texel_kind(&module.types[module.global_variables[var].ty].inner);
    let format = variant
        .storage_formats
        .iter()
        .find(|(slot, _)| *slot == d.register)
        .ok_or(ConvertError::Unsupported("StorageFormat"))?
        .1;
    let format = storage_format(format).ok_or(ConvertError::Unsupported("StorageFormatAbsent"))?;
    if declared != Some(storage_kind(format)) {
        return Err(ConvertError::Unsupported("StorageFormatKind"));
    }
    let access = match (uses.load, uses.store) {
        (true, false) => naga::StorageAccess::LOAD,
        (false, _) => naga::StorageAccess::STORE,
        (true, true) => {
            use naga::StorageFormat as S;
            if !matches!(format, S::R32Float | S::R32Uint | S::R32Sint) {
                return Err(ConvertError::Unsupported("ReadWriteStorageFormat"));
            }
            naga::StorageAccess::LOAD | naga::StorageAccess::STORE
        }
    };
    let ty = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Image {
                dim,
                arrayed,
                class: ImageClass::Storage { format, access },
            },
        },
        Span::UNDEFINED,
    );
    module.global_variables[var].ty = ty;
    Ok(())
}

/// One typed buffer's view, retyped for its route, with the texel functions its uses need.
struct Typed {
    var: Handle<GlobalVariable>,
    buffer: TexelBuffer,
    /// `vec4` of the texel's scalar, which stores take.
    vec4: Handle<Type>,
    load: Option<Handle<Function>>,
    store: Option<Handle<Function>>,
    length: Option<Handle<Function>>,
}

/// Retypes a typed buffer for the variant's route, as the DXBC route declares it.
fn typed_buffer(
    module: &mut Module,
    variant: &VariantKey,
    var: Handle<GlobalVariable>,
    d: &Declared,
    uses: Uses,
    stage: ShaderStage,
) -> Result<(TexelBuffer, Handle<Type>)> {
    let texture = match variant.texel_buffers {
        TexelBufferRoute::Native => return Err(ConvertError::Unsupported("TypedBuffer")),
        TexelBufferRoute::Storage => false,
        TexelBufferRoute::Texture if !d.storage => true,
        TexelBufferRoute::Texture => {
            return Err(ConvertError::Unsupported("TypedBufferStorageAsTexture"));
        }
    };
    if uses.other {
        return Err(ConvertError::Unsupported("TypedBufferUse"));
    }
    let formats = if d.storage {
        &variant.storage_formats
    } else {
        &variant.texel_formats
    };
    let format = formats
        .iter()
        .find(|(slot, _)| *slot == d.register)
        .ok_or(ConvertError::Unsupported("TypedBufferFormat"))?
        .1;
    let declared = texel_kind(&module.types[module.global_variables[var].ty].inner)
        .ok_or_else(|| ConvertError::Malformed("a typed buffer's type".into()))?;
    let layout = TexelBuffer::layout(format, declared)?;
    let atomic = (uses.store && TexelBuffer::narrow(layout.2)).then_some(Scalar::U32);
    let (ty, space, place) = if texture {
        let ty = module.types.insert(
            Type {
                name: None,
                inner: TypeInner::Image {
                    dim: ImageDimension::D2,
                    arrayed: false,
                    class: ImageClass::Sampled {
                        kind: declared,
                        multi: false,
                    },
                },
            },
            Span::UNDEFINED,
        );
        (ty, AddressSpace::Handle, TexelPlace::Texture)
    } else {
        let access = if uses.store {
            naga::StorageAccess::LOAD | naga::StorageAccess::STORE
        } else {
            naga::StorageAccess::LOAD
        };
        let words = TexelBuffer::words(module, atomic);
        (words, AddressSpace::Storage { access }, TexelPlace::Words)
    };
    let global = &mut module.global_variables[var];
    global.ty = ty;
    global.space = space;
    let binding = global
        .binding
        .ok_or_else(|| ConvertError::Malformed("a typed buffer without a binding".into()))?;
    let buffer = TexelBuffer::declared(
        var,
        layout,
        atomic,
        place,
        stage,
        binding.group,
        binding.binding,
    );
    let scalar = buffer.scalar();
    let vec4 = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Vector {
                size: VectorSize::Quad,
                scalar,
            },
        },
        Span::UNDEFINED,
    );
    Ok((buffer, vec4))
}

/// What one function does with typed buffers, read while the buffers still have their image types.
#[derive(Default)]
struct Plan {
    /// (load, the buffer, its coordinate, whether the coordinate is signed)
    loads: Vec<(Handle<Expression>, usize, Handle<Expression>, bool)>,
    /// (size query, the buffer)
    sizes: Vec<(Handle<Expression>, usize)>,
    stores: Vec<Store>,
}

/// A store into a typed buffer.
struct Store {
    buffer: usize,
    coordinate: Handle<Expression>,
    signed: bool,
    value: Handle<Expression>,
    /// The value's components.
    components: u32,
}

fn plan(
    module: &Module,
    f: &Function,
    globals: &[(Handle<GlobalVariable>, Declared)],
    typed: &[Option<usize>],
) -> Result<Plan> {
    let buffer = |e| image_of(f, e, globals).and_then(|i| typed[i]);
    let mut typifier = naga::front::Typifier::new();
    let ctx = naga::proc::ResolveContext::with_locals(module, &f.local_variables, &f.arguments);
    let mut kind = |e: Handle<Expression>| -> Result<(bool, u32)> {
        typifier
            .grow(e, &f.expressions, &ctx)
            .map_err(|e| ConvertError::Malformed(alloc::format!("{e}")))?;
        Ok(match *typifier.get(e, &module.types) {
            TypeInner::Scalar(s) => (s.kind == ScalarKind::Sint, 1),
            TypeInner::Vector { size, scalar } => (scalar.kind == ScalarKind::Sint, size as u32),
            _ => return Err(ConvertError::Malformed("a typed buffer's operand".into())),
        })
    };
    let mut plan = Plan::default();
    for (h, e) in f.expressions.iter() {
        match *e {
            Expression::ImageLoad {
                image, coordinate, ..
            } => {
                if let Some(t) = buffer(image) {
                    plan.loads.push((h, t, coordinate, kind(coordinate)?.0));
                }
            }
            Expression::ImageQuery { image, .. } => {
                if let Some(t) = buffer(image) {
                    plan.sizes.push((h, t));
                }
            }
            _ => {}
        }
    }
    let mut stores = Vec::new();
    let mut stack = vec![&f.body];
    while let Some(block) = stack.pop() {
        for s in block.iter() {
            match s {
                Statement::ImageStore {
                    image,
                    coordinate,
                    value,
                    ..
                } => {
                    if let Some(t) = buffer(*image) {
                        stores.push((t, *coordinate, *value));
                    }
                }
                Statement::Block(b) => stack.push(b),
                Statement::If { accept, reject, .. } => stack.extend([accept, reject]),
                Statement::Loop {
                    body, continuing, ..
                } => stack.extend([body, continuing]),
                Statement::Switch { cases, .. } => stack.extend(cases.iter().map(|c| &c.body)),
                _ => {}
            }
        }
    }
    for (t, coordinate, value) in stores {
        let signed = kind(coordinate)?.0;
        let components = kind(value)?.1;
        plan.stores.push(Store {
            buffer: t,
            coordinate,
            signed,
            value,
            components,
        });
    }
    Ok(plan)
}

/// Appends `e` to `f`'s expressions.
fn append(f: &mut Function, e: Expression) -> Handle<Expression> {
    f.expressions.append(e, Span::UNDEFINED)
}

/// The coordinate as the texel functions' u32 index, with the range of what was appended for it.
fn index(
    f: &mut Function,
    coordinate: Handle<Expression>,
    signed: bool,
) -> (Handle<Expression>, Option<Range<Expression>>) {
    if !signed {
        return (coordinate, None);
    }
    let h = append(
        f,
        Expression::As {
            expr: coordinate,
            kind: ScalarKind::Uint,
            convert: None,
        },
    );
    (h, Some(Range::new_from_bounds(h, h)))
}

/// Rewrites the typed buffers' loads, size queries and stores in `f` as calls of their texel
/// functions.
fn rewrite(f: &mut Function, plan: &Plan, typed: &[Typed]) {
    let mut calls = Vec::new();
    for &(h, t, coordinate, signed) in &plan.loads {
        let (i, pre) = index(f, coordinate, signed);
        calls.push(Placed {
            result: h,
            function: typed[t].load.expect("made for its loads"),
            arguments: vec![i],
            pre,
        });
    }
    for &(h, t) in &plan.sizes {
        calls.push(Placed {
            result: h,
            function: typed[t].length.expect("made for its queries"),
            arguments: Vec::new(),
            pre: None,
        });
    }
    place_calls(f, &calls);
    // Each store's index and value, appended now and emitted just before its call: the zero literal
    // (never emitted) first, then the emitted expressions in one range.
    let mut stores = Vec::new();
    for &Store {
        buffer: t,
        coordinate,
        signed,
        value,
        components,
    } in &plan.stores
    {
        let zero = (components != 4).then(|| {
            append(
                f,
                Expression::Literal(match typed[t].buffer.scalar().kind {
                    ScalarKind::Float => Literal::F32(0.0),
                    ScalarKind::Sint => Literal::I32(0),
                    _ => Literal::U32(0),
                }),
            )
        });
        let start = f.expressions.len();
        let (i, _) = index(f, coordinate, signed);
        let padded = match zero {
            None => value,
            Some(zero) => {
                let parts = (0..4)
                    .map(|c| match (c < components, components) {
                        (true, 1) => value,
                        (true, _) => append(
                            f,
                            Expression::AccessIndex {
                                base: value,
                                index: c,
                            },
                        ),
                        (false, _) => zero,
                    })
                    .collect();
                append(
                    f,
                    Expression::Compose {
                        ty: typed[t].vec4,
                        components: parts,
                    },
                )
            }
        };
        let end = f.expressions.len();
        let range = (end > start)
            .then(|| Range::from_index_range(start as u32..end as u32, &f.expressions));
        stores.push((t, coordinate, value, i, padded, range));
    }
    let Function {
        body, expressions, ..
    } = f;
    // Two equal stores are two statements: each takes its own entry.
    let mut taken = vec![false; stores.len()];
    each_block(body, &mut |block| {
        let found = |s: &Statement, taken: &[bool]| {
            match s {
            Statement::ImageStore {
                image,
                coordinate,
                value,
                ..
            } => stores.iter().enumerate().position(|(n, (t, c, v, ..))| {
                !taken[n]
                    && (*c, *v) == (*coordinate, *value)
                    && matches!(expressions[*image], Expression::GlobalVariable(g) if g == typed[*t].var)
            }),
            _ => None,
        }
        };
        if !block.iter().any(|s| found(s, &taken).is_some()) {
            return;
        }
        for (s, span) in core::mem::take(block).span_into_iter() {
            let Some(n) = found(&s, &taken) else {
                block.push(s, span);
                continue;
            };
            taken[n] = true;
            let (t, _, _, i, padded, range) = &stores[n];
            if let Some(r) = range {
                block.push(Statement::Emit(r.clone()), span);
            }
            block.push(
                Statement::Call {
                    function: typed[*t].store.expect("made for its stores"),
                    arguments: vec![*i, *padded],
                    result: None,
                },
                span,
            );
        }
    });
}

/// Brings the reader's typed buffers and formatless storage images to their final forms. Runs after
/// the bindings are numbered and the helper block for the draw's bases is made.
pub(super) fn finish(
    module: &mut Module,
    variant: &VariantKey,
    globals: &[(Handle<GlobalVariable>, Declared)],
    stage: ShaderStage,
    reflection: &mut Reflection,
) -> Result<()> {
    if globals.is_empty() {
        return Ok(());
    }
    let uses = uses(module, globals);
    // Plans are read while the buffers still have their image types.
    let mut typed_at = vec![None; globals.len()];
    let mut n = 0;
    for (i, (_, d)) in globals.iter().enumerate() {
        if d.buffer {
            typed_at[i] = Some(n);
            n += 1;
        }
    }
    let functions: Vec<Plan> = module
        .functions
        .iter()
        .map(|(_, f)| plan(module, f, globals, &typed_at))
        .collect::<Result<_>>()?;
    let entry_points: Vec<Plan> = module
        .entry_points
        .iter()
        .map(|ep| plan(module, &ep.function, globals, &typed_at))
        .collect::<Result<_>>()?;
    let mut buffers = Vec::new();
    for (i, (var, d)) in globals.iter().enumerate() {
        if d.buffer {
            let (buffer, vec4) = typed_buffer(module, variant, *var, d, uses[i], stage)?;
            buffers.push((i, buffer, vec4));
        } else {
            storage_image(module, variant, *var, d, uses[i])?;
        }
    }
    if buffers.is_empty() {
        return Ok(());
    }
    let (typed, moved) = functions_first(module, |module| {
        buffers
            .into_iter()
            .map(|(i, mut buffer, vec4)| {
                let u = uses[i];
                let load = u.load.then(|| buffer.load(module, reflection));
                let store = u.store.then(|| buffer.store(module, reflection));
                let length = u.size.then(|| buffer.length(module, reflection));
                Typed {
                    var: globals[i].0,
                    buffer,
                    vec4,
                    load,
                    store,
                    length,
                }
            })
            .collect::<Vec<_>>()
    });
    for (plan, h) in functions.iter().zip(moved) {
        rewrite(&mut module.functions[h], plan, &typed);
    }
    for (plan, ep) in entry_points.iter().zip(module.entry_points.iter_mut()) {
        rewrite(&mut ep.function, plan, &typed);
    }
    Ok(())
}
