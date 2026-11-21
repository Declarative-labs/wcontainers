//! Filtering and addressing in the program (`VariantKey::filter_in_shader`, `border_in_shader`): a
//! sample becomes a call into a library written in WGSL (`filter.wgsl`) that reads texels with loads and
//! filters them by the sampler's state from the helper block, so sampler changes build nothing.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use naga::{
    Block, Expression, Function, FunctionArgument, FunctionResult, Handle, ImageClass, ImageQuery,
    Literal, Module, SampleLevel, Scalar, ScalarKind, Span, Statement, Type, TypeInner,
};
use wcontainers_gpu::{Format, HelperValue, Reflection, ShaderStage, VariantFlags};

use super::{
    HelperMember, Outlined, Result, copy_constant, each_block, f32_lit, function, helper_once,
    index, load_helper, push, scalar, types_of, vector,
};
use crate::ConvertError;

const LIBRARY: &str = include_str!("filter.wgsl");

/// The texture shapes the library is made for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    D1,
    D2,
    D2Array,
    D3,
    /// Depth textures, read as Direct3D reads a depth view: (depth, 0, 0, 1).
    Depth2D,
    Depth2DArray,
}

/// The texel type of a float texture's declaration in the library, which a decoded texture's integer
/// type replaces.
const FLOAT_TEXELS: &str = "<f32>";
const UNSIGNED_TEXELS: &str = "<u32>";
const SIGNED_TEXELS: &str = "<i32>";
/// The library's function decoding an integer texel, as the source declares it with a placeholder
/// body ([`super::decoded::texel_function`] gives its body), and a decoded texture's load, its
/// texture's load in place of `LOAD`.
const DECODE_FUNCTION: &str = "decode_texel";
const DECODE_DECLARATION: &str =
    "fn decode_texel(raw: vec4<u32>) -> vec4<f32> { return vec4<f32>(raw); }\n";
const DECODED_LOAD: &str = "decode_texel(bitcast<vec4<u32>>(LOAD))";
const LOAD_PLACEHOLDER: &str = "LOAD";
const NO_DECODE_FUNCTION: &str = "the filter library has no decoding function";

impl Shape {
    /// The library for this shape: `filter.wgsl` with its texture type, axes and loads filled in;
    /// for a texture stored as integers of `decoded`'s kind, loads of those integers decoded.
    fn source(self, decoded: Option<ScalarKind>) -> String {
        let (texture, axes, taps, load) = match self {
            Shape::D1 => (
                "texture_1d<f32>",
                "vec3<f32>(1.0, 0.0, 0.0)",
                "2u",
                "textureLoad(t, c.x, level)",
            ),
            Shape::D2 => (
                "texture_2d<f32>",
                "vec3<f32>(1.0, 1.0, 0.0)",
                "4u",
                "textureLoad(t, c.xy, level)",
            ),
            Shape::D2Array => (
                "texture_2d_array<f32>",
                "vec3<f32>(1.0, 1.0, 0.0)",
                "4u",
                "textureLoad(t, c.xy, layer, level)",
            ),
            Shape::D3 => (
                "texture_3d<f32>",
                "vec3<f32>(1.0)",
                "8u",
                "textureLoad(t, c, level)",
            ),
            Shape::Depth2D => (
                "texture_depth_2d",
                "vec3<f32>(1.0, 1.0, 0.0)",
                "4u",
                "vec4<f32>(textureLoad(t, c.xy, level), 0.0, 0.0, 1.0)",
            ),
            Shape::Depth2DArray => (
                "texture_depth_2d_array",
                "vec3<f32>(1.0, 1.0, 0.0)",
                "4u",
                "vec4<f32>(textureLoad(t, c.xy, layer, level), 0.0, 0.0, 1.0)",
            ),
        };
        let (texture, load, decode) = match decoded {
            None => (String::from(texture), String::from(load), String::new()),
            Some(kind) => (
                texture.replace(
                    FLOAT_TEXELS,
                    match kind {
                        ScalarKind::Sint => SIGNED_TEXELS,
                        _ => UNSIGNED_TEXELS,
                    },
                ),
                DECODED_LOAD.replace(LOAD_PLACEHOLDER, load),
                String::from(DECODE_DECLARATION),
            ),
        };
        format!(
            "{decode}fn load_at(t: {texture}, c: vec3<i32>, layer: i32, level: i32) -> vec4<f32> {{ return {load}; }}\n{}",
            LIBRARY
                .replace("TEXTURE", &texture)
                .replace("AXES", axes)
                .replace("TAPS", taps)
        )
    }
}

