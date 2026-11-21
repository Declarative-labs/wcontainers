//! Counting programs for counted occlusion queries (the interface's occlusion counter): the pixel
//! shader's program writing 1.0 at location 0 in place of its colours, so a target blended One plus
//! One sums the samples that passed. Its discards, depth and sample mask outputs are kept, and its
//! alpha at location 0 stays in the fourth component for alpha to coverage.

use naga::{
    AddressSpace, Binding, Block, Expression, Handle, ImageClass, Literal, Scalar, Span, Statement,
    StorageAccess, StructMember, Type, TypeInner, VectorSize,
};
use wcontainers_gpu::{ShaderModule, ShaderStage};

use crate::error::{Result, not_implemented};

/// The fragment program of a draw without a pixel shader.
pub fn without_pixel_shader() -> Result<ShaderModule> {
    ShaderModule::from_wgsl(
        "@fragment fn main() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }",
        "main",
        ShaderStage::Fragment,
    )
    .or_else(|_| not_implemented("the counting program of a draw without a pixel shader"))
}

/// What each return of the rewritten entry point builds.
struct Plan {
    /// The new result: a struct of the counting colour, then the outputs kept.
    ty: Handle<Type>,
    color: Handle<Type>,
    /// Member indices of the old result's alpha at location 0, if it has a float one.
    alpha: Option<Vec<u32>>,
    /// The old result's built-in outputs: member indices, or the whole value.
    kept: Vec<Option<u32>>,
}

/// `pixel`'s program rewritten to count. A program writing storage is not served: the counting
/// draw would write it again.
pub fn from_pixel_shader(pixel: &ShaderModule) -> Result<ShaderModule> {
    let mut module = pixel.module.clone();
    let writes = module.global_variables.iter().any(|(_, g)| {
        let image_store = matches!(
            module.types[g.ty].inner,
            TypeInner::Image { class: ImageClass::Storage { access, .. }, .. }
                if access.contains(StorageAccess::STORE)
        );
        image_store
            || matches!(g.space, AddressSpace::Storage { access } if access.contains(StorageAccess::STORE))
    });
    if writes {
        return not_implemented("counted occlusion of pixel shaders writing storage");
    }
    let Some(index) = module
        .entry_points
        .iter()
        .position(|e| e.name == pixel.entry_point && e.stage == naga::ShaderStage::Fragment)
    else {
        return not_implemented("counted occlusion of a pixel shader without its entry point");
    };
    let color = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Vector {
                size: VectorSize::Quad,
                scalar: Scalar::F32,
            },
        },
        Span::UNDEFINED,
    );
    let is_color = |ty: Handle<Type>| ty == color;
    let location0 = |b: &Option<Binding>| matches!(b, Some(Binding::Location { location: 0, .. }));
    let old = module.entry_points[index].function.result.clone();
    // The built-in outputs kept, with their types, and where the old alpha is.
    let mut outputs = Vec::new();
    let mut alpha = None;
    match &old {
        None => {}
        Some(r) => match (&module.types[r.ty].inner, &r.binding) {
            (TypeInner::Struct { members, .. }, None) => {
                for (i, m) in members.iter().enumerate() {
                    if let Some(Binding::BuiltIn(b)) = m.binding {
                        outputs.push((Some(i as u32), m.ty, b));
                    } else if location0(&m.binding) && is_color(m.ty) {
                        alpha = Some(vec![i as u32, 3]);
                    }
                }
            }
            (_, Some(Binding::BuiltIn(b))) => outputs.push((None, r.ty, *b)),
            (_, b) if location0(b) && is_color(r.ty) => alpha = Some(vec![3]),
            _ => {}
        },
    }
    let mut members = vec![StructMember {
        name: Some("count".into()),
        ty: color,
        binding: Some(Binding::Location {
            location: 0,
            interpolation: None,
            sampling: None,
            blend_src: None,
            per_primitive: false,
        }),
        offset: 0,
    }];
    let mut offset = 16;
    for (i, &(_, ty, b)) in outputs.iter().enumerate() {
        if !matches!(module.types[ty].inner, TypeInner::Scalar(_)) {
            return not_implemented(
                "counted occlusion of a pixel shader with a vector built-in output",
            );
        }
        members.push(StructMember {
            name: Some(format!("kept{i}")),
            ty,
            binding: Some(Binding::BuiltIn(b)),
            offset,
        });
        offset += 4;
    }
    let ty = module.types.insert(
        Type {
            name: Some("Counting".into()),
            inner: TypeInner::Struct {
                members,
                span: offset.div_ceil(16) * 16,
            },
        },
        Span::UNDEFINED,
    );
    let plan = Plan {
        ty,
        color,
        alpha,
        kept: outputs.iter().map(|o| o.0).collect(),
    };
    let f = &mut module.entry_points[index].function;
    f.result = Some(naga::FunctionResult { ty, binding: None });
    let mut body = std::mem::take(&mut f.body);
    rewrite(&mut body, &mut f.expressions, &plan);
    if !matches!(
        body.last(),
        Some(Statement::Return { .. } | Statement::Kill)
    ) {
        body.extend_block(returning(None, &mut f.expressions, &plan));
    }
    f.body = body;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module);
    let Ok(info) = info else {
        return not_implemented(
            "counted occlusion of a pixel shader whose counting program does not validate",
        );
    };
    let key = *blake3::Hasher::new()
        .update(&pixel.key)
        .update(b"counting")
        .finalize()
        .as_bytes();
    Ok(ShaderModule {
        module,
        info,
        entry_point: pixel.entry_point.clone(),
        stage: ShaderStage::Fragment,
        key,
    })
}

