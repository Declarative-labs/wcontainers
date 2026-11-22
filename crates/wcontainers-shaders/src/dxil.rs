//! DXIL through dxil-spirv (C++, built by `build.rs`) to SPIR-V, read by naga's SPIR-V reader, then
//! given the binding rule of plan section 6.4 and stripped of what WGSL cannot express.

pub(crate) mod arrays;
mod buffer_arrays;
mod components;
mod depth;
mod geometry;
pub(crate) mod images;
mod nonfinite;
mod one_d;
mod registers;
mod selected;
mod staged;
mod tessellation;
mod widths;

use crate::ConvertError;
use crate::container::{self, Element, signature};
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
    fn wcs_dxil_compared_gather_channels(
        data: *const u8,
        size: usize,
        out: *mut *mut u32,
        out_words: *mut usize,
        error: *mut *mut c_char,
    ) -> c_int;
    fn wcs_dxil_declared_registers(
        data: *const u8,
        size: usize,
        out: *mut *mut u32,
        out_words: *mut usize,
        error: *mut *mut c_char,
    ) -> c_int;
    fn wcs_dxil_views(
        data: *const u8,
        size: usize,
        out: *mut *mut u32,
        out_words: *mut usize,
        error: *mut *mut c_char,
    ) -> c_int;
    fn wcs_free(p: *mut c_void);
}

/// The reason in `error` from the C side (a log line, its line end dropped), freed.
fn reason(error: *mut c_char) -> String {
    if error.is_null() {
        return String::from("dxil-spirv gave no reason");
    }
    // SAFETY: a NUL-terminated string from the C side, freed right after.
    let s = unsafe { core::ffi::CStr::from_ptr(error) }
        .to_string_lossy()
        .trim_end()
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

/// The resource classes of Direct3D 12 the SPIR-V's descriptor sets name (`cpp/boundary.h`).
pub(super) use crate::ir::class::COUNTER;
use crate::ir::class::{CONSTANTS, SAMPLER, STORAGE, VIEW};

/// One resource's (class, space, register).
type Resource = [u32; crate::ir::table_word::COUNT];

/// The words of one [`Resource`], and of one constant buffer's (space, register, size in bytes) on
/// both routes.
pub(crate) const TRIPLE: usize = core::mem::size_of::<Resource>() / core::mem::size_of::<u32>();

/// Each resource, at the index the SPIR-V's binding names.
type Resources = Vec<Resource>;

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
    let resources = self::words(table, table_words)
        .as_chunks::<TRIPLE>()
        .0
        .to_vec();
    Ok((spirv, resources))
}

/// The status the C side returns for a call that succeeded.
const SUCCESS: c_int = 0;

/// A C side reader of the DXIL's module, giving words it allocated.
type ListReader =
    unsafe extern "C" fn(*const u8, usize, *mut *mut u32, *mut usize, *mut *mut c_char) -> c_int;

/// The words `read` gives for `dxil`.
fn resource_list(dxil: &[u8], read: ListReader) -> Result<Vec<u32>, ConvertError> {
    let mut out = core::ptr::null_mut();
    let mut n = 0;
    let mut error = core::ptr::null_mut();
    // SAFETY: the pointers are valid for the call; the C side allocates `out` and `error` with malloc.
    let status = unsafe { read(dxil.as_ptr(), dxil.len(), &mut out, &mut n, &mut error) };
    if status != SUCCESS {
        return Err(ConvertError::Rejected(reason(error)));
    }
    Ok(words(out, n))
}

/// (space, register, size in bytes) of every constant buffer the DXIL declares.
fn constant_sizes(dxil: &[u8]) -> Result<Vec<u32>, ConvertError> {
    resource_list(dxil, wcs_dxil_constant_sizes)
}

/// The channel of each comparison gather the DXIL calls, `u32::MAX` for one not constant.
fn compared_gather_channels(dxil: &[u8]) -> Result<Vec<u32>, ConvertError> {
    resource_list(dxil, wcs_dxil_compared_gather_channels)
}

/// Every register the DXIL's resource list declares, used by the program or not, as resources.
fn declared_registers(dxil: &[u8]) -> Result<Resources, ConvertError> {
    Ok(resource_list(dxil, wcs_dxil_declared_registers)?
        .as_chunks::<TRIPLE>()
        .0
        .to_vec())
}