/// Library functions imported into a module: parsed once per shape, each function copied with the
/// functions it calls into `ahead`, which [`Outlined::finish`] puts before every other function.
#[derive(Default)]
pub(super) struct Library {
    /// Each shape's library, by the guest format of a texture stored as integers.
    parsed: Vec<((Shape, Option<Format>), Module)>,
    pub(super) ahead: naga::Arena<Function>,
    /// Each imported function by its form in the target module, so equal copies are made once.
    imported: Vec<(String, Handle<Function>)>,
}

impl Library {
    /// The library function `name` of `shape`, imported into `module`, for a texture stored as
    /// integers where `decoded` gives the guest's format.
    fn function(
        &mut self,
        module: &mut Module,
        (shape, decoded): (Shape, Option<Format>),
        name: &str,
    ) -> Result<Handle<Function>> {
        let at = match self.parsed.iter().position(|(k, _)| *k == (shape, decoded)) {
            Some(at) => at,
            None => {
                let kind = decoded.map(super::decoded::stored_kind).transpose()?;
                let mut parsed =
                    naga::front::wgsl::parse_str(&shape.source(kind)).map_err(|e| {
                        ConvertError::Malformed(format!("the filter library does not parse: {e}"))
                    })?;
                if let Some(format) = decoded {
                    let (stub, vec4f) = parsed
                        .functions
                        .iter()
                        .find(|(_, f)| f.name.as_deref() == Some(DECODE_FUNCTION))
                        .and_then(|(h, f)| Some((h, f.result.as_ref()?.ty)))
                        .ok_or_else(|| ConvertError::Malformed(NO_DECODE_FUNCTION.into()))?;
                    parsed.functions[stub] =
                        super::decoded::texel_function(&parsed.functions[stub], vec4f, format)?;
                }
                self.parsed.push(((shape, decoded), parsed));
                self.parsed.len() - 1
            }
        };
        let library = core::mem::take(&mut self.parsed[at].1);
        let found = library
            .functions
            .iter()
            .find(|(_, f)| f.name.as_deref() == Some(name))
            .map(|(h, _)| h);
        let imported = match found {
            Some(f) => self.import(module, &library, f),
            None => Err(ConvertError::Malformed(format!(
                "the filter library has no {name}"
            ))),
        };
        self.parsed[at].1 = library;
        imported
    }

    pub(super) fn import(
        &mut self,
        module: &mut Module,
        library: &Module,
        f: Handle<Function>,
    ) -> Result<Handle<Function>> {
        let mut g = library.functions[f].clone();
        let mut calls = Vec::new();
        each_block(&mut g.body, &mut |b: &mut Block| {
            for s in b.iter() {
                if let Statement::Call { function, .. } = s {
                    calls.push(*function);
                }
            }
        });
        let mut map: Vec<(Handle<Function>, Handle<Function>)> = Vec::new();
        for c in calls {
            if !map.iter().any(|(from, _)| *from == c) {
                let to = self.import(module, library, c)?;
                map.push((c, to));
            }
        }
        let mapped = |h: Handle<Function>| map.iter().find(|(from, _)| *from == h).unwrap().1;
        each_block(&mut g.body, &mut |b: &mut Block| {
            for s in b.iter_mut() {
                if let Statement::Call { function, .. } = s {
                    *function = mapped(*function);
                }
            }
        });
        for a in &mut g.arguments {
            a.ty = copy_type(module, library, a.ty)?;
        }
        if let Some(r) = &mut g.result {
            r.ty = copy_type(module, library, r.ty)?;
        }
        for (_, l) in g.local_variables.iter_mut() {
            l.ty = copy_type(module, library, l.ty)?;
        }
        for (_, e) in g.expressions.iter_mut() {
            match e {
                Expression::CallResult(h) => *h = mapped(*h),
                Expression::ZeroValue(ty) | Expression::Compose { ty, .. } => {
                    *ty = copy_type(module, library, *ty)?
                }
                Expression::Constant(_)
                | Expression::Override(_)
                | Expression::GlobalVariable(_)
                | Expression::AtomicResult { .. }
                | Expression::WorkGroupUniformLoadResult { .. }
                | Expression::SubgroupOperationResult { .. } => {
                    return Err(ConvertError::Malformed(
                        "the filter library uses module-level objects".into(),
                    ));
                }
                _ => {}
            }
        }
        g.diagnostic_filter_leaf = None;
        let key = format!("{g:?}");
        if let Some((_, h)) = self.imported.iter().find(|(k, _)| *k == key) {
            return Ok(*h);
        }
        let h = self.ahead.append(g, Span::UNDEFINED);
        self.imported.push((key, h));
        Ok(h)
    }
}

