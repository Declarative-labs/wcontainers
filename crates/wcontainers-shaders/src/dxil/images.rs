//! Typed buffers and storage images without a format from dxil-spirv's SPIR-V. naga's reader takes
//! neither, so their image types are rewritten before reading into ones it takes (buffers as 1D
//! images, a missing format as a 32-bit one of the texel's kind), and the module it gives is then
//! brought to the DXBC route's forms: storage images of the bound view's format, typed buffers read
//! and written by the routes of `variant::texel`.

use super::{Placed, each_block, functions_first, place_calls};
use crate::ConvertError;
use crate::build::{Builder, Expr};
use crate::lower::{storage_format, storage_kind};
use crate::variant::texel::{TexelBuffer, TexelPlace};
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use naga::{
    AddressSpace, AtomicFunction, Expression, Function, GlobalVariable, Handle, ImageClass,
    ImageDimension, ImageQuery, Literal, Module, Range, Scalar, ScalarKind, Span, Statement, Type,
    TypeInner, VectorSize,
};
use spirv::{Decoration, Dim, ImageFormat, Op, StorageClass};
use wcontainers_gpu::{Reflection, ShaderStage, TexelBufferRoute, VariantKey};

type Result<T> = core::result::Result<T, ConvertError>;

/// A typed buffer or a storage image, as dxil-spirv declared it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Declared {
    /// Its descriptor set and binding in dxil-spirv's SPIR-V (its class and index, `cpp/dxil.cpp`).
    set: u32,
    binding: u32,
    /// A view a program writes (`RWBuffer`, `RWTexture*`), not a read-only one.
    storage: bool,
    /// A typed buffer, not a storage image.
    buffer: bool,
}

impl Declared {
    /// A typed buffer, not a storage image.
    pub(super) fn buffer(&self) -> bool {
        self.buffer
    }

    /// Its slot once numbered (`bindings`): the binding's place in its kind's range.
    fn slot(&self, module: &Module, var: Handle<GlobalVariable>) -> u32 {
        let kind = match self.storage {
            true => wcontainers_gpu::SlotKind::Storage,
            false => wcontainers_gpu::SlotKind::Texture,
        };
        let binding = module.global_variables[var]
            .binding
            .map_or(0, |b| b.binding);
        binding.wrapping_sub(kind.first_binding())
    }
}

/// How many pointer and array levels are looked through from a variable to its image type.
const ELEMENT_LEVELS: usize = 4;

/// Finds the typed buffers and storage images the SPIR-V declares, and rewrites the image types of
/// typed buffers and formatless storage images in place into ones naga's reader takes.
pub(super) fn prepare(words: &mut [u32]) -> Vec<Declared> {
    use super::words::{
        POINTER_TYPE, POINTER_WORDS, VARIABLE_STORAGE_CLASS, decorate, type_image, type_int,
    };
    let mut sets = BTreeMap::new();
    let mut bindings = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    let mut images = BTreeMap::new();
    // A pointer's or array's element type.
    let mut elements = BTreeMap::new();
    let mut variables = Vec::new();
    let mut at = super::words::HEADER;
    while at < words.len() {
        let count = (words[at] >> super::words::COUNT_SHIFT) as usize;
        let Some(w) = words.get(at..at + count).filter(|_| count > 0) else {
            break;
        };
        match Op::from_u32(w[0] & super::words::OPCODE_MASK) {
            Some(Op::Decorate) if count >= decorate::DECORATE_WORDS => {
                let (target, literal) = (w[decorate::TARGET], w[decorate::LITERAL]);
                if w[decorate::DECORATION] == Decoration::DescriptorSet as u32 {
                    sets.insert(target, literal);
                } else if w[decorate::DECORATION] == Decoration::Binding as u32 {
                    bindings.insert(target, literal);
                }
            }
            Some(Op::TypeInt) if count >= type_int::TYPE_INT_WORDS => {
                let kind = if w[type_int::SIGNEDNESS] != 0 {
                    ScalarKind::Sint
                } else {
                    ScalarKind::Uint
                };
                kinds.insert(w[1], kind);
            }
            Some(Op::TypeFloat) if count >= 3 => {
                kinds.insert(w[1], ScalarKind::Float);
            }
            Some(Op::TypeImage) if count >= type_image::TYPE_IMAGE_WORDS => {
                images.insert(w[1], at);
            }
            Some(Op::TypePointer) if count >= POINTER_WORDS => {
                elements.insert(w[1], w[POINTER_TYPE]);
            }
            Some(Op::TypeArray | Op::TypeRuntimeArray) if count >= 3 => {
                elements.insert(w[1], w[2]);
            }
            Some(Op::Variable)
                if count >= POINTER_WORDS
                    && w[VARIABLE_STORAGE_CLASS] == StorageClass::UniformConstant as u32 =>
            {
                variables.push((w[1], w[2]));
            }
            _ => {}
        }
        at += count;
    }
    let buffer = |words: &[u32], i: usize| words[i + type_image::DIM] == Dim::DimBuffer as u32;
    let storage = |words: &[u32], i: usize| words[i + type_image::SAMPLED] == type_image::STORAGE;
    let formatless = |words: &[u32], i: usize| {
        storage(words, i) && words[i + type_image::FORMAT] == ImageFormat::Unknown as u32
    };
    let mut declared = Vec::new();
    for (ty, id) in variables {
        let mut element = ty;
        for _ in 0..ELEMENT_LEVELS {
            match elements.get(&element) {
                Some(&e) => element = e,
                None => break,
            }
        }
        let (Some(&i), Some(&binding)) = (images.get(&element), bindings.get(&id)) else {
            continue;
        };
        let storage = storage(words, i);
        if buffer(words, i) || storage {
            declared.push(Declared {
                set: sets.get(&id).copied().unwrap_or(0),
                binding,
                storage,
                buffer: buffer(words, i),
            });
        }
    }
    for &i in images.values() {
        if formatless(words, i) {
            let placeholder = match kinds.get(&words[i + type_image::SAMPLED_TYPE]) {
                Some(ScalarKind::Float) => ImageFormat::R32f,
                Some(ScalarKind::Sint) => ImageFormat::R32i,
                Some(ScalarKind::Uint) => ImageFormat::R32ui,
                _ => continue,
            };
            words[i + type_image::FORMAT] = placeholder as u32;
        }
        if buffer(words, i) {
            words[i + type_image::DIM] = Dim::Dim1D as u32;
        }
    }
    declared
}