/// The views of the DXIL's resource list, as `cpp/dxil.cpp`'s `views` gives them
/// (`container::dxil_views`).
pub(crate) fn views(dxil: &[u8]) -> Result<Vec<u32>, ConvertError> {
    resource_list(dxil, wcs_dxil_views)
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
            .as_chunks::<TRIPLE>()
            .0
            .iter()
            // An element of an array of constant buffers (`buffer_arrays`) is sized as its array.
            .filter(|d| d[0] == space && d[1] <= register)
            .max_by_key(|d| d[1])
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
    if crate::container::program_type(dxil) == Some(crate::ir::program::GEOMETRY)
        && container::geometry_input_primitive(dxil)
            .is_some_and(|p| p >= container::FIRST_PATCH_PRIMITIVE)
    {
        return Err(ConvertError::Unsupported(
            crate::refusal::GEOMETRY_PATCH_INPUT,
        ));
    }
    let (mut spirv, mut resources) = to_spirv(dxil, variant.samples)?;
    if evaluates_inputs(&spirv) {
        return Err(ConvertError::Unsupported(crate::refusal::INTERPOLATE_AT));
    }
    if let Some(name) = absent_extension(&spirv) {
        return Err(ConvertError::Unsupported(name));
    }
    widths::widen(&mut spirv)?;
    registers::encode(&mut spirv);
    let geometry = geometry::prepare(&mut spirv)?;
    let tessellation = tessellation::prepare(&mut spirv, dxil)?;
    let selected = selected::prepare(&mut spirv)?;
    let mut declared = images::prepare(&mut spirv);
    let options = naga::front::spv::Options {
        adjust_coordinate_space: false,
        strict_capabilities: false,
        block_ctx_dump_prefix: None,
    };
    let mut module = widths::read(&spirv, &options)?;
    nonfinite::calls(&mut module)?;
    arrays::split(&mut module, &mut resources, &mut declared, variant)?;
    depth::widen(&mut module)?;
    depth::compared_gathers(&mut module, &compared_gather_channels(dxil)?)?;
    one_d::as_two_d(&mut module, &declared)?;
    // Before the atomics' stand-ins are made, as it moves the module's functions.
    components::rows(&mut module)?;
    let mut atomics = images::texel_atomics(&mut module)?;
    // WebGPU's storage buffers are read-only or read-write; one the program only writes is the latter.
    for (_, v) in module.global_variables.iter_mut() {
        if let AddressSpace::Storage { access } = &mut v.space
            && *access == naga::StorageAccess::STORE
        {
            *access |= naga::StorageAccess::LOAD;
        }
    }
    sample_mask_input(&mut module);
    sample_mask_output(&mut module);
    unconverted(&module)?;
    if !module.overrides.is_empty() {
        // dxil-spirv's only specialisation constant here: the targets' sample count.
        return Err(ConvertError::Unsupported(crate::refusal::SAMPLE_COUNT));
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
        _ => return Err(ConvertError::Unsupported(crate::refusal::STAGE)),
    };
    let extras = match stage {
        ShaderStage::Compute => StageExtras::Compute {
            workgroup_size: ep.workgroup_size,
        },
        ShaderStage::Fragment => pixel_extras(&module, !atomics.is_empty()),
        _ => StageExtras::None,
    };
    module.entry_points[0].name = crate::ENTRY_POINT.into();
    module.entry_points[0].early_depth_test = None;
    let placed = match stage {
        ShaderStage::Compute => Vec::new(),
        _ => registers::lay_out(&mut module)?,
    };
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
        slow_paths: Vec::new(),
    };
    if atomic_signedness_differs(&spirv) {
        reflection
            .slow_paths
            .push(wcontainers_gpu::SlowPath::AtomicExchangeLoop);
    }
    let ep = &module.entry_points[0].function;
    let inputs = signature(dxil, &container::ISG1);
    let outputs = signature(dxil, &container::OSG1);
    let pixel = stage == ShaderStage::Fragment;
    for a in &ep.arguments {
        stage_values(&module, a.ty, a.binding.as_ref(), &mut reflection.inputs);
    }
    if let Some(r) = &ep.result {
        stage_values(&module, r.ty, r.binding.as_ref(), &mut reflection.outputs);
    }
    container::place_system_values(&mut reflection.inputs, &inputs);
    container::place_system_values(&mut reflection.outputs, &outputs);
    for p in &placed {
        let (signature, into) = match p.output {
            false => (&inputs, &mut reflection.inputs),
            true => (&outputs, &mut reflection.outputs),
        };
        into.push(placed_value(&module, p, pixel && p.output, signature));
    }
    let slots = bindings(
        &mut module,
        &resources,
        &declared_registers(dxil)?,
        set,
        &mut reflection,
    )?;
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
    selected::finish(&mut module, &selected, variant, &mut reflection, &inputs)?;
    let stage = match &geometry {
        Some(declared) => {
            let (extras, [inputs, outputs]) =
                geometry::finish(&mut module, declared, variant, [&inputs, &outputs])?;
            reflection.stage = SourceStage::Geometry;
            reflection.extras = extras;
            reflection.inputs = inputs;
            reflection.outputs = outputs;
            match module.entry_points[0].stage {
                naga::ShaderStage::Vertex => ShaderStage::Vertex,
                _ => ShaderStage::Compute,
            }
        }
        None => match &tessellation {
            Some(declared) => {
                let (source, extras, inputs) =
                    tessellation::finish(&mut module, declared, [&inputs, &outputs])?;
                reflection.stage = source;
                reflection.extras = extras;
                reflection.inputs = inputs;
                stage
            }
            None => stage,
        },
    };
    helper_invocations(&mut module);
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

/// dxil-spirv declares the coverage input as SPIR-V's one-element array, which the reader keeps; the
/// common form takes a u32. The entry point's argument becomes the u32, stored into element 0 of the
/// reader's private copy.
fn sample_mask_input(module: &mut Module) {
    let u32_ty = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Scalar(Scalar::U32),
        },
        Span::UNDEFINED,
    );
    let Some(ep) = module.entry_points.first_mut() else {
        return;
    };
    let f = &mut ep.function;
    let Some(index) = f.arguments.iter().position(|a| {
        matches!(a.binding, Some(Binding::BuiltIn(BuiltIn::SampleMask)))
            && matches!(module.types[a.ty].inner, TypeInner::Array { .. })
    }) else {
        return;
    };
    f.arguments[index].ty = u32_ty;
    let argument = f
        .expressions
        .iter()
        .find(|(_, e)| matches!(e, Expression::FunctionArgument(i) if *i as usize == index))
        .map(|(h, _)| h);
    let Some(argument) = argument else {
        return;
    };
    let mut body = naga::Block::with_capacity(f.body.len());
    for (s, span) in core::mem::take(&mut f.body).span_into_iter() {
        match s {
            Statement::Store { pointer, value } if value == argument => {
                let element = f.expressions.append(
                    Expression::AccessIndex {
                        base: pointer,
                        index: 0,
                    },
                    span,
                );
                body.push(
                    Statement::Emit(Range::new_from_bounds(element, element)),
                    span,
                );
                body.push(
                    Statement::Store {
                        pointer: element,
                        value,
                    },
                    span,
                );
            }
            s => body.push(s, span),
        }
    }
    f.body = body;
}