/// Copies type `ty` of `library` into `module`.
fn copy_type(module: &mut Module, library: &Module, ty: Handle<Type>) -> Result<Handle<Type>> {
    let t = &library.types[ty];
    let inner = match &t.inner {
        TypeInner::Pointer { base, space } => TypeInner::Pointer {
            base: copy_type(module, library, *base)?,
            space: *space,
        },
        TypeInner::Array {
            base,
            size: size @ naga::ArraySize::Constant(_),
            stride,
        } => TypeInner::Array {
            base: copy_type(module, library, *base)?,
            size: *size,
            stride: *stride,
        },
        TypeInner::Array { .. } | TypeInner::Struct { .. } | TypeInner::BindingArray { .. } => {
            return Err(ConvertError::Malformed(
                "the filter library uses a type it cannot copy".into(),
            ));
        }
        other => other.clone(),
    };
    Ok(module.types.insert(
        Type {
            name: t.name.clone(),
            inner,
        },
        Span::UNDEFINED,
    ))
}

/// Where a filtered sample's values come from: the sampler's state, bias and border colour, and the
/// texture's resource clamp.
pub(super) struct Sources {
    /// The sampled texture's variable, which the callee reads directly so that back ends that answer
    /// size queries per bound view (WebGL 2) see which texture is asked.
    pub image: Handle<naga::GlobalVariable>,
    pub texture: Option<u32>,
    pub sampler: u32,
    /// The texture is in `filter_in_shader` (otherwise the sampler is in `border_in_shader`).
    pub by_texture: bool,
    pub state: HelperMember,
    pub bias: Option<HelperMember>,
    pub clamp: Option<HelperMember>,
    /// The guest's format of a texture stored as integers (`VariantKey::decoded`).
    pub decoded: Option<Format>,
}