/// Declares the elements an array split into (`arrays::split`), at `elements`, as the array at
/// `array` was declared.
pub(super) fn declare_elements(
    declared: &mut Vec<Declared>,
    array: naga::ResourceBinding,
    elements: core::ops::Range<u32>,
) {
    let Some(&d) = declared
        .iter()
        .find(|d| (d.set, d.binding) == (array.group, array.binding))
    else {
        return;
    };
    declared.extend(elements.map(|binding| Declared { binding, ..d }));
}

/// Each declared image's global variable, matched by set, binding and kind while the bindings are
/// still dxil-spirv's.
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
            .find(|d| (d.set, d.binding, d.storage) == (b.group, b.binding, storage))
        {
            found.push((h, *d));
        }
    }
    found
}

/// An atomic operation on a typed buffer's element or a storage image's texel. naga's reader leaves
/// it on the image indexed by its whole coordinate (the IR has no image atomic with a result);
/// [`texel_atomics`] makes it a call of a function of its own, whose body [`finish`] writes once the
/// image's route is known.
pub(super) struct TexelAtomic {
    function: Handle<Function>,
    image: Handle<GlobalVariable>,
    fun: AtomicFunction,
}

/// The image global a texel pointer from the reader indexes, if `e` is one.
fn texel_pointer(
    module: &Module,
    f: &Function,
    e: Handle<Expression>,
) -> Option<Handle<GlobalVariable>> {
    let Expression::Access { base, .. } = f.expressions[e] else {
        return None;
    };
    match f.expressions[base] {
        Expression::GlobalVariable(g)
            if matches!(
                module.types[module.global_variables[g].ty].inner,
                TypeInner::Image { .. }
            ) =>
        {
            Some(g)
        }
        _ => None,
    }
}

/// The atomic operations on texel pointers in one function: (the operation's pointer, the image,
/// the coordinate, the operation, the value, the result).
type Found = (
    Handle<Expression>,
    Handle<GlobalVariable>,
    Handle<Expression>,
    AtomicFunction,
    Handle<Expression>,
    Option<Handle<Expression>>,
);

fn found_in(module: &Module, f: &Function) -> Result<Vec<Found>> {
    let pointers = f
        .expressions
        .iter()
        .filter(|&(h, _)| texel_pointer(module, f, h).is_some())
        .count();
    let mut found = Vec::new();
    let mut stack = vec![&f.body];
    while let Some(block) = stack.pop() {
        for s in block.iter() {
            match *s {
                Statement::Atomic {
                    pointer,
                    fun,
                    value,
                    result,
                } => {
                    if let Some(g) = texel_pointer(module, f, pointer) {
                        let Expression::Access { index, .. } = f.expressions[pointer] else {
                            unreachable!("a texel pointer is an access");
                        };
                        found.push((pointer, g, index, fun, value, result));
                    }
                }
                Statement::Block(ref b) => stack.push(b),
                Statement::If {
                    ref accept,
                    ref reject,
                    ..
                } => stack.extend([accept, reject]),
                Statement::Loop {
                    ref body,
                    ref continuing,
                    ..
                } => stack.extend([body, continuing]),
                Statement::Switch { ref cases, .. } => stack.extend(cases.iter().map(|c| &c.body)),
                _ => {}
            }
        }
    }
    // Each texel pointer serves one atomic operation; atomic loads and stores read as plain loads
    // and stores of it.
    let mut used: Vec<_> = found.iter().map(|a| a.0).collect();
    used.sort();
    used.dedup();
    if used.len() != pointers || used.len() != found.len() {
        return Err(ConvertError::Unsupported(crate::refusal::TEXEL_POINTER_USE));
    }
    Ok(found)
}

