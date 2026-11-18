//! DXIL through dxil-spirv (C++, built by `build.rs`) to SPIR-V, read by naga's SPIR-V reader, then
//! given the binding rule of plan section 6.4 and stripped of what WGSL cannot express.

mod images;
mod nonfinite;

use crate::ConvertError;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::ffi::{c_char, c_int, c_void};
use naga::{
    AddressSpace, Binding, BuiltIn, Expression, Function, FunctionArgument, GlobalVariable, Handle,
    Literal, Module, Range, Scalar, ScalarKind, Span, Statement, StructMember, Type, TypeInner,
};
use wcontainers_gpu::{
    DrawBaseSource, HELPER_SET, HelperSlot, HelperValue, Interpolation, ReflectedBinding,
    Reflection, ScalarKind as ValueKind, ShaderStage, SlotKind, SourceStage, StageExtras,
    StageValue, SystemValue, VariantKey, helper_binding,
};

unsafe extern "C" {
    fn wcs_dxil_to_spirv(
        data: *const u8,
        size: usize,
        samples: u32,
        out: *mut *mut u32,
        out_words: *mut usize,
        table: *mut *mut u32,
        table_words: *mut usize,
        error: *mut *mut c_char,
    ) -> c_int;
    fn wcs_dxil_constant_sizes(
        data: *const u8,
        size: usize,
        out: *mut *mut u32,
        out_words: *mut usize,
        error: *mut *mut c_char,
    ) -> c_int;
    fn wcs_free(p: *mut c_void);
}

/// The reason in `error` from the C side, freed.
fn reason(error: *mut c_char) -> String {
    if error.is_null() {
        return String::from("dxil-spirv gave no reason");
    }
    // SAFETY: a NUL-terminated string from the C side, freed right after.
    let s = unsafe { core::ffi::CStr::from_ptr(error) }
        .to_string_lossy()
        .into();
    unsafe { wcs_free(error.cast()) };
    s
}

/// Words the C side allocated, copied out and freed.
fn words(out: *mut u32, words: usize) -> Vec<u32> {
    // SAFETY: `out` holds `words` words written by the C side, freed right after.
    let v = unsafe { core::slice::from_raw_parts(out, words) }.to_vec();
    unsafe { wcs_free(out.cast()) };
    v
}

/// A resource class of Direct3D 12, as the SPIR-V's descriptor set names it (`cpp/dxil.cpp`).
const CONSTANTS: u32 = 0;
const VIEW: u32 = 1;
const STORAGE: u32 = 2;
const SAMPLER: u32 = 3;
pub(super) const COUNTER: u32 = 4;

/// Each resource's (class, space, register), at the index the SPIR-V's binding names.
type Resources = Vec<[u32; 3]>;

/// DXIL to SPIR-V words, the targets' sample count fixed where `samples` is not zero, with the
/// resources the SPIR-V's bindings index.
fn to_spirv(dxil: &[u8], samples: u8) -> Result<(Vec<u32>, Resources), ConvertError> {
    let mut out = core::ptr::null_mut();
    let mut words = 0;
    let mut table = core::ptr::null_mut();
    let mut table_words = 0;
    let mut error = core::ptr::null_mut();
    // SAFETY: the pointers are valid for the call; the C side allocates `out` and `error` with malloc.
    let status = unsafe {
        wcs_dxil_to_spirv(
            dxil.as_ptr(),
            dxil.len(),
            samples.into(),
            &mut out,
            &mut words,
            &mut table,
            &mut table_words,
            &mut error,
        )
    };
    if status != 0 {
        return Err(ConvertError::Rejected(reason(error)));
    }
    let spirv = self::words(out, words);
    let resources = self::words(table, table_words).as_chunks::<3>().0.to_vec();
    Ok((spirv, resources))
}

/// (space, register, size in bytes) of every constant buffer the DXIL declares.
fn constant_sizes(dxil: &[u8]) -> Result<Vec<u32>, ConvertError> {
    let mut out = core::ptr::null_mut();
    let mut n = 0;
    let mut error = core::ptr::null_mut();
    // SAFETY: the pointers are valid for the call; the C side allocates `out` and `error` with malloc.
    let status =
        unsafe { wcs_dxil_constant_sizes(dxil.as_ptr(), dxil.len(), &mut out, &mut n, &mut error) };
    if status != 0 {
        return Err(ConvertError::Rejected(reason(error)));
    }
    Ok(words(out, n))
}

/// The largest constant buffer, in bytes, which dxil-spirv declares for every dynamically indexed one.
const MAX_CONSTANT_BYTES: u32 = 64 * 1024;