/// Refuses by name what the reader's module has that the common form cannot take as it stands.
fn unconverted(module: &Module) -> Result<(), ConvertError> {
    let ep = module
        .entry_points
        .first()
        .ok_or(ConvertError::Malformed("no entry point".into()))?;
    // Every built-in the entry point reads or writes, with its type.
    let mut builtins = Vec::new();
    let mut add = |ty: Handle<Type>, binding: Option<&Binding>| {
        let mut one = |ty, binding: Option<&Binding>| {
            if let Some(Binding::BuiltIn(b)) = binding {
                builtins.push((*b, ty));
            }
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
        add(a.ty, a.binding.as_ref());
    }
    if let Some(r) = ep.function.result.as_ref() {
        add(r.ty, r.binding.as_ref());
    }
    if ep.stage != naga::ShaderStage::Fragment
        && builtins.iter().any(|(b, _)| *b == BuiltIn::PrimitiveIndex)
    {
        return Err(ConvertError::Unsupported(crate::refusal::PRIMITIVE_ID));
    }
    // Cull distances are drawn through the stage helper from a vertex program only, and no stage
    // reads clip distances.
    if ep.stage != naga::ShaderStage::Vertex
        && builtins.iter().any(|(b, _)| *b == BuiltIn::CullDistance)
    {
        return Err(ConvertError::Unsupported(crate::refusal::CULL_DISTANCE));
    }
    if ep.stage == naga::ShaderStage::Fragment
        && builtins.iter().any(|(b, _)| *b == BuiltIn::ClipDistances)
    {
        return Err(ConvertError::Unsupported(crate::refusal::CLIP_DISTANCE));
    }
    // WebGPU's subgroups serve pixel and compute programs only, and have no subgroup barrier.
    if module.uses_subgroups() && ep.stage == naga::ShaderStage::Vertex {
        return Err(ConvertError::Unsupported(
            crate::refusal::SUBGROUPS_IN_VERTEX,
        ));
    }
    let functions = core::iter::once(&ep.function).chain(module.functions.iter().map(|(_, f)| f));
    for f in functions {
        let mut body = f.body.clone();
        let mut barrier = false;
        each_block(&mut body, &mut |b| {
            barrier |= b.iter().any(|s| {
                matches!(s, Statement::ControlBarrier(b) | Statement::MemoryBarrier(b)
                    if b.contains(naga::Barrier::SUB_GROUP))
            });
        });
        if barrier {
            return Err(ConvertError::Unsupported(crate::refusal::SUBGROUP_BARRIER));
        }
    }
    // dxil-spirv reads a component of a constant buffer picked at run time through a second view of
    // the buffer as scalars, 4 bytes apart; WebGPU's uniform arrays step by 16 bytes, and WGSL takes
    // one variable per binding.
    let scalar_view = |v: &GlobalVariable| match module.types[v.ty].inner {
        TypeInner::Struct { ref members, .. } => members.iter().any(|m| {
            matches!(module.types[m.ty].inner, TypeInner::Array { stride, .. }
                if !naga::proc::Alignment::MIN_UNIFORM.is_aligned(stride))
        }),
        _ => false,
    };
    if module
        .global_variables
        .iter()
        .any(|(_, v)| v.space == AddressSpace::Uniform && scalar_view(v))
    {
        return Err(ConvertError::Unsupported(
            crate::refusal::CONSTANT_COMPONENT_INDEXING,
        ));
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
        return Err(ConvertError::Unsupported(
            crate::refusal::NON_FINITE_CONSTANT,
        ));
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

/// dxil-spirv declares the coverage output as SPIR-V's one-element array too: the entry point's result
/// structure takes a u32 in its place, the array's element 0, composed again before each return.
fn sample_mask_output(module: &mut Module) {
    let Some(result) = module
        .entry_points
        .first()
        .and_then(|e| e.function.result.as_ref())
    else {
        return;
    };
    let old = result.ty;
    let TypeInner::Struct { members, span } = module.types[old].inner.clone() else {
        return;
    };
    let Some(mask) = members.iter().position(|m| {
        matches!(m.binding, Some(Binding::BuiltIn(BuiltIn::SampleMask)))
            && matches!(module.types[m.ty].inner, TypeInner::Array { .. })
    }) else {
        return;
    };
    let u32_ty = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Scalar(Scalar::U32),
        },
        Span::UNDEFINED,
    );
    // The reader starts the private array at its element's value (all samples covered): the
    // array of that value.
    let array = members[mask].ty;
    for (_, v) in module.global_variables.iter_mut() {
        if let Some(init) = v.init
            && v.ty == array
            && matches!(module.global_expressions[init], Expression::Literal(_))
        {
            v.init = Some(module.global_expressions.append(
                Expression::Compose {
                    ty: array,
                    components: alloc::vec![init],
                },
                Span::UNDEFINED,
            ));
        }
    }
    let mut members = members;
    members[mask].ty = u32_ty;
    let new = module.types.insert(
        Type {
            name: module.types[old].name.clone(),
            inner: TypeInner::Struct { members, span },
        },
        Span::UNDEFINED,
    );
    let count = match &module.types[new].inner {
        TypeInner::Struct { members, .. } => members.len() as u32,
        _ => return,
    };
    let f = &mut module.entry_points[0].function;
    f.result.as_mut().unwrap().ty = new;
    let mut body = core::mem::take(&mut f.body);
    each_block(&mut body, &mut |block| {
        let mut out = naga::Block::with_capacity(block.len());
        for (s, span) in core::mem::take(block).span_into_iter() {
            let Statement::Return { value: Some(value) } = s else {
                out.push(s, span);
                continue;
            };
            let first = f.expressions.len();
            let components = (0..count)
                .map(|index| {
                    let member = f
                        .expressions
                        .append(Expression::AccessIndex { base: value, index }, span);
                    if index as usize != mask {
                        return member;
                    }
                    f.expressions.append(
                        Expression::AccessIndex {
                            base: member,
                            index: 0,
                        },
                        span,
                    )
                })
                .collect();
            let composed = f.expressions.append(
                Expression::Compose {
                    ty: new,
                    components,
                },
                span,
            );
            let start = f.expressions.iter().nth(first).map(|(h, _)| h).unwrap();
            out.push(
                Statement::Emit(Range::new_from_bounds(start, composed)),
                span,
            );
            out.push(
                Statement::Return {
                    value: Some(composed),
                },
                span,
            );
        }
        *block = out;
    });
    f.body = body;
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

/// Sets the flag naga's SPIR-V reader reads for `OpIsHelperInvocationEXT` (`IsHelperLane()` and the
/// guards dxil-spirv puts around wave operations in pixel shaders). WGSL has no helper query, so a
/// pixel program takes it from its coverage, which is zero in helper invocations, and sets it at each
/// demotion (`Kill`, which WGSL's `discard` makes a helper). Elsewhere the flag stays false. Both
/// routes use the flag: DXIL's helper queries, and the compare-exchange loops of either route.
pub(crate) fn helper_invocations(module: &mut Module) {
    let Some((flag, _)) = module
        .global_variables
        .iter()
        .find(|(_, v)| v.name.as_deref() == Some(naga::front::spv::HELPER_INVOCATION))
    else {
        return;
    };
    let demotions = |f: &mut Function| {
        let pointer = f
            .expressions
            .append(Expression::GlobalVariable(flag), Span::UNDEFINED);
        let yes = f
            .expressions
            .append(Expression::Literal(Literal::Bool(true)), Span::UNDEFINED);
        each_block(&mut f.body, &mut |block| {
            let mut out = naga::Block::with_capacity(block.len());
            for (s, span) in core::mem::take(block).span_into_iter() {
                if matches!(s, Statement::Kill) {
                    out.push(
                        Statement::Store {
                            pointer,
                            value: yes,
                        },
                        span,
                    );
                }
                out.push(s, span);
            }
            *block = out;
        });
    };
    for (_, f) in module.functions.iter_mut() {
        demotions(f);
    }
    for ep in &mut module.entry_points {
        demotions(&mut ep.function);
    }
    let ep = &mut module.entry_points[0];
    if ep.stage != naga::ShaderStage::Fragment {
        return;
    }
    let u32_ty = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Scalar(Scalar::U32),
        },
        Span::UNDEFINED,
    );
    let f = &mut ep.function;
    let mask = |b: &Option<Binding>| matches!(b, Some(Binding::BuiltIn(BuiltIn::SampleMask)));
    // The coverage argument, or the member of an argument's structure holding it.
    let found = f.arguments.iter().enumerate().find_map(|(i, a)| {
        if mask(&a.binding) {
            return Some((i, None));
        }
        match &module.types[a.ty].inner {
            TypeInner::Struct { members, .. } => members
                .iter()
                .position(|m| mask(&m.binding))
                .map(|m| (i, Some(m as u32))),
            _ => None,
        }
    });
    let (index, member) = found.unwrap_or_else(|| {
        f.arguments.push(FunctionArgument {
            name: Some("coverage".into()),
            ty: u32_ty,
            binding: Some(Binding::BuiltIn(BuiltIn::SampleMask)),
        });
        (f.arguments.len() - 1, None)
    });
    let e = &mut f.expressions;
    let zero = e.append(Expression::Literal(Literal::U32(0)), Span::UNDEFINED);
    let mut coverage = e.append(Expression::FunctionArgument(index as u32), Span::UNDEFINED);
    let mut first = None;
    if let Some(m) = member {
        coverage = e.append(
            Expression::AccessIndex {
                base: coverage,
                index: m,
            },
            Span::UNDEFINED,
        );
        first = Some(coverage);
    }
    let helper = e.append(
        Expression::Binary {
            op: naga::BinaryOperator::Equal,
            left: coverage,
            right: zero,
        },
        Span::UNDEFINED,
    );
    let pointer = e.append(Expression::GlobalVariable(flag), Span::UNDEFINED);
    let mut start = naga::Block::new();
    start.push(
        Statement::Emit(Range::new_from_bounds(first.unwrap_or(helper), helper)),
        Span::UNDEFINED,
    );
    start.push(
        Statement::Store {
            pointer,
            value: helper,
        },
        Span::UNDEFINED,
    );
    for (s, span) in core::mem::take(&mut f.body).span_into_iter() {
        start.push(s, span);
    }
    f.body = start;
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
        if (scalar.width, width) != (FLOAT_BYTES, FLOAT_BYTES) {
            return Err(ConvertError::Unsupported(
                crate::refusal::FLOAT_TO_INT_WIDTH,
            ));
        }
        found.push((h, (kind, size), expr));
    }
    Ok(found)
}

