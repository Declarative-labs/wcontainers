//! DXIL through dxil-spirv (C++, built by `build.rs`) to SPIR-V, read by naga's SPIR-V reader, then
//! given the binding rule of plan section 6.4 and stripped of what WGSL cannot express.

use crate::ConvertError;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::ffi::{c_char, c_int, c_void};
use naga::{
    AddressSpace, Binding, BuiltIn, Expression, Function, FunctionArgument, GlobalVariable, Handle,
    ImageClass, Literal, Module, Range, Scalar, ScalarKind, Span, Statement, StructMember, Type,
    TypeInner,
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
        out: *mut *mut u32,
        out_words: *mut usize,
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

/// DXIL to SPIR-V words.
fn to_spirv(dxil: &[u8]) -> Result<Vec<u32>, ConvertError> {
    let mut out = core::ptr::null_mut();
    let mut words = 0;
    let mut error = core::ptr::null_mut();
    // SAFETY: the pointers are valid for the call; the C side allocates `out` and `error` with malloc.
    let status =
        unsafe { wcs_dxil_to_spirv(dxil.as_ptr(), dxil.len(), &mut out, &mut words, &mut error) };
    if status != 0 {
        return Err(ConvertError::Rejected(reason(error)));
    }
    Ok(self::words(out, words))
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
/// at least. Runs before `bindings`, while a binding is still (space, register).
fn declared_sizes(dxil: &[u8], module: &mut Module) -> Result<(), ConvertError> {
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
        let declared = sizes
            .as_chunks::<3>()
            .0
            .iter()
            .find(|d| (d[0], d[1]) == (binding.group, binding.binding))
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
    let spirv = to_spirv(dxil)?;
    let options = naga::front::spv::Options {
        adjust_coordinate_space: false,
        strict_capabilities: false,
        block_ctx_dump_prefix: None,
    };
    let mut module = naga::front::spv::Frontend::new(spirv.iter().copied(), &options)
        .parse()
        .map_err(|e| ConvertError::Reader(format!("{e}")))?;
    unconverted(&module)?;
    declared_sizes(dxil, &mut module)?;
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
        ShaderStage::Fragment => pixel_extras(&module),
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
    let registers = bindings(&mut module, set, &mut reflection)?;
    reflection.sampled_pairs = sampled_pairs(&module, &registers)?;
    draw_bases(&mut module, variant, &mut reflection)?;
    Ok((module, reflection, stage))
}

/// What a pixel program asks of the pipeline beyond its code: early tests (reported, since WGSL cannot
/// force them), writes to storage, discards, and running once per sample.
fn pixel_extras(module: &Module) -> StageExtras {
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
    let mut side_effects = false;
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
    if ep.stage == naga::ShaderStage::Vertex
        && !builtins
            .iter()
            .any(|(b, _)| matches!(b, BuiltIn::Position { .. }))
    {
        return Err(ConvertError::Unsupported("VertexWithoutPosition"));
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
/// the vertex stage's helper constants.
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
        DrawBaseSource::Helpers => Some(helper_block(module, reflection)),
        DrawBaseSource::Arguments => {
            return Err(ConvertError::Unsupported("DrawBaseFromArguments"));
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

/// The (texture register, sampler register) pairs the program samples with, in the order its code
/// first uses them.
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

/// Numbers every resource by the rule of 6.4 for the stage's set (Direct3D 10 and 11's layout until
/// root signatures number Direct3D 12's), reading dxil-spirv's register (binding) and space (group).
/// Returns each resource's register.
fn bindings(
    module: &mut Module,
    set: u32,
    reflection: &mut Reflection,
) -> Result<Vec<(Handle<GlobalVariable>, u32)>, ConvertError> {
    let mut registers = Vec::new();
    for (handle, var) in module.global_variables.iter_mut() {
        let Some(binding) = var.binding.as_mut() else {
            continue;
        };
        let kind = match (var.space, &module.types[var.ty].inner) {
            (AddressSpace::Uniform, _) => SlotKind::Uniform,
            (AddressSpace::Storage { .. }, _) => SlotKind::Storage,
            (AddressSpace::Handle, TypeInner::Sampler { .. }) => SlotKind::Sampler,
            (
                AddressSpace::Handle,
                TypeInner::Image {
                    class: ImageClass::Storage { .. },
                    ..
                },
            ) => SlotKind::Storage,
            (AddressSpace::Handle, TypeInner::Image { .. }) => SlotKind::Texture,
            (AddressSpace::Handle, TypeInner::BindingArray { .. }) => {
                return Err(ConvertError::Unsupported("ResourceArray"));
            }
            _ => return Err(ConvertError::Unsupported("ResourceKind")),
        };
        let (space, register) = (binding.group, binding.binding);
        // Direct3D 11's numbering knows one space; other spaces are numbered by a root signature.
        if space != 0 {
            return Err(ConvertError::Unsupported("RegisterSpace"));
        }
        let number = kind
            .binding(register)
            .ok_or(ConvertError::Unsupported("BindingsBeyondLimit"))?;
        *binding = naga::ResourceBinding {
            group: set,
            binding: number,
        };
        reflection.bindings.push(ReflectedBinding {
            kind,
            space,
            register,
            count: 1,
            set,
            binding: number,
            added: None,
        });
        registers.push((handle, register));
    }
    Ok(registers)
}