/// dxil-spirv declares every dynamically indexed constant buffer 64 KiB long, so WebGPU would require
/// 64 KiB bound. Such a buffer is declared with the size the bytecode gives it, which a program binds
/// at least. Runs before `bindings`, while a binding still indexes `resources`.
fn declared_sizes(
    dxil: &[u8],
    resources: &Resources,
    module: &mut Module,
) -> Result<(), ConvertError> {
    let mut sizes = None;
    let mut resized = false;
    for (_, var) in module.global_variables.iter_mut() {
        let (AddressSpace::Uniform, Some(binding)) = (var.space, var.binding) else {
            continue;
        };
        let TypeInner::Struct { ref members, .. } = module.types[var.ty].inner else {
            continue;
        };
        let [member] = &members[..] else {
            continue;
        };
        let TypeInner::Array {
            base,
            size: naga::ArraySize::Constant(length),
            stride,
        } = module.types[member.ty].inner
        else {
            continue;
        };
        if length.get().checked_mul(stride) != Some(MAX_CONSTANT_BYTES) {
            continue;
        }
        let sizes = match &sizes {
            Some(s) => s,
            None => sizes.insert(constant_sizes(dxil)?),
        };
        let Some(&[CONSTANTS, space, register]) = resources.get(binding.binding as usize) else {
            continue;
        };
        let declared = sizes
            .as_chunks::<3>()
            .0
            .iter()
            .find(|d| (d[0], d[1]) == (space, register))
            .map(|d| d[2].div_ceil(stride));
        let Some(declared) = declared.and_then(core::num::NonZeroU32::new) else {
            continue;
        };
        if declared >= length {
            continue;
        }
        let member = member.clone();
        let name = module.types[var.ty].name.clone();
        let array = module.types.insert(
            Type {
                name: None,
                inner: TypeInner::Array {
                    base,
                    size: naga::ArraySize::Constant(declared),
                    stride,
                },
            },
            Span::UNDEFINED,
        );
        var.ty = module.types.insert(
            Type {
                name,
                inner: TypeInner::Struct {
                    members: alloc::vec![StructMember {
                        ty: array,
                        ..member
                    }],
                    span: declared.get() * stride,
                },
            },
            Span::UNDEFINED,
        );
        resized = true;
    }
    if resized {
        // Drops the 64 KiB types, which writers would still declare.
        naga::compact::compact(module, naga::compact::KeepUnused::No);
    }
    Ok(())
}

/// Converts DXIL into a naga module and its reflection.
pub(crate) fn lower(
    dxil: &[u8],
    variant: &VariantKey,
) -> Result<(Module, Reflection, ShaderStage), ConvertError> {
    let (mut spirv, resources) = to_spirv(dxil, variant.samples)?;
    let declared = images::prepare(&mut spirv);
    let options = naga::front::spv::Options {
        adjust_coordinate_space: false,
        strict_capabilities: false,
        block_ctx_dump_prefix: None,
    };
    let mut module = naga::front::spv::Frontend::new(spirv.iter().copied(), &options)
        .parse()
        .map_err(|e| ConvertError::Reader(format!("{e}")))?;
    nonfinite::calls(&mut module)?;
    let mut atomics = images::texel_atomics(&mut module)?;
    // WebGPU's storage buffers are read-only or read-write; one the program only writes is the latter.
    for (_, v) in module.global_variables.iter_mut() {
        if let AddressSpace::Storage { access } = &mut v.space
            && *access == naga::StorageAccess::STORE
        {
            *access |= naga::StorageAccess::LOAD;
        }
    }
    unconverted(&module)?;
    if !module.overrides.is_empty() {
        // dxil-spirv's only specialisation constant here: the targets' sample count.
        return Err(ConvertError::Unsupported("SampleCount"));
    }
    if let Some(moved) = direct3d_float_to_int(&mut module)? {
        images::remap(&mut atomics, &moved);
    }
    declared_sizes(dxil, &resources, &mut module)?;
    let images = images::globals(&module, &declared);
    crate::lower::derivatives_anywhere(&mut module);
    let ep = module
        .entry_points
        .first()
        .ok_or(ConvertError::Malformed("no entry point".into()))?;
    let (stage, source_stage, set) = match ep.stage {
        naga::ShaderStage::Vertex => (ShaderStage::Vertex, SourceStage::Vertex, 0),
        naga::ShaderStage::Fragment => (ShaderStage::Fragment, SourceStage::Pixel, 1),
        naga::ShaderStage::Compute => (ShaderStage::Compute, SourceStage::Compute, 0),
        _ => return Err(ConvertError::Unsupported("Stage")),
    };
    let extras = match stage {
        ShaderStage::Compute => StageExtras::Compute {
            workgroup_size: ep.workgroup_size,
        },
        ShaderStage::Fragment => pixel_extras(&module, !atomics.is_empty()),
        _ => StageExtras::None,
    };
    module.entry_points[0].name = "main".into();
    module.entry_points[0].early_depth_test = None;
    if stage == ShaderStage::Vertex {
        output_interpolation(&mut module, &variant.output_interpolation);
    }
    let mut reflection = Reflection {
        stage: source_stage,
        inputs: Vec::new(),
        outputs: Vec::new(),
        bindings: Vec::new(),
        helpers: Vec::new(),
        helper_size: 0,
        sampled_pairs: Vec::new(),
        extras,
    };
    let ep = &module.entry_points[0].function;
    let inputs = signature(dxil, b"ISG1");
    for a in &ep.arguments {
        let into = &mut reflection.inputs;
        stage_values(&module, a.ty, a.binding.as_ref(), false, &inputs, into);
    }
    if let Some(r) = &ep.result {
        let outputs = signature(dxil, b"OSG1");
        let pixel = stage == ShaderStage::Fragment;
        let into = &mut reflection.outputs;
        stage_values(&module, r.ty, r.binding.as_ref(), pixel, &outputs, into);
    }
    let slots = bindings(&mut module, &resources, set, &mut reflection)?;
    reflection.sampled_pairs = sampled_pairs(&module, &slots)?;
    draw_bases(&mut module, variant, &mut reflection)?;
    images::finish(
        &mut module,
        variant,
        &images,
        &mut atomics,
        stage,
        &mut reflection,
    )?;
    Ok((module, reflection, stage))
}