/// A function giving Direct3D's f32 to integer conversion: 0 for NaN, the integer's limits at and
/// past them.
/// The bytes of the 32-bit floats and integers Direct3D's conversions take.
const FLOAT_BYTES: u8 = core::mem::size_of::<f32>() as u8;

fn float_to_int_function(module: &mut Module, (kind, size): FloatToInt) -> Function {
    let mut ty = |kind| {
        let scalar = Scalar {
            kind,
            width: FLOAT_BYTES,
        };
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
        // i32::MIN as f32 is exact, and the first float above the range is its negation.
        (
            -(i32::MIN as f32),
            i32::MIN as f32,
            max,
            min,
            Literal::I32(0),
        )
    } else {
        let (max, zero) = (Literal::U32(u32::MAX), Literal::U32(0));
        // u32::MAX rounds up to 2^32, the first float above the range.
        (u32::MAX as f32, 0.0, max, zero, zero)
    };
    let literals = [
        Literal::F32(above),
        Literal::F32(below),
        max,
        min,
        zero,
        Literal::U32(!(-0.0f32).to_bits()),
        Literal::U32(f32::INFINITY.to_bits()),
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
        convert: Some(FLOAT_BYTES),
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

/// Copies of the module's functions, then of its entry points' functions, in the order an index
/// into "functions, then entry points" names them.
fn all_functions(module: &Module) -> Vec<Function> {
    module
        .functions
        .iter()
        .map(|(_, f)| f.clone())
        .chain(module.entry_points.iter().map(|ep| ep.function.clone()))
        .collect()
}

/// One expression to be computed by a helper: the function holding it (an index into functions,
/// then entry points), the expression, and the run-time operands passed to the helper with their
/// types.
pub(crate) struct Read {
    pub(crate) function: usize,
    pub(crate) user: Handle<Expression>,
    pub(crate) passed: Vec<(Handle<Expression>, Handle<Type>)>,
}

/// Makes each expression of `found` (function index, expression) a call of the helper `build` makes
/// for it, which takes the expression's operands that are neither constants nor globals. Each
/// function's operands are typed after `stand_in` edits a copy of it with its expressions of `found`.
pub(crate) fn outline(
    module: &mut Module,
    found: &[(usize, Handle<Expression>)],
    stand_in: impl Fn(&mut Function, &[Handle<Expression>]),
    build: impl Fn(&mut Module, &Function, &Read) -> Result<Function, ConvertError>,
) -> Result<(), ConvertError> {
    let n = module.functions.len();
    let mut reads = Vec::new();
    for (i, mut f) in all_functions(module).into_iter().enumerate() {
        let here: Vec<_> = found.iter().filter(|r| r.0 == i).map(|r| r.1).collect();
        if here.is_empty() {
            continue;
        }
        let original = f.expressions.clone();
        stand_in(&mut f, &here);
        let mut typifier = naga::front::Typifier::new();
        let ctx = naga::proc::ResolveContext::with_locals(module, &f.local_variables, &f.arguments);
        for &h in &here {
            let mut user = original[h].clone();
            let mut passed = Vec::new();
            for o in arrays::operands(&mut user)? {
                let o = *o;
                if arrays::constant(&f, o)
                    || matches!(f.expressions[o], Expression::GlobalVariable(_))
                {
                    continue;
                }
                typifier
                    .grow(o, &f.expressions, &ctx)
                    .map_err(|e| ConvertError::Malformed(format!("{e}")))?;
                passed.push((o, typifier.get(o, &module.types).clone()));
            }
            reads.push((i, h, passed));
        }
    }
    let reads: Vec<Read> = reads
        .into_iter()
        .map(|(function, user, passed)| Read {
            function,
            user,
            passed: passed
                .into_iter()
                .map(|(e, inner)| {
                    let ty = module
                        .types
                        .insert(Type { name: None, inner }, Span::UNDEFINED);
                    (e, ty)
                })
                .collect(),
        })
        .collect();
    let mut built = Vec::new();
    for r in &reads {
        let f = match r.function < n {
            true => {
                module
                    .functions
                    .iter()
                    .nth(r.function)
                    .expect("a function")
                    .1
            }
            false => &module.entry_points[r.function - n].function,
        }
        .clone();
        built.push(build(module, &f, r)?);
    }
    let (helpers, moved) = functions_first(module, |module| -> Vec<Handle<Function>> {
        built
            .into_iter()
            .map(|h| module.functions.append(h, Span::UNDEFINED))
            .collect()
    });
    for i in 0..n + module.entry_points.len() {
        let f = match moved.get(i) {
            Some(&h) => &mut module.functions[h],
            None => &mut module.entry_points[i - n].function,
        };
        let calls: Vec<Placed> = reads
            .iter()
            .zip(&helpers)
            .filter(|(r, _)| r.function == i)
            .map(|(r, &helper)| Placed {
                result: r.user,
                function: helper,
                arguments: r.passed.iter().map(|p| p.0).collect(),
                pre: None,
            })
            .collect();
        place_calls(f, &calls);
    }
    Ok(())
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
        name: Some(crate::ENTRY_POINT.into()),
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
    let (ty, size) = crate::build::draw_bases_block(module);
    reflection.helpers.push(HelperSlot {
        value: HelperValue::DrawBase,
        offset: 0,
    });
    reflection.helper_size = size;
    module.global_variables.append(
        GlobalVariable {
            name: Some(crate::build::HELPERS.into()),
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

/// The reflection's built-in inputs or outputs: the entry point's, named from the signature later
/// ([`crate::container::place_system_values`]).
fn stage_values(
    module: &Module,
    ty: Handle<Type>,
    binding: Option<&Binding>,
    out: &mut Vec<StageValue>,
) {
    if let TypeInner::Struct { ref members, .. } = module.types[ty].inner {
        for m in members {
            stage_values(module, m.ty, m.binding.as_ref(), out);
        }
        return;
    }
    let Some(Binding::BuiltIn(b)) = binding else {
        return;
    };
    let (kind, size) = match module.types[ty].inner {
        TypeInner::Scalar(s) => (s.kind, 1),
        TypeInner::Vector { size, scalar } => (scalar.kind, size as u32),
        TypeInner::Array { .. } => (ScalarKind::Float, 1),
        _ => return,
    };
    let system = match b {
        BuiltIn::Position { .. } => SystemValue::Position,
        BuiltIn::VertexIndex => SystemValue::VertexId,
        BuiltIn::InstanceIndex => SystemValue::InstanceId,
        BuiltIn::PrimitiveIndex => SystemValue::PrimitiveId,
        BuiltIn::FrontFacing => SystemValue::FrontFacing,
        BuiltIn::SampleIndex => SystemValue::SampleIndex,
        BuiltIn::SampleMask => SystemValue::Coverage,
        BuiltIn::FragDepth => SystemValue::Depth,
        BuiltIn::ClipDistances => SystemValue::ClipDistance,
        BuiltIn::CullDistance => SystemValue::CullDistance,
        _ => return,
    };
    let scalar = value_kind(kind);
    let interpolation = if scalar == ValueKind::Float {
        Interpolation::Perspective
    } else {
        Interpolation::Flat
    };
    out.push(StageValue {
        semantic: String::new(),
        semantic_index: 0,
        register: 0,
        mask: ((1u32 << size) - 1) as u8,
        scalar,
        interpolation,
        system: Some(system),
        location: 0,
        component: 0,
        stream: 0,
    });
}

fn value_kind(kind: ScalarKind) -> ValueKind {
    match kind {
        ScalarKind::Float => ValueKind::Float,
        ScalarKind::Sint => ValueKind::Sint,
        _ => ValueKind::Uint,
    }
}

/// The reflection of a value at a register, named from the signature, with the interpolation its
/// register has in the finished entry point.
fn placed_value(
    module: &Module,
    p: &registers::Placed,
    pixel_output: bool,
    signature: &[Element],
) -> StageValue {
    use crate::ir::sysval;
    let (kind, size) = match module.types[p.ty].inner {
        TypeInner::Vector { size, scalar } => (scalar.kind, size as u32),
        TypeInner::Scalar(s) => (s.kind, 1),
        _ => (ScalarKind::Float, 1),
    };
    let scalar = value_kind(kind);
    let ep = &module.entry_points[0].function;
    let at = |b: &Option<Binding>| match b {
        Some(Binding::Location {
            location,
            interpolation,
            sampling,
            ..
        }) if *location == p.register => Some((*interpolation, *sampling)),
        _ => None,
    };
    let modes = if p.output {
        ep.result.as_ref().and_then(|r| {
            at(&r.binding).or_else(|| match &module.types[r.ty].inner {
                TypeInner::Struct { members, .. } => members.iter().find_map(|m| at(&m.binding)),
                _ => None,
            })
        })
    } else {
        ep.arguments.iter().find_map(|a| at(&a.binding))
    };
    use naga::{Interpolation as I, Sampling as S};
    let interpolation = match modes.unwrap_or_default() {
        _ if scalar != ValueKind::Float => Interpolation::Flat,
        (Some(I::Flat), _) => Interpolation::Flat,
        (Some(I::Linear), Some(S::Centroid)) => Interpolation::LinearCentroid,
        (Some(I::Linear), Some(S::Sample)) => Interpolation::LinearSample,
        (Some(I::Linear), _) => Interpolation::Linear,
        (_, Some(S::Centroid)) => Interpolation::PerspectiveCentroid,
        (_, Some(S::Sample)) => Interpolation::PerspectiveSample,
        _ => Interpolation::Perspective,
    };
    let element = signature.iter().find(|e| {
        e.register == p.register
            && (e.system == sysval::NONE || e.system == sysval::TARGET)
            && e.mask.trailing_zeros() == p.component
    });
    let own_mask = (((1u32 << size) - 1) << p.component) as u8;
    StageValue {
        semantic: element.map(|e| e.semantic.clone()).unwrap_or_default(),
        semantic_index: element.map_or(0, |e| e.index),
        register: p.register,
        mask: element.map_or(own_mask, |e| e.mask),
        scalar,
        interpolation,
        system: pixel_output.then_some(SystemValue::Target(p.register as u8)),
        location: p.register,
        component: p.component as u8,
        stream: 0,
    }
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
                return Err(ConvertError::Unsupported(
                    crate::refusal::SAMPLED_THROUGH_ARGUMENT,
                ));
            };
            if !pairs.contains(&(texture, sampler)) {
                pairs.push((texture, sampler));
            }
        }
    }
    Ok(pairs)
}

/// Numbers every resource by Direct3D 12's rule (6.4): within the stage's set, each kind's slots are
/// the registers of that kind the DXIL's resource list declares (`declared`, used by the program or
/// not, so the numbering is the one a front reads from the container) in (space, register) order,
/// from the kind's first binding, so register spaces and registers of any size fit while the program has no more resources
/// of a kind than its range holds. Kinds follow Direct3D's register classes as on the DXBC route
/// (shader resource views, buffers among them, are textures; unordered access views are storage); a
/// counter takes its view's slot. A slot (`binding - kind.first_binding()`) is what the variant's
/// per-slot fields and the sampled pairs name. Returns each resource's slot.
fn bindings(
    module: &mut Module,
    resources: &Resources,
    declared_registers: &Resources,
    set: u32,
    reflection: &mut Reflection,
) -> Result<Vec<(Handle<GlobalVariable>, u32)>, ConvertError> {
    let mut declared = Vec::new();
    for (handle, var) in module.global_variables.iter() {
        let Some(binding) = var.binding else {
            continue;
        };
        if let TypeInner::BindingArray { .. } = module.types[var.ty].inner {
            return Err(ConvertError::Unsupported(crate::refusal::RESOURCE_ARRAY));
        }
        let &[class, space, register] = resources
            .get(binding.binding as usize)
            .ok_or_else(|| ConvertError::Malformed(PAST_THE_LIST.into()))?;
        let kind =
            slot_kind(binding.group).ok_or_else(|| ConvertError::Malformed(NO_CLASS.into()))?;
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
    let listed = declared_registers
        .iter()
        .filter_map(|&[class, space, register]| Some((rank(slot_kind(class)?), space, register)));
    let mut order: Vec<(u32, u32, u32)> = declared
        .iter()
        .map(|&(_, kind, space, register)| (rank(kind), space, register))
        .chain(listed)
        .collect();
    order.sort_unstable();
    order.dedup();
    let mut slots = Vec::new();
    for (handle, kind, space, register) in declared {
        let key = (rank(kind), space, register);
        let first = order.partition_point(|o| o.0 < key.0);
        let slot = (order.partition_point(|o| *o < key) - first) as u32;
        let number = kind.binding(slot).ok_or(ConvertError::Unsupported(
            crate::refusal::BINDINGS_BEYOND_LIMIT,
        ))?;
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
            stride: 0,
        });
        slots.push((handle, slot));
    }
    Ok(slots)
}

/// The slot kind of a resource class (`cpp/boundary.h`).
/// Why a resource is malformed: its class is none of Direct3D's.
const NO_CLASS: &str = "a resource of no class";
/// Why a resource is malformed: its binding names no entry of the resource list.
const PAST_THE_LIST: &str = "a binding past the resource list";

fn slot_kind(class: u32) -> Option<SlotKind> {
    match class {
        CONSTANTS => Some(SlotKind::Uniform),
        VIEW => Some(SlotKind::Texture),
        STORAGE => Some(SlotKind::Storage),
        SAMPLER => Some(SlotKind::Sampler),
        COUNTER => Some(SlotKind::Counter),
        _ => None,
    }
}

/// SPIR-V's physical layout (its specification, 2.3 and 2.2.1): the header's words before the first
/// instruction, and where an instruction's first word keeps its word count and its opcode.
pub(crate) mod words {
    pub const HEADER: usize = 5;
    /// The header's word holding the bound of the module's ids.
    pub const BOUND: usize = 3;
    pub const COUNT_SHIFT: u32 = u16::BITS;
    pub const OPCODE_MASK: u32 = u16::MAX as u32;

    /// `OpDecorate`'s words (3.32.2): the target, the decoration, then its first literal.
    pub mod decorate {
        pub const TARGET: usize = 1;
        pub const DECORATION: usize = 2;
        pub const LITERAL: usize = 3;
        pub const DECORATE_WORDS: usize = LITERAL + 1;
    }

    /// `OpTypeInt`'s words (3.32.6): the result, the width, then the signedness.
    pub mod type_int {
        pub const SIGNEDNESS: usize = 3;
        pub const TYPE_INT_WORDS: usize = SIGNEDNESS + 1;
    }

    /// `OpTypeImage`'s words (3.32.6) up to its format; the access qualifier may follow.
    pub mod type_image {
        pub const SAMPLED_TYPE: usize = 2;
        pub const DIM: usize = 3;
        pub const SAMPLED: usize = 7;
        pub const FORMAT: usize = 8;
        pub const TYPE_IMAGE_WORDS: usize = FORMAT + 1;
        /// `Sampled` of an image used without a sampler: a storage image.
        pub const STORAGE: u32 = 2;
    }

    /// `OpTypePointer`'s and `OpVariable`'s third word: the pointee type, the storage class.
    pub const POINTER_TYPE: usize = 3;
    pub const VARIABLE_STORAGE_CLASS: usize = 3;
    pub const POINTER_WORDS: usize = POINTER_TYPE + 1;

    /// A type declaration's result id.
    pub const RESULT_ID: usize = 1;
    /// An instruction's result type and result id, where it has both.
    pub const RESULT_TYPE: usize = 1;
    pub const RESULT_AFTER_TYPE: usize = 2;
    /// An atomic instruction's pointer, after its result type and result.
    pub const ATOMIC_POINTER: usize = 3;

    /// `OpExtInst`'s instruction number (3.32.4), after the result type, result and set.
    pub const EXT_INSTRUCTION: usize = 4;
}

/// Whether `spirv` takes an input at another place in the pixel (GLSL.std.450's `InterpolateAt*`),
/// which naga's reader has no form for.
fn evaluates_inputs(spirv: &[u32]) -> bool {
    use spirv::{GlslStd450Op as GLOp, Op};
    const EVALUATIONS: [GLOp; 3] = [
        GLOp::InterpolateAtCentroid,
        GLOp::InterpolateAtSample,
        GLOp::InterpolateAtOffset,
    ];
    let mut at = words::HEADER;
    while at < spirv.len() {
        let count = (spirv[at] >> words::COUNT_SHIFT) as usize;
        if count == 0 || at + count > spirv.len() {
            return false;
        }
        let instruction = at + words::EXT_INSTRUCTION;
        if spirv[at] & words::OPCODE_MASK == Op::ExtInst as u32
            && instruction < at + count
            && EVALUATIONS.iter().any(|&e| e as u32 == spirv[instruction])
        {
            return true;
        }
        at += count;
    }
    false
}

/// The name of the feature WebGPU lacks that `spirv` asks for by a SPIR-V extension: fragment
/// interlock (rasterizer ordered views), the stencil reference from a pixel program, and the layer
/// or viewport chosen before the geometry stage. The names are the DXBC route's.
fn absent_extension(spirv: &[u32]) -> Option<&'static str> {
    use spirv::{BuiltIn, Decoration, Op};
    let mut extensions = Vec::new();
    let mut layer = false;
    let mut at = words::HEADER;
    while at < spirv.len() {
        let count = (spirv[at] >> words::COUNT_SHIFT) as usize;
        if count == 0 || at + count > spirv.len() {
            return None;
        }
        let w = &spirv[at + 1..at + count];
        match Op::from_u32(spirv[at] & words::OPCODE_MASK) {
            Some(Op::Extension) => {
                let name: Vec<u8> = w.iter().flat_map(|x| x.to_le_bytes()).collect();
                extensions.push(name);
            }
            // OpDecorate: the target, the decoration, then the built-in.
            Some(Op::Decorate)
                if w.get(1) == Some(&(Decoration::BuiltIn as u32))
                    && w.get(2) == Some(&(BuiltIn::Layer as u32)) =>
            {
                layer = true;
            }
            _ => {}
        }
        at += count;
    }
    let has = |name: &str| {
        extensions
            .iter()
            .any(|e| e.split(|&b| b == 0).next() == Some(name.as_bytes()))
    };
    let (_, name) = ABSENT_EXTENSIONS.iter().find(|(e, _)| has(e))?;
    Some(match *name {
        crate::refusal::VIEWPORT_ARRAY_INDEX if layer => crate::refusal::RENDER_TARGET_ARRAY_INDEX,
        name => name,
    })
}