/// Replaces sample `h` of function `owner` by a call into the library.
pub(super) fn sample(
    module: &mut Module,
    stage: ShaderStage,
    flags: VariantFlags,
    reflection: &mut Reflection,
    outlined: &mut Outlined,
    (owner, h): (usize, Handle<Expression>),
    sources: Sources,
) -> Result<()> {
    let f = function(module, owner).clone();
    let Expression::ImageSample {
        image,
        gather,
        coordinate,
        array_index,
        offset,
        level,
        depth_ref,
        ..
    } = f.expressions[h].clone()
    else {
        unreachable!()
    };
    let operand_types = types_of(module, &f, &[image])?;
    let depth = matches!(
        module.types[operand_types[0]].inner,
        TypeInner::Image {
            class: ImageClass::Depth { multi: false },
            ..
        }
    );
    if depth && sources.decoded.is_some() {
        return Err(ConvertError::Unsupported(crate::refusal::DECODED_TEXTURE));
    }
    if depth_ref.is_some() {
        return Err(ConvertError::Unsupported(if sources.by_texture {
            "FilterInShaderOnComparison"
        } else {
            "BorderInShaderOnComparison"
        }));
    }
    let mut operands = alloc::vec![image, coordinate];
    operands.extend(array_index);
    let next = operands.len();
    match level {
        SampleLevel::Exact(e) | SampleLevel::Bias(e) => operands.push(e),
        SampleLevel::Gradient { x, y } => operands.extend([x, y]),
        SampleLevel::Auto | SampleLevel::Zero => {}
    }
    operands.push(h);
    let types = types_of(module, &f, &operands)?;
    let shape = match module.types[types[0]].inner {
        TypeInner::Image {
            dim,
            arrayed,
            class,
        } => {
            match class {
                ImageClass::Sampled {
                    kind: ScalarKind::Float,
                    multi: false,
                } => {}
                ImageClass::Depth { multi: false } => {}
                ImageClass::Depth { .. } => {
                    return Err(ConvertError::Unsupported(
                        crate::refusal::FILTER_IN_SHADER_ON_DEPTH,
                    ));
                }
                _ => {
                    return Err(ConvertError::Unsupported(
                        crate::refusal::FILTER_IN_SHADER_ON_INTEGER,
                    ));
                }
            }
            match (dim, arrayed) {
                (naga::ImageDimension::D2, false) if depth => Shape::Depth2D,
                (naga::ImageDimension::D2, true) if depth => Shape::Depth2DArray,
                (naga::ImageDimension::D1, false) => Shape::D1,
                (naga::ImageDimension::D2, false) => Shape::D2,
                (naga::ImageDimension::D2, true) => Shape::D2Array,
                (naga::ImageDimension::D3, false) => Shape::D3,
                (naga::ImageDimension::Cube, _) => {
                    return Err(ConvertError::Unsupported(
                        crate::refusal::FILTER_IN_SHADER_ON_CUBE,
                    ));
                }
                _ => {
                    return Err(ConvertError::Unsupported(
                        crate::refusal::FILTER_IN_SHADER_DIMENSION,
                    ));
                }
            }
        }
        _ => return Err(ConvertError::Malformed("a sample of no image".into())),
    };
    let result = *types.last().unwrap();
    let mut callee = Function {
        name: Some(format!("filter_{}", sources.sampler)),
        result: Some(FunctionResult {
            ty: result,
            binding: None,
        }),
        ..Default::default()
    };
    let mut body = Block::new();
    let n = operands.len() - 1;
    // The texture is read through its variable; the other operands are arguments.
    let mut args = alloc::vec![push(
        &mut callee,
        &mut body,
        Expression::GlobalVariable(sources.image)
    )];
    for (i, &ty) in types[1..n].iter().enumerate() {
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
    let vec3f = vector(module, 3, Scalar::F32);
    let vec3u = vector(module, 3, Scalar::U32);
    let vec3i = vector(module, 3, Scalar::I32);
    // Coordinates and gradients as three components, zero past the texture's axes.
    let components = |ty: Handle<Type>| match module.types[ty].inner {
        TypeInner::Vector { size, .. } => size as usize,
        _ => 1,
    };
    let count = components(types[1]);
    let widen = |callee: &mut Function, body: &mut Block, e: Handle<Expression>, ty| {
        if count == 3 {
            return e;
        }
        let mut parts = alloc::vec![e];
        for _ in count..3 {
            let zero = match ty {
                t if t == vec3i => callee
                    .expressions
                    .append(Expression::Literal(Literal::I32(0)), Span::UNDEFINED),
                _ => f32_lit(callee, 0.0),
            };
            parts.push(zero);
        }
        push(
            callee,
            body,
            Expression::Compose {
                ty,
                components: parts,
            },
        )
    };
    let coordinate = widen(&mut callee, &mut body, args[1], vec3f);
    // Level 0's size, one past the texture's axes, and the layer clamped to the layers.
    let zero = callee
        .expressions
        .append(Expression::Literal(Literal::I32(0)), Span::UNDEFINED);
    let one = callee
        .expressions
        .append(Expression::Literal(Literal::U32(1)), Span::UNDEFINED);
    let query = |callee: &mut Function, body: &mut Block, query| {
        push(
            callee,
            body,
            Expression::ImageQuery {
                image: args[0],
                query,
            },
        )
    };
    let size = query(
        &mut callee,
        &mut body,
        ImageQuery::Size { level: Some(zero) },
    );
    let size = match shape {
        Shape::D3 => size,
        Shape::D1 => push(
            &mut callee,
            &mut body,
            Expression::Compose {
                ty: vec3u,
                components: alloc::vec![size, one, one],
            },
        ),
        Shape::D2 | Shape::D2Array | Shape::Depth2D | Shape::Depth2DArray => push(
            &mut callee,
            &mut body,
            Expression::Compose {
                ty: vec3u,
                components: alloc::vec![size, one],
            },
        ),
    };
    let layer = match array_index {
        Some(_) => {
            let a = args[next - 1];
            let a = match module.types[types[next - 1]].inner {
                TypeInner::Scalar(Scalar {
                    kind: ScalarKind::Uint,
                    ..
                }) => push(
                    &mut callee,
                    &mut body,
                    Expression::As {
                        expr: a,
                        kind: ScalarKind::Sint,
                        convert: None,
                    },
                ),
                _ => a,
            };
            let layers = query(&mut callee, &mut body, ImageQuery::NumLayers);
            let layers = push(
                &mut callee,
                &mut body,
                Expression::As {
                    expr: layers,
                    kind: ScalarKind::Sint,
                    convert: Some(4),
                },
            );
            let minus_one = callee
                .expressions
                .append(Expression::Literal(Literal::I32(-1)), Span::UNDEFINED);
            let last = super::binary(
                &mut callee,
                &mut body,
                naga::BinaryOperator::Add,
                layers,
                minus_one,
            );
            push(
                &mut callee,
                &mut body,
                Expression::Math {
                    fun: naga::MathFunction::Clamp,
                    arg: a,
                    arg1: Some(zero),
                    arg2: Some(last),
                    arg3: None,
                },
            )
        }
        None => zero,
    };
    let offset = match offset {
        Some(o) => {
            let o = copy_constant(&f, o, &mut callee);
            widen(&mut callee, &mut body, o, vec3i)
        }
        None => push(&mut callee, &mut body, Expression::ZeroValue(vec3i)),
    };
    let state = load_helper(&mut callee, &mut body, sources.state);
    let [s0, s1, s2] = [0, 1, 2].map(|i| index(&mut callee, &mut body, state, i));
    let vec4f = vector(module, 4, Scalar::F32);
    let border_at = helper_once(
        module,
        reflection,
        stage,
        HelperValue::BorderColor {
            sampler: sources.sampler,
        },
        vec4f,
    );
    let border = load_helper(&mut callee, &mut body, border_at);
    let u32_ty = scalar(module, Scalar::U32);
    let texture = sources
        .texture
        .ok_or(ConvertError::Unsupported(crate::refusal::TEXTURE_ARGUMENT))?;
    let components_at = helper_once(
        module,
        reflection,
        stage,
        HelperValue::TextureComponents { texture },
        u32_ty,
    );
    let components = load_helper(&mut callee, &mut body, components_at);
    let (function, arguments) = match gather {
        Some(component) => {
            let component = callee.expressions.append(
                Expression::Literal(Literal::U32(component as u32)),
                Span::UNDEFINED,
            );
            let function =
                outlined
                    .library
                    .function(module, (shape, sources.decoded), "filter_gather")?;
            (
                function,
                alloc::vec![
                    args[0], size, coordinate, component, offset, layer, s0, s1, border, components
                ],
            )
        }
        None => {
            let derivatives = |callee: &mut Function, body: &mut Block| {
                if stage != ShaderStage::Fragment {
                    return Err(ConvertError::Malformed(
                        "an implicit level outside a pixel shader".into(),
                    ));
                }
                Ok(
                    [naga::DerivativeAxis::X, naga::DerivativeAxis::Y].map(|axis| {
                        push(
                            callee,
                            body,
                            Expression::Derivative {
                                axis,
                                // Per quad, as the texture unit takes them.
                                ctrl: naga::DerivativeControl::Coarse,
                                expr: coordinate,
                            },
                        )
                    }),
                )
            };
            let zero = |callee: &mut Function, body: &mut Block| {
                push(callee, body, Expression::ZeroValue(vec3f))
            };
            let (dx, dy, explicit, mut lod) = match level {
                SampleLevel::Auto => {
                    let [dx, dy] = derivatives(&mut callee, &mut body)?;
                    (dx, dy, false, f32_lit(&mut callee, 0.0))
                }
                SampleLevel::Bias(_) => {
                    let [dx, dy] = derivatives(&mut callee, &mut body)?;
                    (dx, dy, false, args[next])
                }
                SampleLevel::Gradient { .. } => {
                    let dx = widen(&mut callee, &mut body, args[next], vec3f);
                    let dy = widen(&mut callee, &mut body, args[next + 1], vec3f);
                    (dx, dy, false, f32_lit(&mut callee, 0.0))
                }
                SampleLevel::Exact(_) => {
                    let z = zero(&mut callee, &mut body);
                    // A depth texture's level is an integer.
                    let lod = match module.types[types[next]].inner {
                        TypeInner::Scalar(Scalar {
                            kind: ScalarKind::Float,
                            ..
                        }) => args[next],
                        _ => push(
                            &mut callee,
                            &mut body,
                            Expression::As {
                                expr: args[next],
                                kind: ScalarKind::Float,
                                convert: Some(Scalar::F32.width),
                            },
                        ),
                    };
                    (z, z, true, lod)
                }
                SampleLevel::Zero => {
                    let z = zero(&mut callee, &mut body);
                    (z, z, true, f32_lit(&mut callee, 0.0))
                }
            };
            if let Some(bias_at) = sources.bias {
                let bias = load_helper(&mut callee, &mut body, bias_at);
                lod = super::binary(&mut callee, &mut body, naga::BinaryOperator::Add, lod, bias);
            }
            let explicit = callee.expressions.append(
                Expression::Literal(Literal::Bool(explicit)),
                Span::UNDEFINED,
            );
            let levels = if flags.contains(VariantFlags::TEXTURE_INFO) {
                let at = helper_once(
                    module,
                    reflection,
                    stage,
                    HelperValue::TextureLevels { texture },
                    u32_ty,
                );
                load_helper(&mut callee, &mut body, at)
            } else {
                push(
                    &mut callee,
                    &mut body,
                    Expression::ImageQuery {
                        image: args[0],
                        query: ImageQuery::NumLevels,
                    },
                )
            };
            let min_lod = match sources.clamp {
                Some(clamp_at) => {
                    let m = load_helper(&mut callee, &mut body, clamp_at);
                    let bits = index(&mut callee, &mut body, m, 0);
                    push(
                        &mut callee,
                        &mut body,
                        Expression::As {
                            expr: bits,
                            kind: ScalarKind::Float,
                            convert: None,
                        },
                    )
                }
                None => f32_lit(&mut callee, f32::MIN),
            };
            let function =
                outlined
                    .library
                    .function(module, (shape, sources.decoded), "filter_sample")?;
            (
                function,
                alloc::vec![
                    args[0], size, coordinate, dx, dy, explicit, lod, offset, layer, s0, s1, s2,
                    border, components, levels, min_lod
                ],
            )
        }
    };
    let mut value = callee
        .expressions
        .append(Expression::CallResult(function), Span::UNDEFINED);
    body.push(
        Statement::Call {
            function,
            arguments,
            result: Some(value),
        },
        Span::UNDEFINED,
    );
    // A depth sample gives the depth alone.
    if matches!(module.types[result].inner, TypeInner::Scalar(_)) {
        value = index(&mut callee, &mut body, value, 0);
    }
    if let Some(clamp_at) = sources.clamp {
        // A gather reads level 0 only, which a clamp of one or more leaves out.
        let read = gather.is_some().then(|| f32_lit(&mut callee, 0.0));
        value = super::min_lod_zero(&mut callee, &mut body, clamp_at, read, result, value);
    }
    body.push(Statement::Return { value: Some(value) }, Span::UNDEFINED);
    callee.body = body;
    outlined.add(owner, h, callee, operands[1..n].to_vec());
    Ok(())
}