/// What a pixel program asks of the pipeline beyond its code: early tests (reported, since WGSL cannot
/// force them), writes to storage (`texel_atomics` where it runs atomics on typed views), discards,
/// and running once per sample.
fn pixel_extras(module: &Module, texel_atomics: bool) -> StageExtras {
    let ep = &module.entry_points[0];
    let functions =
        || core::iter::once(&ep.function).chain(module.functions.iter().map(|(_, f)| f));
    let storage = |f: &Function, pointer: Handle<Expression>| {
        let mut e = pointer;
        loop {
            match f.expressions[e] {
                Expression::Access { base, .. } | Expression::AccessIndex { base, .. } => e = base,
                Expression::GlobalVariable(g) => {
                    return matches!(
                        module.global_variables[g].space,
                        AddressSpace::Storage { .. }
                    );
                }
                _ => return false,
            }
        }
    };
    let mut side_effects = texel_atomics;
    let mut discards = false;
    for f in functions() {
        let mut stack = vec![&f.body];
        while let Some(block) = stack.pop() {
            for s in block.iter() {
                match s {
                    Statement::Kill => discards = true,
                    Statement::ImageStore { .. } | Statement::Atomic { .. } => side_effects = true,
                    Statement::Store { pointer, .. } if storage(f, *pointer) => side_effects = true,
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
    let per_sample = ep.function.arguments.iter().any(|a| {
        let sampled = |b: &Binding| {
            matches!(
                b,
                Binding::BuiltIn(BuiltIn::SampleIndex)
                    | Binding::Location {
                        sampling: Some(naga::Sampling::Sample),
                        ..
                    }
            )
        };
        a.binding.as_ref().is_some_and(sampled)
            || matches!(&module.types[a.ty].inner, TypeInner::Struct { members, .. }
                if members.iter().any(|m| m.binding.as_ref().is_some_and(sampled)))
    });
    StageExtras::Pixel {
        // `Allow` is conservative depth, only a hint.
        early_tests: ep.early_depth_test == Some(naga::EarlyDepthTest::Force),
        side_effects,
        discards,
        per_sample,
    }
}

/// Refuses by name what the reader's module has that the common form cannot take as it stands.
fn unconverted(module: &Module) -> Result<(), ConvertError> {
    let ep = module
        .entry_points
        .first()
        .ok_or(ConvertError::Malformed("no entry point".into()))?;
    if module.global_variables.iter().any(|(_, v)| {
        matches!(
            module.types[v.ty].inner,
            TypeInner::Image {
                dim: naga::ImageDimension::D1,
                arrayed: true,
                ..
            }
        )
    }) {
        return Err(ConvertError::Unsupported("Texture1DArray"));
    }
    // WGSL has no multisampled array textures (the DXBC route refuses them by the same name).
    if module.global_variables.iter().any(|(_, v)| {
        matches!(
            module.types[v.ty].inner,
            TypeInner::Image {
                arrayed: true,
                class: naga::ImageClass::Sampled { multi: true, .. }
                    | naga::ImageClass::Depth { multi: true },
                ..
            }
        )
    }) {
        return Err(ConvertError::Unsupported("MultisampledArrayTexture"));
    }
    // Every built-in the entry point reads or writes, with its type, and every location it reads
    // (`false`) or writes (`true`).
    let mut builtins = Vec::new();
    let mut locations = Vec::new();
    let mut add = |ty: Handle<Type>, binding: Option<&Binding>, output: bool| {
        let mut one = |ty, binding: Option<&Binding>| match binding {
            Some(Binding::BuiltIn(b)) => builtins.push((*b, ty)),
            Some(Binding::Location { location, .. }) => locations.push((output, *location)),
            None => {}
        };
        match (binding, &module.types[ty].inner) {
            (None, TypeInner::Struct { members, .. }) => {
                for m in members {
                    one(m.ty, m.binding.as_ref());
                }
            }
            _ => one(ty, binding),
        }
    };
    for a in &ep.function.arguments {
        add(a.ty, a.binding.as_ref(), false);
    }
    let result = ep.function.result.as_ref();
    if let Some(r) = result {
        add(r.ty, r.binding.as_ref(), true);
    }
    // dxc packs small values into one register's components; WGSL gives each value a location.
    locations.sort_unstable();
    if locations.windows(2).any(|w| w[0] == w[1]) {
        return Err(ConvertError::Unsupported("PackedStageValues"));
    }
    if builtins.iter().any(|(b, _)| *b == BuiltIn::PrimitiveIndex) {
        return Err(ConvertError::Unsupported("PrimitiveId"));
    }
    // dxil-spirv reads a component of a constant buffer picked at run time through a second view of
    // the buffer as scalars, 4 bytes apart; WebGPU's uniform arrays step by 16 bytes, and WGSL takes
    // one variable per binding.
    let scalar_view = |v: &GlobalVariable| {
        match module.types[v.ty].inner {
        TypeInner::Struct { ref members, .. } => members.iter().any(|m| {
            matches!(module.types[m.ty].inner, TypeInner::Array { stride, .. } if stride % 16 != 0)
        }),
        _ => false,
    }
    };
    if module
        .global_variables
        .iter()
        .any(|(_, v)| v.space == AddressSpace::Uniform && scalar_view(v))
    {
        return Err(ConvertError::Unsupported("ConstantComponentIndexing"));
    }
    // naga's IR holds no infinite or NaN literal; the reader makes them from SPIR-V's constants.
    let non_finite =
        |e: &Expression| matches!(*e, Expression::Literal(Literal::F32(x)) if !x.is_finite());
    let functions = core::iter::once(&ep.function).chain(module.functions.iter().map(|(_, f)| f));
    if module.global_expressions.iter().any(|(_, e)| non_finite(e))
        || functions
            .flat_map(|f| f.expressions.iter())
            .any(|(_, e)| non_finite(e))
    {
        return Err(ConvertError::Unsupported("NonFiniteConstant"));
    }
    if builtins.iter().any(|(b, ty)| {
        *b == BuiltIn::SampleMask && matches!(module.types[*ty].inner, TypeInner::Array { .. })
    }) {
        return Err(ConvertError::Reader(
            "the sample mask stays an array".into(),
        ));
    }
    Ok(())
}

/// Calls `f` on `block` and on every block nested in it.
fn each_block(block: &mut naga::Block, f: &mut impl FnMut(&mut naga::Block)) {
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

/// An f32 to integer conversion: the integer's kind and the vector size.
type FloatToInt = (ScalarKind, Option<naga::VectorSize>);

/// A conversion's expression, its kind and its operand.
type Conversion = (Handle<Expression>, FloatToInt, Handle<Expression>);

/// Each f32 to i32 or u32 conversion in `f`.
fn float_to_ints(module: &Module, f: &Function) -> Result<Vec<Conversion>, ConvertError> {
    let mut typifier = naga::front::Typifier::new();
    let ctx = naga::proc::ResolveContext::with_locals(module, &f.local_variables, &f.arguments);
    let mut found = Vec::new();
    for (h, e) in f.expressions.iter() {
        let Expression::As {
            expr,
            kind: kind @ (ScalarKind::Sint | ScalarKind::Uint),
            convert: Some(width),
        } = *e
        else {
            continue;
        };
        typifier
            .grow(expr, &f.expressions, &ctx)
            .map_err(|e| ConvertError::Malformed(format!("{e}")))?;
        let (size, scalar) = match *typifier.get(expr, &module.types) {
            TypeInner::Scalar(s) => (None, s),
            TypeInner::Vector { size, scalar } => (Some(size), scalar),
            _ => continue,
        };
        if scalar.kind != ScalarKind::Float {
            continue;
        }
        if (scalar.width, width) != (4, 4) {
            return Err(ConvertError::Unsupported("FloatToIntWidth"));
        }
        found.push((h, (kind, size), expr));
    }
    Ok(found)
}

/// A function giving Direct3D's f32 to integer conversion: 0 for NaN, the integer's limits at and
/// past them.
fn float_to_int_function(module: &mut Module, (kind, size): FloatToInt) -> Function {
    let mut ty = |kind| {
        let scalar = Scalar { kind, width: 4 };
        let inner = match size {
            None => TypeInner::Scalar(scalar),
            Some(size) => TypeInner::Vector { size, scalar },
        };
        module
            .types
            .insert(Type { name: None, inner }, Span::UNDEFINED)
    };
    let float = ty(ScalarKind::Float);
    let int = ty(kind);
    let mut f = Function {
        name: Some(String::from(if kind == ScalarKind::Sint {
            "d3d_ftoi"
        } else {
            "d3d_ftou"
        })),
        arguments: alloc::vec![FunctionArgument {
            name: Some("x".into()),
            ty: float,
            binding: None,
        }],
        result: Some(naga::FunctionResult {
            ty: int,
            binding: None,
        }),
        ..Default::default()
    };
    let e = &mut f.expressions;
    let x = e.append(Expression::FunctionArgument(0), Span::UNDEFINED);
    let (above, below, max, min, zero) = if kind == ScalarKind::Sint {
        let (max, min) = (Literal::I32(i32::MAX), Literal::I32(i32::MIN));
        (2_147_483_648.0, -2_147_483_648.0, max, min, Literal::I32(0))
    } else {
        let (max, zero) = (Literal::U32(u32::MAX), Literal::U32(0));
        (4_294_967_296.0, 0.0, max, zero, zero)
    };
    let literals = [
        Literal::F32(above),
        Literal::F32(below),
        max,
        min,
        zero,
        Literal::U32(0x7fff_ffff),
        Literal::U32(0x7f80_0000),
    ]
    .map(|l| e.append(Expression::Literal(l), Span::UNDEFINED));
    let start = e.len();
    let [above, below, max, min, zero, magnitude, infinity] = literals.map(|value| match size {
        None => value,
        Some(size) => e.append(Expression::Splat { size, value }, Span::UNDEFINED),
    });
    let mut add = |expr| e.append(expr, Span::UNDEFINED);
    let converted = add(Expression::As {
        expr: x,
        kind,
        convert: Some(4),
    });
    let bits = add(Expression::As {
        expr: x,
        kind: ScalarKind::Uint,
        convert: None,
    });
    let bits = add(Expression::Binary {
        op: naga::BinaryOperator::And,
        left: bits,
        right: magnitude,
    });
    let nan = add(Expression::Binary {
        op: naga::BinaryOperator::Greater,
        left: bits,
        right: infinity,
    });
    let high = add(Expression::Binary {
        op: naga::BinaryOperator::GreaterEqual,
        left: x,
        right: above,
    });
    let low = add(Expression::Binary {
        op: naga::BinaryOperator::LessEqual,
        left: x,
        right: below,
    });
    let value = add(Expression::Select {
        condition: high,
        accept: max,
        reject: converted,
    });
    let value = add(Expression::Select {
        condition: low,
        accept: min,
        reject: value,
    });
    let value = add(Expression::Select {
        condition: nan,
        accept: zero,
        reject: value,
    });
    let range = Range::from_index_range(start as u32..f.expressions.len() as u32, &f.expressions);
    f.body.push(Statement::Emit(range), Span::UNDEFINED);
    f.body
        .push(Statement::Return { value: Some(value) }, Span::UNDEFINED);
    f
}

/// naga's float to integer conversion is not Direct3D's, which gives 0 for NaN and the integer's
/// limits at and past them (WGSL and naga's HLSL stop at the largest float below the limit, and
/// differ on NaN). dxbc-spirv builds Direct3D's for DXBC; here every f32 to i32 or u32 conversion of
/// the DXIL module becomes a call of a function that gives it. Gives where the module's functions
/// went, if they moved.
fn direct3d_float_to_int(
    module: &mut Module,
) -> Result<Option<Vec<Handle<Function>>>, ConvertError> {
    // (function index, or entry point index + functions, and its conversions)
    let mut found = Vec::new();
    for (i, (_, f)) in module.functions.iter().enumerate() {
        found.push((i, float_to_ints(module, f)?));
    }
    let n = module.functions.len();
    for (i, ep) in module.entry_points.iter().enumerate() {
        found.push((n + i, float_to_ints(module, &ep.function)?));
    }
    found.retain(|(_, c)| !c.is_empty());
    if found.is_empty() {
        return Ok(None);
    }
    let (helpers, moved) = functions_first(module, |module| {
        let mut helpers: Vec<(FloatToInt, Handle<Function>)> = Vec::new();
        for (_, conversions) in &found {
            for &(_, key, _) in conversions {
                if !helpers.iter().any(|(k, _)| *k == key) {
                    let f = float_to_int_function(module, key);
                    helpers.push((key, module.functions.append(f, Span::UNDEFINED)));
                }
            }
        }
        helpers
    });
    for (i, conversions) in found {
        let f = match moved.get(i) {
            Some(&h) => &mut module.functions[h],
            None => &mut module.entry_points[i - n].function,
        };
        let calls: Vec<Placed> = conversions
            .iter()
            .map(|&(h, key, arg)| {
                let helper = helpers.iter().find(|(k, _)| *k == key).map(|(_, h)| *h);
                Placed {
                    result: h,
                    function: helper.expect("made above"),
                    arguments: alloc::vec![arg],
                    pre: None,
                }
            })
            .collect();
        place_calls(f, &calls);
    }
    Ok(Some(moved))
}

/// Runs `make`, which appends functions, so that they come before every function the module had
/// (callees precede their callers in naga's arena). Returns what `make` gives and where each of the
/// module's functions went.
fn functions_first<T>(
    module: &mut Module,
    make: impl FnOnce(&mut Module) -> T,
) -> (T, Vec<Handle<Function>>) {
    let old: Vec<_> = module.functions.drain().collect();
    let made = make(module);
    let moved: Vec<Handle<Function>> = old
        .into_iter()
        .map(|(_, f, span)| module.functions.append(f, span))
        .collect();
    let remap = |f: &mut Function| {
        each_block(&mut f.body, &mut |block| {
            for s in block.iter_mut() {
                if let Statement::Call { function, .. } = s {
                    *function = moved[function.index()];
                }
            }
        });
        for (_, e) in f.expressions.iter_mut() {
            if let Expression::CallResult(function) = e {
                *function = moved[function.index()];
            }
        }
    };
    for &h in &moved {
        remap(&mut module.functions[h]);
    }
    for ep in module.entry_points.iter_mut() {
        remap(&mut ep.function);
    }
    (made, moved)
}

/// A call that takes the place of expression `result`, its arguments made by the expressions in
/// `pre` (appended for it, not yet emitted), which are emitted just before it.
struct Placed {
    result: Handle<Expression>,
    function: Handle<Function>,
    arguments: Vec<Handle<Expression>>,
    pre: Option<Range<Expression>>,
}

/// Makes each of `calls` a call statement where its result was emitted, and its result the call's.
fn place_calls(f: &mut Function, calls: &[Placed]) {
    if calls.is_empty() {
        return;
    }
    each_block(&mut f.body, &mut |block| {
        let placed = |s: &Statement| matches!(s, Statement::Emit(r) if r.clone().any(|h| calls.iter().any(|c| c.result == h)));
        if !block.iter().any(placed) {
            return;
        }
        for (s, span) in core::mem::take(block).span_into_iter() {
            let Statement::Emit(range) = s else {
                block.push(s, span);
                continue;
            };
            // The emitted run so far, split at each call.
            let mut run: Option<(Handle<Expression>, Handle<Expression>)> = None;
            for h in range {
                let Some(c) = calls.iter().find(|c| c.result == h) else {
                    run = Some((run.map_or(h, |r| r.0), h));
                    continue;
                };
                if let Some((first, last)) = run.take() {
                    block.push(Statement::Emit(Range::new_from_bounds(first, last)), span);
                }
                if let Some(pre) = &c.pre {
                    block.push(Statement::Emit(pre.clone()), span);
                }
                block.push(
                    Statement::Call {
                        function: c.function,
                        arguments: c.arguments.clone(),
                        result: Some(h),
                    },
                    span,
                );
            }
            if let Some((first, last)) = run {
                block.push(Statement::Emit(Range::new_from_bounds(first, last)), span);
            }
        }
    });
    for c in calls {
        *f.expressions.get_mut(c.result) = Expression::CallResult(c.function);
    }
}

/// Writes the vertex program's float outputs with the interpolation the variant asks for their locations.
fn output_interpolation(module: &mut Module, asked: &[(u32, Interpolation)]) {
    let types = &mut module.types;
    let ep = &mut module.entry_points[0].function;
    let Some(result) = ep.result.as_mut() else {
        return;
    };
    let set = |binding: &mut Binding, ty: &TypeInner| {
        if let Binding::Location {
            location,
            interpolation,
            sampling,
            ..
        } = binding
            && ty.scalar_kind() == Some(ScalarKind::Float)
        {
            (*interpolation, *sampling) =
                crate::lower::naga_interpolation(crate::lower::output_modes(asked, *location));
        }
    };
    if let Some(binding) = result.binding.as_mut() {
        set(binding, &types[result.ty].inner);
        return;
    }
    let old = result.ty;
    let TypeInner::Struct { mut members, span } = types[old].inner.clone() else {
        return;
    };
    for m in &mut members {
        if let Some(binding) = m.binding.as_mut() {
            set(binding, &types[m.ty].inner);
        }
    }
    let name = types[old].name.clone();
    let new = types.insert(
        Type {
            name,
            inner: TypeInner::Struct { members, span },
        },
        Span::UNDEFINED,
    );
    result.ty = new;
    for (_, e) in ep.expressions.iter_mut() {
        if let Expression::Compose { ty, .. } | Expression::ZeroValue(ty) = e
            && *ty == old
        {
            *ty = new;
        }
    }
    for (_, v) in ep.local_variables.iter_mut() {
        if v.ty == old {
            v.ty = new;
        }
    }
}

/// dxil-spirv reads `SV_VertexID` and `SV_InstanceID` as `VertexIndex - BaseVertex` and `InstanceIndex -
/// BaseInstance`; WGSL has no base built-ins. The entry point becomes a function called by a new entry
/// point that passes the bases from the variant's source: zero (the front promises bases of zero) or
/// the vertex stage's helper constants (which the variant pass reads from an indirect draw's
/// arguments for [`DrawBaseSource::Arguments`]).
fn draw_bases(
    module: &mut Module,
    variant: &VariantKey,
    reflection: &mut Reflection,
) -> Result<(), ConvertError> {
    let is_base = |a: &FunctionArgument| {
        matches!(
            a.binding,
            Some(Binding::BuiltIn(
                BuiltIn::BaseVertex | BuiltIn::BaseInstance
            ))
        )
    };
    if !module.entry_points[0]
        .function
        .arguments
        .iter()
        .any(is_base)
    {
        return Ok(());
    }
    let helpers = match variant.draw_base {
        DrawBaseSource::None => None,
        DrawBaseSource::Helpers | DrawBaseSource::Arguments => {
            Some(helper_block(module, reflection))
        }
    };
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
        result,
        ..Default::default()
    };
    let mut passed = Vec::new();
    for a in &arguments {
        if !is_base(a) {
            let n = f.arguments.len() as u32;
            f.arguments.push(a.clone());
            passed.push(
                f.expressions
                    .append(Expression::FunctionArgument(n), Span::UNDEFINED),
            );
            continue;
        }
        let instance = a.binding == Some(Binding::BuiltIn(BuiltIn::BaseInstance));
        let sint =
            matches!(module.types[a.ty].inner, TypeInner::Scalar(s) if s.kind == ScalarKind::Sint);
        let Some(g) = helpers else {
            let zero = if sint {
                Literal::I32(0)
            } else {
                Literal::U32(0)
            };
            passed.push(
                f.expressions
                    .append(Expression::Literal(zero), Span::UNDEFINED),
            );
            continue;
        };
        let base = f
            .expressions
            .append(Expression::GlobalVariable(g), Span::UNDEFINED);
        let emit = |f: &mut Function, e| {
            let h = f.expressions.append(e, Span::UNDEFINED);
            f.body.push(
                Statement::Emit(Range::new_from_bounds(h, h)),
                Span::UNDEFINED,
            );
            h
        };
        // base_vertex is an i32 at index 0, first_instance a u32 at index 1.
        let pointer = emit(
            &mut f,
            Expression::AccessIndex {
                base,
                index: u32::from(instance),
            },
        );
        let mut value = emit(&mut f, Expression::Load { pointer });
        if sint == instance {
            value = emit(
                &mut f,
                Expression::As {
                    expr: value,
                    kind: if sint {
                        ScalarKind::Sint
                    } else {
                        ScalarKind::Uint
                    },
                    convert: None,
                },
            );
        }
        passed.push(value);
    }
    let returned = f.result.is_some().then(|| {
        f.expressions
            .append(Expression::CallResult(callee), Span::UNDEFINED)
    });
    f.body.push(
        Statement::Call {
            function: callee,
            arguments: passed,
            result: returned,
        },
        Span::UNDEFINED,
    );
    f.body
        .push(Statement::Return { value: returned }, Span::UNDEFINED);
    module.entry_points[0].function = f;
    Ok(())
}

/// The vertex stage's helper constants: the draw's bases at offset 0, laid out as the DXBC route's.
fn helper_block(module: &mut Module, reflection: &mut Reflection) -> Handle<GlobalVariable> {
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
    let member = |name: &str, ty, offset| StructMember {
        name: Some(name.into()),
        ty,
        binding: None,
        offset,
    };
    let ty = module.types.insert(
        Type {
            name: Some("Helpers".into()),
            inner: TypeInner::Struct {
                members: alloc::vec![
                    member("base_vertex", i32_ty, 0),
                    member("first_instance", u32_ty, 4),
                    member("draw", u32_ty, 8),
                ],
                span: 16,
            },
        },
        Span::UNDEFINED,
    );
    reflection.helpers.push(HelperSlot {
        value: HelperValue::DrawBase,
        offset: 0,
    });
    reflection.helper_size = 16;
    module.global_variables.append(
        GlobalVariable {
            name: Some("helpers".into()),
            space: AddressSpace::Uniform,
            binding: Some(naga::ResourceBinding {
                group: HELPER_SET,
                binding: helper_binding(ShaderStage::Vertex),
            }),
            ty,
            init: None,
            memory_decorations: Default::default(),
        },
        Span::UNDEFINED,
    )
}

/// An element of a DXIL container's input or output signature.
struct Element {
    semantic: String,
    index: u32,
    /// Direct3D's system value name (`D3D_NAME`), 0 for none.
    system: u32,
    register: u32,
    mask: u8,
}

/// The elements of the container's signature chunk `tag` (`ISG1` or `OSG1`: 32-byte elements).
fn signature(container: &[u8], tag: &[u8; 4]) -> Vec<Element> {
    let word = |b: &[u8], at: usize| {
        b.get(at..at + 4)
            .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
    };
    let mut out = Vec::new();
    let Some(count) = word(container, 28) else {
        return out;
    };
    for i in 0..count as usize {
        let Some(at) = word(container, 32 + 4 * i).map(|a| a as usize) else {
            break;
        };
        if container.get(at..at + 4) != Some(&tag[..]) {
            continue;
        }
        let size = word(container, at + 4).unwrap_or(0) as usize;
        let Some(chunk) = container.get(at + 8..at + 8 + size) else {
            break;
        };
        let (n, first) = (word(chunk, 0).unwrap_or(0), word(chunk, 4).unwrap_or(0));
        for e in 0..n as usize {
            let at = first as usize + 32 * e;
            let (Some(name), Some(index), Some(system), Some(register), Some(&mask)) = (
                word(chunk, at + 4),
                word(chunk, at + 8),
                word(chunk, at + 12),
                word(chunk, at + 20),
                chunk.get(at + 24),
            ) else {
                break;
            };
            let semantic = chunk
                .get(name as usize..)
                .and_then(|s| s.split(|&c| c == 0).next())
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .unwrap_or_default();
            out.push(Element {
                semantic,
                index,
                system,
                register,
                mask,
            });
        }
    }
    out
}

/// The reflection's inputs or outputs: the entry point's values, named from the signature.
fn stage_values(
    module: &Module,
    ty: Handle<Type>,
    binding: Option<&Binding>,
    pixel_output: bool,
    signature: &[Element],
    out: &mut Vec<StageValue>,
) {
    if let TypeInner::Struct { ref members, .. } = module.types[ty].inner {
        for m in members {
            stage_values(
                module,
                m.ty,
                m.binding.as_ref(),
                pixel_output,
                signature,
                out,
            );
        }
        return;
    }
    let Some(binding) = binding else {
        return;
    };
    let (kind, size) = match module.types[ty].inner {
        TypeInner::Scalar(s) => (s.kind, 1),
        TypeInner::Vector { size, scalar } => (scalar.kind, size as u32),
        TypeInner::Array { .. } => (ScalarKind::Float, 1),
        _ => return,
    };
    let scalar = match kind {
        ScalarKind::Float => ValueKind::Float,
        ScalarKind::Sint => ValueKind::Sint,
        _ => ValueKind::Uint,
    };
    let (location, interpolation, system, element) = match *binding {
        Binding::Location {
            location,
            interpolation,
            sampling,
            ..
        } => {
            use naga::{Interpolation as I, Sampling as S};
            let interpolation = match (interpolation, sampling) {
                _ if scalar != ValueKind::Float => Interpolation::Flat,
                (Some(I::Flat), _) => Interpolation::Flat,
                (Some(I::Linear), Some(S::Centroid)) => Interpolation::LinearCentroid,
                (Some(I::Linear), Some(S::Sample)) => Interpolation::LinearSample,
                (Some(I::Linear), _) => Interpolation::Linear,
                (_, Some(S::Centroid)) => Interpolation::PerspectiveCentroid,
                (_, Some(S::Sample)) => Interpolation::PerspectiveSample,
                _ => Interpolation::Perspective,
            };
            let system = pixel_output.then_some(SystemValue::Target(location as u8));
            let element = signature
                .iter()
                .find(|e| e.register == location && (e.system == 0 || e.system == 64));
            (location, interpolation, system, element)
        }
        Binding::BuiltIn(b) => {
            let (system, code) = match b {
                BuiltIn::Position { .. } => (SystemValue::Position, 1),
                BuiltIn::VertexIndex => (SystemValue::VertexId, 6),
                BuiltIn::InstanceIndex => (SystemValue::InstanceId, 8),
                BuiltIn::FrontFacing => (SystemValue::FrontFacing, 9),
                BuiltIn::SampleIndex => (SystemValue::SampleIndex, 10),
                BuiltIn::SampleMask => (SystemValue::Coverage, 66),
                BuiltIn::FragDepth => (SystemValue::Depth, 65),
                BuiltIn::ClipDistances => (SystemValue::ClipDistance, 2),
                _ => return,
            };
            let element = signature.iter().find(|e| e.system == code);
            let interpolation = if scalar == ValueKind::Float {
                Interpolation::Perspective
            } else {
                Interpolation::Flat
            };
            (0, interpolation, Some(system), element)
        }
    };
    out.push(StageValue {
        semantic: element.map(|e| e.semantic.clone()).unwrap_or_default(),
        semantic_index: element.map_or(0, |e| e.index),
        register: element.map_or(location, |e| e.register),
        mask: element.map_or(((1u32 << size) - 1) as u8, |e| e.mask),
        scalar,
        interpolation,
        system,
        location,
        component: 0,
    });
}

/// The (texture slot, sampler slot) pairs the program samples with, in the order its code first uses
/// them.
fn sampled_pairs(
    module: &Module,
    registers: &[(Handle<GlobalVariable>, u32)],
) -> Result<Vec<(u32, u32)>, ConvertError> {
    let register = |f: &Function, e: Handle<Expression>| match f.expressions[e] {
        Expression::GlobalVariable(g) => registers.iter().find(|(h, _)| *h == g).map(|r| r.1),
        _ => None,
    };
    let functions = module.functions.iter().map(|(_, f)| f);
    let mut pairs = Vec::new();
    for f in core::iter::once(&module.entry_points[0].function).chain(functions) {
        for (_, e) in f.expressions.iter() {
            let Expression::ImageSample { image, sampler, .. } = *e else {
                continue;
            };
            let (Some(texture), Some(sampler)) = (register(f, image), register(f, sampler)) else {
                return Err(ConvertError::Unsupported("SampledThroughArgument"));
            };
            if !pairs.contains(&(texture, sampler)) {
                pairs.push((texture, sampler));
            }
        }
    }
    Ok(pairs)
}

/// Numbers every resource by Direct3D 12's rule (6.4): within the stage's set, each kind's slots are
/// the program's own resources of that kind in (space, register) order, from the kind's first
/// binding, so register spaces and registers of any size fit while the program has no more resources
/// of a kind than its range holds. Kinds follow Direct3D's register classes as on the DXBC route
/// (shader resource views, buffers among them, are textures; unordered access views are storage); a
/// counter takes its view's slot. A slot (`binding - kind.first_binding()`) is what the variant's
/// per-slot fields and the sampled pairs name. Returns each resource's slot.
fn bindings(
    module: &mut Module,
    resources: &Resources,
    set: u32,
    reflection: &mut Reflection,
) -> Result<Vec<(Handle<GlobalVariable>, u32)>, ConvertError> {
    let mut declared = Vec::new();
    for (handle, var) in module.global_variables.iter() {
        let Some(binding) = var.binding else {
            continue;
        };
        if let TypeInner::BindingArray { .. } = module.types[var.ty].inner {
            return Err(ConvertError::Unsupported("ResourceArray"));
        }
        let &[class, space, register] = resources
            .get(binding.binding as usize)
            .ok_or_else(|| ConvertError::Malformed("a binding past the resource list".into()))?;
        let kind = match binding.group {
            CONSTANTS => SlotKind::Uniform,
            VIEW => SlotKind::Texture,
            STORAGE => SlotKind::Storage,
            SAMPLER => SlotKind::Sampler,
            COUNTER => SlotKind::Counter,
            _ => return Err(ConvertError::Malformed("a resource of no class".into())),
        };
        if binding.group != class && !(binding.group == COUNTER && class == STORAGE) {
            return Err(ConvertError::Malformed(
                "a resource of another class".into(),
            ));
        }
        declared.push((handle, kind, space, register));
    }
    // Counters are numbered by their views, so they are ranked among the storage slots.
    let rank = |kind: SlotKind| match kind {
        SlotKind::Counter => SlotKind::Storage as u32,
        k => k as u32,
    };
    let mut order: Vec<(u32, u32, u32)> = declared
        .iter()
        .map(|&(_, kind, space, register)| (rank(kind), space, register))
        .collect();
    order.sort_unstable();
    order.dedup();
    let mut slots = Vec::new();
    for (handle, kind, space, register) in declared {
        let key = (rank(kind), space, register);
        let first = order.partition_point(|o| o.0 < key.0);
        let slot = (order.partition_point(|o| *o < key) - first) as u32;
        let number = kind
            .binding(slot)
            .ok_or(ConvertError::Unsupported("BindingsBeyondLimit"))?;
        module.global_variables[handle].binding = Some(naga::ResourceBinding {
            group: set,
            binding: number,
        });
        reflection.bindings.push(ReflectedBinding {
            kind,
            space,
            register,
            count: 1,
            set,
            binding: number,
            added: None,
        });
        slots.push((handle, slot));
    }
    Ok(slots)
}