/// SPIR-V extensions for features WebGPU lacks, and the features' names.
const ABSENT_EXTENSIONS: [(&str, &str); 3] = [
    (
        "SPV_EXT_fragment_shader_interlock",
        crate::refusal::RASTERIZER_ORDERED_VIEW,
    ),
    ("SPV_EXT_shader_stencil_export", crate::refusal::STENCIL_REF),
    (
        "SPV_EXT_shader_viewport_index_layer",
        crate::refusal::VIEWPORT_ARRAY_INDEX,
    ),
];

/// Whether an atomic minimum or maximum in `spirv` is of the other signedness than the integers it
/// works on, which naga's reader makes a compare-exchange loop.
fn atomic_signedness_differs(spirv: &[u32]) -> bool {
    use spirv::Op;
    use std::collections::HashMap;
    const SIGNED: [Op; 2] = [Op::AtomicSMin, Op::AtomicSMax];
    const UNSIGNED: [Op; 2] = [Op::AtomicUMin, Op::AtomicUMax];
    // The instructions giving pointers: variables, access chains, texel pointers, parameters.
    const POINTERS: [Op; 6] = [
        Op::Variable,
        Op::AccessChain,
        Op::InBoundsAccessChain,
        Op::PtrAccessChain,
        Op::ImageTexelPointer,
        Op::FunctionParameter,
    ];
    // Signedness of integer types, pointee of pointer types, type of each result.
    let mut signed = HashMap::new();
    let mut pointee = HashMap::new();
    let mut types = HashMap::new();
    let mut atomics = Vec::new();
    let mut at = words::HEADER;
    while at < spirv.len() {
        let count = (spirv[at] >> words::COUNT_SHIFT) as usize;
        if count == 0 || at + count > spirv.len() {
            return false;
        }
        let Some(op) = Op::from_u32(spirv[at] & words::OPCODE_MASK) else {
            at += count;
            continue;
        };
        let w = &spirv[at..at + count];
        match op {
            Op::TypeInt if w.len() >= words::type_int::TYPE_INT_WORDS => {
                signed.insert(w[words::RESULT_ID], w[words::type_int::SIGNEDNESS] != 0);
            }
            Op::TypePointer if w.len() >= words::POINTER_WORDS => {
                pointee.insert(w[words::RESULT_ID], w[words::POINTER_TYPE]);
            }
            _ if (SIGNED.contains(&op) || UNSIGNED.contains(&op))
                && w.len() > words::ATOMIC_POINTER =>
            {
                atomics.push((SIGNED.contains(&op), w[words::ATOMIC_POINTER]));
            }
            _ => {}
        }
        if POINTERS.contains(&op) && w.len() > words::RESULT_AFTER_TYPE {
            types.insert(w[words::RESULT_AFTER_TYPE], w[words::RESULT_TYPE]);
        }
        at += count;
    }
    atomics.iter().any(|&(op_signed, pointer)| {
        types
            .get(&pointer)
            .and_then(|t| pointee.get(t))
            .and_then(|p| signed.get(p))
            .is_some_and(|&memory_signed| memory_signed != op_signed)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Registers the resource list declares but the program leaves unused still take their slots, as
    /// a front reading the container numbers them.
    #[test]
    fn declared_but_unused_registers_keep_their_slots() {
        let mut module = Module::default();
        let ty = module.types.insert(
            Type {
                name: None,
                inner: TypeInner::Image {
                    dim: naga::ImageDimension::D2,
                    arrayed: false,
                    class: naga::ImageClass::Storage {
                        format: naga::StorageFormat::R32Sint,
                        access: naga::StorageAccess::LOAD,
                    },
                },
            },
            Span::UNDEFINED,
        );
        // The program uses u1 and u3 of space 0; the resource list declares u0 to u3.
        let used = [1, 3];
        let resources: Resources = used.iter().map(|&r| [STORAGE, 0, r]).collect();
        let vars: Vec<_> = (0..used.len() as u32)
            .map(|binding| {
                module.global_variables.append(
                    naga::GlobalVariable {
                        name: None,
                        space: AddressSpace::Handle,
                        binding: Some(naga::ResourceBinding {
                            group: STORAGE,
                            binding,
                        }),
                        ty,
                        init: None,
                        memory_decorations: Default::default(),
                    },
                    Span::UNDEFINED,
                )
            })
            .collect();
        let declared: Resources = (0..=used[1]).map(|r| [STORAGE, 0, r]).collect();
        let mut reflection = Reflection {
            stage: SourceStage::Compute,
            inputs: Vec::new(),
            outputs: Vec::new(),
            bindings: Vec::new(),
            helpers: Vec::new(),
            helper_size: 0,
            sampled_pairs: Vec::new(),
            extras: StageExtras::None,
            slow_paths: Vec::new(),
        };
        let slots = bindings(&mut module, &resources, &declared, 0, &mut reflection).unwrap();
        assert_eq!(slots, [(vars[0], used[0]), (vars[1], used[1])]);
        let slots: Vec<u32> = reflection.bindings.iter().map(|b| b.slot()).collect();
        assert_eq!(slots, used);
    }
}