/// Makes each atomic operation on a texel pointer a call of a function taking the coordinate, the
/// value and any comparand and giving what the operation gives, so the module can be typed until
/// [`finish`] writes those functions. Any other use of a texel pointer is refused.
pub(super) fn texel_atomics(module: &mut Module) -> Result<Vec<TexelAtomic>> {
    let handles: Vec<Handle<Function>> = module.functions.iter().map(|(h, _)| h).collect();
    let n = handles.len();
    let count = n + module.entry_points.len();
    let mut found = Vec::new();
    for i in 0..count {
        let f = match handles.get(i) {
            Some(&h) => &module.functions[h],
            None => &module.entry_points[i - n].function,
        };
        found.push(found_in(module, f)?);
    }
    if found.iter().all(|f| f.is_empty()) {
        return Ok(Vec::new());
    }
    // Each texel pointer becomes its image, so the module can be typed; then each operation's
    // stand-in, one per image, operation and operand types.
    // (image, operation, compared, coordinate type, value type, result type)
    type Shape = (
        Handle<GlobalVariable>,
        core::mem::Discriminant<AtomicFunction>,
        bool,
        Handle<Type>,
        Handle<Type>,
        Option<Handle<Type>>,
    );
    let mut shapes: Vec<(Shape, AtomicFunction)> = Vec::new();
    // Per function, each operation's shape.
    let mut of: Vec<Vec<usize>> = Vec::new();
    for (i, atomics) in found.iter().enumerate() {
        let f = match handles.get(i) {
            Some(&h) => &mut module.functions[h],
            None => &mut module.entry_points[i - n].function,
        };
        for &(pointer, g, ..) in atomics {
            *f.expressions.get_mut(pointer) = Expression::GlobalVariable(g);
        }
        let f = match handles.get(i) {
            Some(&h) => &module.functions[h],
            None => &module.entry_points[i - n].function,
        };
        let mut typifier = naga::front::Typifier::new();
        let ctx = naga::proc::ResolveContext::with_locals(module, &f.local_variables, &f.arguments);
        let mut inners = Vec::new();
        for &(_, _, coordinate, _, value, result) in atomics {
            let mut inner = |e: Handle<Expression>| -> Result<TypeInner> {
                typifier
                    .grow(e, &f.expressions, &ctx)
                    .map_err(|e| ConvertError::Malformed(alloc::format!("{e}")))?;
                Ok(typifier.get(e, &module.types).clone())
            };
            let c = inner(coordinate)?;
            let v = inner(value)?;
            let r = match result.map(|r| &f.expressions[r]) {
                None => None,
                Some(&Expression::AtomicResult { ty, .. }) => Some(ty),
                Some(_) => return Err(ConvertError::Malformed("an atomic's result".into())),
            };
            inners.push((c, v, r));
        }
        let mut each = Vec::new();
        for (&(_, g, _, fun, ..), (c, v, r)) in atomics.iter().zip(inners) {
            let insert = |module: &mut Module, inner| {
                module
                    .types
                    .insert(Type { name: None, inner }, Span::UNDEFINED)
            };
            let c = insert(module, c);
            let v = insert(module, v);
            let r = r.map(|r| spelled(module, r));
            let compared = matches!(fun, AtomicFunction::Exchange { compare: Some(_) });
            let shape = (g, core::mem::discriminant(&fun), compared, c, v, r);
            let k = match shapes.iter().position(|(s, _)| *s == shape) {
                Some(k) => k,
                None => {
                    shapes.push((shape, fun));
                    shapes.len() - 1
                }
            };
            each.push(k);
        }
        of.push(each);
    }
    let (made, moved) = functions_first(module, |module| {
        shapes
            .iter()
            .enumerate()
            .map(|(k, &((image, _, compared, c, v, r), fun))| {
                let mut b = Builder::new(&alloc::format!("texel_atomic_{k}"));
                let mut arguments = vec![("coordinate", c), ("value", v)];
                if compared {
                    arguments.push(("compare", v));
                }
                for (name, ty) in arguments {
                    b.f.arguments.push(naga::FunctionArgument {
                        name: Some(name.into()),
                        ty,
                        binding: None,
                    });
                }
                b.f.result = r.map(|ty| naga::FunctionResult { ty, binding: None });
                let zero = r.map(|ty| b.expr(Expression::ZeroValue(ty)));
                b.ret(zero);
                let function = module.functions.append(b.finish(), Span::UNDEFINED);
                TexelAtomic {
                    function,
                    image,
                    fun,
                }
            })
            .collect::<Vec<_>>()
    });
    for (i, (atomics, shapes)) in found.iter().zip(of).enumerate() {
        let f = match moved.get(i) {
            Some(&h) => &mut module.functions[h],
            None => &mut module.entry_points[i - n].function,
        };
        let Function {
            body, expressions, ..
        } = f;
        each_block(body, &mut |block| {
            for s in block.iter_mut() {
                let Statement::Atomic { pointer, fun, .. } = *s else {
                    continue;
                };
                let Some(at) = atomics.iter().position(|a| a.0 == pointer) else {
                    continue;
                };
                let (_, _, coordinate, _, value, result) = atomics[at];
                let function = made[shapes[at]].function;
                let mut arguments = vec![coordinate, value];
                if let AtomicFunction::Exchange { compare: Some(c) } = fun {
                    arguments.push(c);
                }
                *s = Statement::Call {
                    function,
                    arguments,
                    result,
                };
                if let Some(r) = result {
                    *expressions.get_mut(r) = Expression::CallResult(function);
                }
            }
        });
    }
    Ok(made)
}