/// Replaces every return in `block` with one of the counting result.
fn rewrite(block: &mut Block, exprs: &mut naga::Arena<Expression>, plan: &Plan) {
    let old = std::mem::take(block);
    for (mut s, span) in old.span_into_iter() {
        match &mut s {
            Statement::Return { value } => {
                block.extend_block(returning(*value, exprs, plan));
                continue;
            }
            Statement::Block(b) => rewrite(b, exprs, plan),
            Statement::If { accept, reject, .. } => {
                rewrite(accept, exprs, plan);
                rewrite(reject, exprs, plan);
            }
            Statement::Loop {
                body, continuing, ..
            } => {
                rewrite(body, exprs, plan);
                rewrite(continuing, exprs, plan);
            }
            Statement::Switch { cases, .. } => {
                for c in cases {
                    rewrite(&mut c.body, exprs, plan);
                }
            }
            _ => {}
        }
        block.push(s, span);
    }
}

/// The statements returning the counting result built from the old return value `value`.
fn returning(
    value: Option<Handle<Expression>>,
    exprs: &mut naga::Arena<Expression>,
    plan: &Plan,
) -> Block {
    let at = Span::UNDEFINED;
    let one = exprs.append(Expression::Literal(Literal::F32(1.0)), at);
    let first = exprs.len();
    let alpha = match (&plan.alpha, value) {
        (Some(path), Some(v)) => path.iter().fold(v, |base, &index| {
            exprs.append(Expression::AccessIndex { base, index }, at)
        }),
        _ => one,
    };
    let color = exprs.append(
        Expression::Compose {
            ty: plan.color,
            components: vec![one, one, one, alpha],
        },
        at,
    );
    let mut components = vec![color];
    for &k in &plan.kept {
        let v = value.expect("a result with built-in outputs is returned");
        components.push(match k {
            Some(index) => exprs.append(Expression::AccessIndex { base: v, index }, at),
            None => v,
        });
    }
    let result = exprs.append(
        Expression::Compose {
            ty: plan.ty,
            components,
        },
        at,
    );
    let mut block = Block::new();
    block.push(Statement::Emit(exprs.range_from(first)), at);
    block.push(
        Statement::Return {
            value: Some(result),
        },
        at,
    );
    block
}