/// How a function uses one image.
#[derive(Clone, Copy, Default)]
struct Uses {
    load: bool,
    store: bool,
    /// Size and layer count queries.
    size: bool,
    /// Atomic operations on its elements or texels.
    atomic: bool,
    /// Any other use: other queries, samples, passing it to a function.
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
fn uses(
    module: &Module,
    globals: &[(Handle<GlobalVariable>, Declared)],
    atomics: &[TexelAtomic],
) -> Vec<Uses> {
    let mut uses = vec![Uses::default(); globals.len()];
    for a in atomics {
        if let Some(i) = globals.iter().position(|(h, _)| *h == a.image) {
            uses[i].atomic = true;
        }
    }
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
                    query: ImageQuery::Size { .. } | ImageQuery::NumLayers,
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
    let slot = d.slot(module, var);
    let format = variant
        .storage_formats
        .iter()
        .find(|(s, _)| *s == slot)
        .ok_or(ConvertError::Unsupported(crate::refusal::STORAGE_FORMAT))?
        .1;
    let format = storage_format(format).ok_or(ConvertError::Unsupported(
        crate::refusal::STORAGE_FORMAT_ABSENT,
    ))?;
    if declared != Some(storage_kind(format)) {
        return Err(ConvertError::Unsupported(
            crate::refusal::STORAGE_FORMAT_KIND,
        ));
    }
    let access = match (uses.load, uses.store) {
        (true, false) => naga::StorageAccess::LOAD,
        (false, _) => naga::StorageAccess::STORE,
        (true, true) => {
            use naga::StorageFormat as S;
            if !matches!(format, S::R32Float | S::R32Uint | S::R32Sint) {
                return Err(ConvertError::Unsupported(
                    crate::refusal::READ_WRITE_STORAGE_FORMAT,
                ));
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

/// One typed buffer or storage image read through a buffer, retyped for its route, with the
/// functions its uses need.
struct Texels {
    var: Handle<GlobalVariable>,
    buffer: TexelBuffer,
    /// `vec4` of the texel's scalar, which stores take.
    vec4: Handle<Type>,
    /// A storage image's dimension and whether it is an array; `None` for a typed buffer.
    image: Option<(ImageDimension, bool)>,
    /// Typed buffers: (index) -> vec4. Images: (coordinate, layer if an array) -> vec4.
    load: Option<Handle<Function>>,
    /// As `load`, with the value last.
    store: Option<Handle<Function>>,
    /// Typed buffers: the length. Images: the size query's value.
    length: Option<Handle<Function>>,
    /// Images that are arrays: the layer count.
    layers: Option<Handle<Function>>,
}

/// The scalar of a typed view's words where they are atomic: where an atomic operation uses them
/// (a float view's bits as u32), or where elements narrower than a word are written.
fn word_scalar(uses: Uses, declared: ScalarKind, size: u32) -> Option<Scalar> {
    match (uses.atomic, declared) {
        (true, ScalarKind::Sint) => Some(Scalar::I32),
        (true, _) => Some(Scalar::U32),
        (false, _) => (uses.store && TexelBuffer::narrow(size)).then_some(Scalar::U32),
    }
}

/// Retypes `var` as a storage buffer of words (`atomic` as [`TexelBuffer::words`]) the program
/// reads, and writes where it stores or runs atomics.
fn as_words(module: &mut Module, var: Handle<GlobalVariable>, uses: Uses, atomic: Option<Scalar>) {
    let access = if uses.store || uses.atomic {
        naga::StorageAccess::LOAD | naga::StorageAccess::STORE
    } else {
        naga::StorageAccess::LOAD
    };
    let words = TexelBuffer::words(module, atomic);
    let global = &mut module.global_variables[var];
    global.ty = words;
    global.space = AddressSpace::Storage { access };
}

/// `vec4` of `scalar`.
fn vec4(module: &mut Module, scalar: Scalar) -> Handle<Type> {
    module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Vector {
                size: VectorSize::Quad,
                scalar,
            },
        },
        Span::UNDEFINED,
    )
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
        TexelBufferRoute::Native => {
            return Err(ConvertError::Unsupported(crate::refusal::TYPED_BUFFER));
        }
        TexelBufferRoute::Storage => false,
        TexelBufferRoute::Texture if !d.storage => true,
        TexelBufferRoute::Texture => {
            return Err(ConvertError::Unsupported(
                crate::refusal::TYPED_BUFFER_STORAGE_AS_TEXTURE,
            ));
        }
    };
    if uses.other {
        return Err(ConvertError::Unsupported(crate::refusal::TYPED_BUFFER_USE));
    }
    let formats = if d.storage {
        &variant.storage_formats
    } else {
        &variant.texel_formats
    };
    let slot = d.slot(module, var);
    let format = formats
        .iter()
        .find(|(s, _)| *s == slot)
        .ok_or(ConvertError::Unsupported(
            crate::refusal::TYPED_BUFFER_FORMAT,
        ))?
        .1;
    let declared = texel_kind(&module.types[module.global_variables[var].ty].inner)
        .ok_or_else(|| ConvertError::Malformed("a typed buffer's type".into()))?;
    let layout = TexelBuffer::layout(format, declared)?;
    let atomic = word_scalar(uses, declared, layout.2);
    let place = if texture {
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
        let global = &mut module.global_variables[var];
        global.ty = ty;
        global.space = AddressSpace::Handle;
        TexelPlace::Texture
    } else {
        as_words(module, var, uses, atomic);
        TexelPlace::Words
    };
    let binding = module.global_variables[var]
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
    let vec4 = vec4(module, buffer.scalar());
    Ok((buffer, vec4))
}

/// Retypes a storage image read and written through a buffer of its texels
/// (`VariantKey::texels_through_buffer`), its format the bound view's, laid out as
/// `HelperValue::TexelLayout` says; as the DXBC route declares it.
fn through_image(
    module: &mut Module,
    variant: &VariantKey,
    var: Handle<GlobalVariable>,
    d: &Declared,
    uses: Uses,
    stage: ShaderStage,
) -> Result<(TexelBuffer, Handle<Type>, (ImageDimension, bool))> {
    let inner = &module.types[module.global_variables[var].ty].inner;
    let TypeInner::Image {
        dim,
        arrayed,
        class,
    } = *inner
    else {
        return Err(ConvertError::Malformed("a storage image's type".into()));
    };
    if class.is_multisampled() {
        return Err(ConvertError::Unsupported(
            crate::refusal::MULTISAMPLED_STORAGE,
        ));
    }
    if dim == ImageDimension::Cube {
        return Err(ConvertError::Malformed("a cube storage image".into()));
    }
    if uses.other {
        return Err(ConvertError::Unsupported(
            crate::refusal::TEXELS_THROUGH_BUFFER_USE,
        ));
    }
    let declared = texel_kind(inner)
        .ok_or_else(|| ConvertError::Malformed("a storage image's type".into()))?;
    let slot = d.slot(module, var);
    let format = variant
        .storage_formats
        .iter()
        .find(|(s, _)| *s == slot)
        .ok_or(ConvertError::Unsupported(crate::refusal::STORAGE_FORMAT))?
        .1;
    let layout = TexelBuffer::layout(format, declared).map_err(|e| match e {
        ConvertError::Unsupported(crate::refusal::TYPED_BUFFER_FORMAT_ABSENT) => {
            ConvertError::Unsupported(crate::refusal::STORAGE_FORMAT_ABSENT)
        }
        ConvertError::Unsupported(crate::refusal::TYPED_BUFFER_FORMAT_KIND) => {
            ConvertError::Unsupported(crate::refusal::STORAGE_FORMAT_KIND)
        }
        e => e,
    })?;
    let atomic = word_scalar(uses, declared, layout.2);
    as_words(module, var, uses, atomic);
    let binding = module.global_variables[var]
        .binding
        .ok_or_else(|| ConvertError::Malformed("a storage image without a binding".into()))?;
    let place = TexelPlace::Image(slot);
    let buffer = TexelBuffer::declared(
        var,
        layout,
        atomic,
        place,
        stage,
        binding.group,
        binding.binding,
    );
    let vec4 = vec4(module, buffer.scalar());
    Ok((buffer, vec4, (dim, arrayed)))
}

/// The number of coordinate components of a storage image of `dim`.
fn components(dim: ImageDimension) -> u32 {
    match dim {
        ImageDimension::D1 => 1,
        ImageDimension::D2 => 2,
        _ => 3,
    }
}

/// `u32` or a vector of `n` of them.
fn uints(module: &mut Module, n: u32) -> Handle<Type> {
    let inner = match n {
        1 => TypeInner::Scalar(Scalar::U32),
        n => TypeInner::Vector {
            size: match n {
                2 => VectorSize::Bi,
                3 => VectorSize::Tri,
                _ => VectorSize::Quad,
            },
            scalar: Scalar::U32,
        },
    };
    module
        .types
        .insert(Type { name: None, inner }, Span::UNDEFINED)
}

/// A texel's index in the buffer, from its coordinate's `n` u32 components and a layer: as the
/// layout has them, (x, y, layer or depth), a 1D array's layer third.
fn texel_index(
    b: &mut Builder,
    at: Handle<Function>,
    coordinate: Expr,
    n: u32,
    layer: Option<Expr>,
) -> Expr {
    let mut xyz: Vec<Expr> = match n {
        1 => vec![coordinate],
        n => (0..n).map(|i| b.field(coordinate, i)).collect(),
    };
    if let Some(l) = layer {
        while xyz.len() < 2 {
            xyz.push(b.u32(0));
        }
        xyz.push(l);
    }
    while xyz.len() < 3 {
        xyz.push(b.u32(0));
    }
    b.call(at, xyz, true).expect("returns")
}

impl Texels {
    /// Makes the functions a storage image's uses need, over its texel buffer's.
    fn image_functions(&mut self, module: &mut Module, reflection: &mut Reflection, uses: Uses) {
        let (dim, arrayed) = self.image.expect("a storage image");
        let n = components(dim);
        let coordinate_ty = uints(module, n);
        let u32_ty = uints(module, 1);
        let name = alloc::format!("texels_{}", self.var.index());
        let arguments = |b: &mut Builder| {
            let mut names = vec![("coordinate", coordinate_ty)];
            if arrayed {
                names.push(("layer", u32_ty));
            }
            for (name, ty) in names {
                b.f.arguments.push(naga::FunctionArgument {
                    name: Some(name.into()),
                    ty,
                    binding: None,
                });
            }
            let c = b.argument(0);
            let layer = arrayed.then(|| b.argument(1));
            (c, layer)
        };
        if uses.load {
            let at = self.buffer.at(module, reflection);
            let load = self.buffer.load(module, reflection);
            let mut b = Builder::new(&alloc::format!("{name}_load"));
            let (c, layer) = arguments(&mut b);
            b.f.result = Some(naga::FunctionResult {
                ty: self.vec4,
                binding: None,
            });
            let i = texel_index(&mut b, at, c, n, layer);
            let v = b.call(load, vec![i], true);
            b.ret(v);
            self.load = Some(module.functions.append(b.finish(), Span::UNDEFINED));
        }
        if uses.store {
            let at = self.buffer.at(module, reflection);
            let store = self.buffer.store(module, reflection);
            let mut b = Builder::new(&alloc::format!("{name}_store"));
            let (c, layer) = arguments(&mut b);
            b.f.arguments.push(naga::FunctionArgument {
                name: Some("value".into()),
                ty: self.vec4,
                binding: None,
            });
            let value = b.argument(b.f.arguments.len() as u32 - 1);
            let i = texel_index(&mut b, at, c, n, layer);
            b.call(store, vec![i, value], false);
            self.store = Some(module.functions.append(b.finish(), Span::UNDEFINED));
        }
        if uses.size {
            let size = self.buffer.size(module, reflection);
            let shaped = uints(module, n);
            let mut b = Builder::new(&alloc::format!("{name}_size"));
            b.f.result = Some(naga::FunctionResult {
                ty: shaped,
                binding: None,
            });
            let s = b.call(size, Vec::new(), true).expect("returns");
            let v = match n {
                1 => b.field(s, 0),
                2 => {
                    let x = b.field(s, 0);
                    let y = b.field(s, 1);
                    b.compose(shaped, vec![x, y])
                }
                _ => s,
            };
            b.ret(Some(v));
            self.length = Some(module.functions.append(b.finish(), Span::UNDEFINED));
            if arrayed {
                let mut b = Builder::new(&alloc::format!("{name}_layers"));
                b.f.result = Some(naga::FunctionResult {
                    ty: u32_ty,
                    binding: None,
                });
                let s = b.call(size, Vec::new(), true).expect("returns");
                let v = b.field(s, 2);
                b.ret(Some(v));
                self.layers = Some(module.functions.append(b.finish(), Span::UNDEFINED));
            }
        }
    }

    /// Writes the body of the stand-in `a.function` (taking the texel pointer's coordinate, the value
    /// and any comparand): the operation on the element or texel, by the buffer's atomic function.
    fn atomic(
        &mut self,
        module: &mut Module,
        reflection: &mut Reflection,
        a: &TexelAtomic,
    ) -> Result<()> {
        let fun = self.buffer.atomic(module, reflection, a.fun)?;
        let at = self.image.map(|_| self.buffer.at(module, reflection));
        let stand_in = &module.functions[a.function];
        let (name, arguments, result) = (
            stand_in.name.clone(),
            stand_in.arguments.clone(),
            stand_in.result.clone(),
        );
        let natural = self.buffer.scalar();
        let kind = |module: &Module, ty: Handle<Type>| match module.types[ty].inner {
            TypeInner::Scalar(s) | TypeInner::Vector { scalar: s, .. } => Some(s.kind),
            _ => None,
        };
        let coordinate_kind = kind(module, arguments[0].ty)
            .ok_or_else(|| ConvertError::Malformed("a texel pointer's coordinate".into()))?;
        let value_kind = kind(module, arguments[1].ty)
            .ok_or_else(|| ConvertError::Malformed("an atomic's value".into()))?;
        let mut b = Builder::new("");
        b.f.name = name;
        b.f.arguments = arguments.clone();
        b.f.result = result.clone();
        let mut c = b.argument(0);
        if coordinate_kind != ScalarKind::Uint {
            c = b.bitcast(c, ScalarKind::Uint);
        }
        let index = match (self.image, at) {
            (Some((dim, arrayed)), Some(at)) => {
                let n = components(dim);
                let all = n + arrayed as u32;
                let (xyz, layer) = match (arrayed, all) {
                    (false, _) => (c, None),
                    (true, 2) => {
                        let x = b.field(c, 0);
                        let l = b.field(c, 1);
                        (x, Some(l))
                    }
                    (true, _) => {
                        let ty = uints(module, n);
                        let parts = (0..n).map(|i| b.field(c, i)).collect();
                        let xy = b.compose(ty, parts);
                        let l = b.field(c, n);
                        (xy, Some(l))
                    }
                };
                texel_index(&mut b, at, xyz, n, layer)
            }
            _ => c,
        };
        let as_natural = |b: &mut Builder, v: Expr| match value_kind == natural.kind {
            true => v,
            false => b.bitcast(v, natural.kind),
        };
        let value = b.argument(1);
        let mut call = vec![index, as_natural(&mut b, value)];
        let compare = (arguments.len() > 2).then(|| b.argument(2));
        if let Some(c) = compare {
            call.push(as_natural(&mut b, c));
        }
        let old = b.call(fun, call, true).expect("returns");
        let old = match value_kind == natural.kind {
            true => old,
            false => b.bitcast(old, value_kind),
        };
        let r = match (result, compare) {
            (None, _) => None,
            (Some(_), None) => Some(old),
            (Some(r), Some(c)) => {
                let exchanged = b.binary(naga::BinaryOperator::Equal, old, c);
                Some(b.compose(r.ty, vec![old, exchanged]))
            }
        };
        b.ret(r);
        module.functions[a.function] = b.finish();
        Ok(())
    }
}

/// What one function does with typed views, read while they still have their image types.
#[derive(Default)]
struct Plan {
    /// (load, the view, its coordinate, its layer)
    loads: Vec<(Handle<Expression>, usize, Operand, Option<Operand>)>,
    /// (query, the view, whether it is the layer count)
    sizes: Vec<(Handle<Expression>, usize, bool)>,
    stores: Vec<Store>,
}

/// An operand of a texel function and whether it is signed.
type Operand = (Handle<Expression>, bool);

/// A store into a typed view.
struct Store {
    view: usize,
    coordinate: Operand,
    layer: Option<Operand>,
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
    let view = |e| image_of(f, e, globals).and_then(|i| typed[i]);
    let mut typifier = naga::front::Typifier::new();
    let ctx = naga::proc::ResolveContext::with_locals(module, &f.local_variables, &f.arguments);
    let mut kind = |e: Handle<Expression>| -> Result<(bool, u32)> {
        typifier
            .grow(e, &f.expressions, &ctx)
            .map_err(|e| ConvertError::Malformed(alloc::format!("{e}")))?;
        Ok(match *typifier.get(e, &module.types) {
            TypeInner::Scalar(s) => (s.kind == ScalarKind::Sint, 1),
            TypeInner::Vector { size, scalar } => (scalar.kind == ScalarKind::Sint, size as u32),
            _ => return Err(ConvertError::Malformed("a typed view's operand".into())),
        })
    };
    let mut plan = Plan::default();
    for (h, e) in f.expressions.iter() {
        match *e {
            Expression::ImageLoad {
                image,
                coordinate,
                array_index,
                ..
            } => {
                if let Some(t) = view(image) {
                    let c = (coordinate, kind(coordinate)?.0);
                    let layer = match array_index {
                        Some(l) => Some((l, kind(l)?.0)),
                        None => None,
                    };
                    plan.loads.push((h, t, c, layer));
                }
            }
            Expression::ImageQuery { image, query } => {
                if let Some(t) = view(image) {
                    plan.sizes.push((h, t, query == ImageQuery::NumLayers));
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
                    array_index,
                    value,
                } => {
                    if let Some(t) = view(*image) {
                        stores.push((t, *coordinate, *array_index, *value));
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
    for (t, coordinate, layer, value) in stores {
        let coordinate = (coordinate, kind(coordinate)?.0);
        let layer = match layer {
            Some(l) => Some((l, kind(l)?.0)),
            None => None,
        };
        let components = kind(value)?.1;
        plan.stores.push(Store {
            view: t,
            coordinate,
            layer,
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

/// The operands as the texel functions' u32 values, appended where signed, with the range of what
/// was appended.
fn unsigned(
    f: &mut Function,
    operands: &[Operand],
) -> (Vec<Handle<Expression>>, Option<Range<Expression>>) {
    let start = f.expressions.len();
    let values = operands
        .iter()
        .map(|&(e, signed)| match signed {
            false => e,
            true => append(
                f,
                Expression::As {
                    expr: e,
                    kind: ScalarKind::Uint,
                    convert: None,
                },
            ),
        })
        .collect();
    let end = f.expressions.len();
    let range =
        (end > start).then(|| Range::from_index_range(start as u32..end as u32, &f.expressions));
    (values, range)
}

/// Rewrites the typed views' loads, size queries and stores in `f` as calls of their functions.
fn rewrite(f: &mut Function, plan: &Plan, typed: &[Texels]) {
    let mut calls = Vec::new();
    for &(h, t, coordinate, layer) in &plan.loads {
        let operands: Vec<Operand> = core::iter::once(coordinate).chain(layer).collect();
        let (arguments, pre) = unsigned(f, &operands);
        calls.push(Placed {
            result: h,
            function: typed[t].load.expect("made for its loads"),
            arguments,
            pre,
        });
    }
    for &(h, t, layers) in &plan.sizes {
        let function = match layers {
            true => typed[t].layers,
            false => typed[t].length,
        };
        calls.push(Placed {
            result: h,
            function: function.expect("made for its queries"),
            arguments: Vec::new(),
            pre: None,
        });
    }
    place_calls(f, &calls);
    // Each store's operands and value, appended now and emitted just before its call: the zero
    // literal (never emitted) first, then the emitted expressions in one range.
    let mut stores = Vec::new();
    for s in &plan.stores {
        let t = s.view;
        let zero = (s.components != 4).then(|| {
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
        let operands: Vec<Operand> = core::iter::once(s.coordinate).chain(s.layer).collect();
        let (mut arguments, _) = unsigned(f, &operands);
        let padded = match zero {
            None => s.value,
            Some(zero) => {
                let parts = (0..4)
                    .map(|c| match (c < s.components, s.components) {
                        (true, 1) => s.value,
                        (true, _) => append(
                            f,
                            Expression::AccessIndex {
                                base: s.value,
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
        arguments.push(padded);
        let end = f.expressions.len();
        let range = (end > start)
            .then(|| Range::from_index_range(start as u32..end as u32, &f.expressions));
        stores.push((s, arguments, range));
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
                array_index,
                value,
            } => stores.iter().enumerate().position(|(n, (st, ..))| {
                !taken[n]
                    && (st.coordinate.0, st.layer.map(|l| l.0), st.value)
                        == (*coordinate, *array_index, *value)
                    && matches!(expressions[*image], Expression::GlobalVariable(g) if g == typed[st.view].var)
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
            let (st, arguments, range) = &stores[n];
            if let Some(r) = range {
                block.push(Statement::Emit(r.clone()), span);
            }
            block.push(
                Statement::Call {
                    function: typed[st.view].store.expect("made for its stores"),
                    arguments: arguments.clone(),
                    result: None,
                },
                span,
            );
        }
    });
}

/// An unordered access view's counter, which dxil-spirv declares as a typed buffer: the interface's
/// counter, one atomic word at the counter binding of the view's slot, which only atomic operations
/// use.
fn counter(module: &mut Module, var: Handle<GlobalVariable>, uses: Uses) -> Result<()> {
    if uses.load || uses.store || uses.size || uses.other {
        return Err(ConvertError::Unsupported(crate::refusal::COUNTER_USE));
    }
    let ty = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Atomic(Scalar::U32),
        },
        Span::UNDEFINED,
    );
    let global = &mut module.global_variables[var];
    global.ty = ty;
    global.space = AddressSpace::Storage {
        access: naga::StorageAccess::LOAD | naga::StorageAccess::STORE,
    };
    Ok(())
}

/// Writes the body of the stand-in `a.function` on counter `var`: the operation on its word.
fn counter_atomic(module: &mut Module, var: Handle<GlobalVariable>, a: &TexelAtomic) -> Result<()> {
    if matches!(a.fun, AtomicFunction::Exchange { compare: Some(_) }) {
        return Err(ConvertError::Malformed("a counter compared".into()));
    }
    let stand_in = &module.functions[a.function];
    let (name, arguments, result) = (
        stand_in.name.clone(),
        stand_in.arguments.clone(),
        stand_in.result.clone(),
    );
    let signed = matches!(
        module.types[arguments[1].ty].inner,
        TypeInner::Scalar(Scalar {
            kind: ScalarKind::Sint,
            ..
        })
    );
    let u32_ty = uints(module, 1);
    let mut b = Builder::new("");
    b.f.name = name;
    b.f.arguments = arguments;
    b.f.result = result.clone();
    let p = b.global(var);
    let mut value = b.argument(1);
    if signed {
        value = b.bitcast(value, ScalarKind::Uint);
    }
    let old = b.expr(Expression::AtomicResult {
        ty: u32_ty,
        comparison: false,
    });
    b.stmt(Statement::Atomic {
        pointer: p,
        fun: a.fun,
        value,
        result: Some(old),
    });
    let old = match signed {
        true => b.bitcast(old, ScalarKind::Sint),
        false => old,
    };
    b.ret(result.map(|_| old));
    module.functions[a.function] = b.finish();
    Ok(())
}

/// Moves each stand-in of `atomics` where `moved` (a [`functions_first`] result) put it.
pub(super) fn remap(atomics: &mut [TexelAtomic], moved: &[Handle<Function>]) {
    for a in atomics {
        a.function = moved[a.function.index()];
    }
}

/// Brings the reader's typed buffers and storage images to their final forms: typed buffers by the
/// variant's route, storage images read through buffers where the variant says, other storage
/// images of the bound views' formats, and the stand-ins of `atomics` written. Atomic operations on a
/// storage image are refused unless it is read through a buffer. Runs after the bindings are
/// numbered and the helper block for the draw's bases is made.
pub(super) fn finish(
    module: &mut Module,
    variant: &VariantKey,
    globals: &[(Handle<GlobalVariable>, Declared)],
    atomics: &mut [TexelAtomic],
    stage: ShaderStage,
    reflection: &mut Reflection,
) -> Result<()> {
    if globals.is_empty() {
        return Ok(());
    }
    let uses = uses(module, globals, atomics);
    let through = |module: &Module, i: usize| {
        let (var, d) = &globals[i];
        let slot = d.slot(module, *var);
        !d.buffer && slot < 64 && variant.texels_through_buffer >> slot & 1 != 0
    };
    // Plans are read while the views still have their image types.
    let mut typed_at = vec![None; globals.len()];
    let mut n = 0;
    for (i, (_, d)) in globals.iter().enumerate() {
        if d.set != super::COUNTER && (d.buffer || through(module, i)) {
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
    let mut views = Vec::new();
    let mut counters = Vec::new();
    for (i, (var, d)) in globals.iter().enumerate() {
        if d.set == super::COUNTER {
            counter(module, *var, uses[i])?;
            counters.push(*var);
        } else if d.buffer {
            let (buffer, vec4) = typed_buffer(module, variant, *var, d, uses[i], stage)?;
            views.push((i, buffer, vec4, None));
        } else if through(module, i) {
            let (buffer, vec4, image) = through_image(module, variant, *var, d, uses[i], stage)?;
            views.push((i, buffer, vec4, Some(image)));
        } else if uses[i].atomic {
            // WebGPU has no atomics on storage textures.
            return Err(ConvertError::Unsupported("StorageImageAtomics"));
        } else {
            // dxil-spirv's own format (R32 where the program reads) is not the view's.
            storage_image(module, variant, *var, d, uses[i])?;
        }
    }
    if views.is_empty() && atomics.is_empty() {
        return Ok(());
    }
    let (typed, moved) = functions_first(module, |module| {
        views
            .into_iter()
            .map(|(i, buffer, vec4, image)| -> Result<Texels> {
                let u = uses[i];
                let mut t = Texels {
                    var: globals[i].0,
                    buffer,
                    vec4,
                    image,
                    load: None,
                    store: None,
                    length: None,
                    layers: None,
                };
                match image {
                    Some(_) => t.image_functions(module, reflection, u),
                    None => {
                        t.load = u.load.then(|| t.buffer.load(module, reflection));
                        t.store = u.store.then(|| t.buffer.store(module, reflection));
                        t.length = u.size.then(|| t.buffer.length(module, reflection));
                    }
                }
                // The functions the stand-ins call, made here so they precede them.
                for a in atomics.iter().filter(|a| a.image == t.var) {
                    t.buffer.atomic(module, reflection, a.fun)?;
                    if image.is_some() {
                        t.buffer.at(module, reflection);
                    }
                }
                Ok(t)
            })
            .collect::<Result<Vec<_>>>()
    });
    let mut typed = typed?;
    remap(atomics, &moved);
    for a in atomics.iter() {
        if counters.contains(&a.image) {
            counter_atomic(module, a.image, a)?;
            continue;
        }
        let t = typed
            .iter_mut()
            .find(|t| t.var == a.image)
            .ok_or(ConvertError::Unsupported(crate::refusal::TEXEL_POINTER_USE))?;
        t.atomic(module, reflection, a)?;
    }
    for (plan, h) in functions.iter().zip(moved) {
        rewrite(&mut module.functions[h], plan, &typed);
    }
    for (plan, ep) in entry_points.iter().zip(module.entry_points.iter_mut()) {
        rewrite(&mut ep.function, plan, &typed);
    }
    Ok(())
}

/// A structure WGSL can name in place of a predeclared one (an atomic compare-exchange's result,
/// which WGSL spells only as the built-in's return): the same members under a declared name.
fn spelled(module: &mut Module, ty: Handle<Type>) -> Handle<Type> {
    if !module
        .special_types
        .predeclared_types
        .values()
        .any(|&t| t == ty)
    {
        return ty;
    }
    let inner = module.types[ty].inner.clone();
    module.types.insert(
        Type {
            name: Some(COMPARE_EXCHANGE_RESULT.into()),
            inner,
        },
        Span::UNDEFINED,
    )
}

/// The declared name of a compare-exchange's result returned from a function.
const COMPARE_EXCHANGE_RESULT: &str = "CompareExchangeResult";
