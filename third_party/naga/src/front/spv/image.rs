use alloc::vec::Vec;

use crate::{
    arena::{Handle, UniqueArena},
    Scalar,
};

use super::{Error, LookupExpression, LookupHelper as _};

/// The words of `OpImageQueryLod`: the opcode, the result's type and id, the sampled image and the
/// coordinate.
pub(super) const IMAGE_QUERY_LOD_WORDS: u16 = 5;

/// A stand-in function's name, the types of its arguments and the type of its result.
pub(super) type StandInSignature = (&'static str, Vec<Handle<crate::Type>>, Handle<crate::Type>);

#[derive(Clone, Debug)]
pub(super) struct LookupSampledImage {
    image: Handle<crate::Expression>,
    sampler: Handle<crate::Expression>,
}

bitflags::bitflags! {
    /// Flags describing sampling method.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct SamplingFlags: u32 {
        /// Regular sampling.
        const REGULAR = 0x1;
        /// Comparison sampling.
        const COMPARISON = 0x2;
    }
}

impl super::BlockContext<'_> {
    pub(super) fn get_image_expr_ty(
        &self,
        handle: Handle<crate::Expression>,
    ) -> Result<Handle<crate::Type>, Error> {
        match self.expressions[handle] {
            crate::Expression::GlobalVariable(handle) => {
                Ok(self.module.global_variables[handle].ty)
            }
            crate::Expression::FunctionArgument(i) => Ok(self.arguments[i as usize].ty),
            // An element of an array of images has the array's element type.
            crate::Expression::Access { base, .. }
            | crate::Expression::AccessIndex { base, .. } => {
                let ty = self.get_image_expr_ty(base)?;
                match self.module.types[ty].inner {
                    crate::TypeInner::BindingArray { base, .. } => Ok(base),
                    _ => Ok(ty),
                }
            }
            ref other => Err(Error::InvalidImageExpression(other.clone())),
        }
    }
}

/// Options of a sampling operation.
#[derive(Debug)]
pub struct SamplingOptions {
    /// Projection sampling: the division by W is expected to happen
    /// in the texture unit.
    pub project: bool,
    /// Depth comparison sampling with a reference value.
    pub compare: bool,
    /// Gather sampling: Operates on four samples of one channel.
    pub gather: bool,
}

enum ExtraCoordinate {
    ArrayLayer,
    Projection,
    Garbage,
}

/// Return the texture coordinates separated from the array layer,
/// and/or divided by the projection term.
///
/// The Proj sampling ops expect an extra coordinate for the W.
/// The arrayed (can't be Proj!) images expect an extra coordinate for the layer.
fn extract_image_coordinates(
    image_dim: crate::ImageDimension,
    extra_coordinate: ExtraCoordinate,
    base: Handle<crate::Expression>,
    coordinate_ty: Handle<crate::Type>,
    ctx: &mut super::BlockContext,
) -> (Handle<crate::Expression>, Option<Handle<crate::Expression>>) {
    let (given_size, kind) = match ctx.module.types[coordinate_ty].inner {
        crate::TypeInner::Scalar(Scalar { kind, .. }) => (None, kind),
        crate::TypeInner::Vector {
            size,
            scalar: Scalar { kind, .. },
        } => (Some(size), kind),
        ref other => unreachable!("Unexpected texture coordinate {:?}", other),
    };

    let required_size = image_dim.required_coordinate_size();
    // `parse_type_image` makes the float vector; integer coordinates (texel loads) need their own.
    let required_ty = required_size.map(|size| {
        ctx.module.types.insert(
            crate::Type {
                name: None,
                inner: crate::TypeInner::Vector {
                    size,
                    scalar: Scalar { kind, width: 4 },
                },
            },
            crate::Span::UNDEFINED,
        )
    });
    let extra_expr = crate::Expression::AccessIndex {
        base,
        index: required_size.map_or(1, |size| size as u32),
    };

    let base_span = ctx.expressions.get_span(base);

    match extra_coordinate {
        ExtraCoordinate::ArrayLayer => {
            let extracted = match required_size {
                None => ctx
                    .expressions
                    .append(crate::Expression::AccessIndex { base, index: 0 }, base_span),
                Some(size) => {
                    let mut components = Vec::with_capacity(size as usize);
                    for index in 0..size as u32 {
                        let comp = ctx
                            .expressions
                            .append(crate::Expression::AccessIndex { base, index }, base_span);
                        components.push(comp);
                    }
                    ctx.expressions.append(
                        crate::Expression::Compose {
                            ty: required_ty.unwrap(),
                            components,
                        },
                        base_span,
                    )
                }
            };
            let array_index_f32 = ctx.expressions.append(extra_expr, base_span);
            let array_index = ctx.expressions.append(
                crate::Expression::As {
                    kind: crate::ScalarKind::Sint,
                    expr: array_index_f32,
                    convert: Some(4),
                },
                base_span,
            );
            (extracted, Some(array_index))
        }
        ExtraCoordinate::Projection => {
            let projection = ctx.expressions.append(extra_expr, base_span);
            let divided = match required_size {
                None => {
                    let temp = ctx
                        .expressions
                        .append(crate::Expression::AccessIndex { base, index: 0 }, base_span);
                    ctx.expressions.append(
                        crate::Expression::Binary {
                            op: crate::BinaryOperator::Divide,
                            left: temp,
                            right: projection,
                        },
                        base_span,
                    )
                }
                Some(size) => {
                    let mut components = Vec::with_capacity(size as usize);
                    for index in 0..size as u32 {
                        let temp = ctx
                            .expressions
                            .append(crate::Expression::AccessIndex { base, index }, base_span);
                        let comp = ctx.expressions.append(
                            crate::Expression::Binary {
                                op: crate::BinaryOperator::Divide,
                                left: temp,
                                right: projection,
                            },
                            base_span,
                        );
                        components.push(comp);
                    }
                    ctx.expressions.append(
                        crate::Expression::Compose {
                            ty: required_ty.unwrap(),
                            components,
                        },
                        base_span,
                    )
                }
            };
            (divided, None)
        }
        ExtraCoordinate::Garbage if given_size == required_size => (base, None),
        ExtraCoordinate::Garbage => {
            use crate::SwizzleComponent as Sc;
            let cut_expr = match required_size {
                None => crate::Expression::AccessIndex { base, index: 0 },
                Some(size) => crate::Expression::Swizzle {
                    size,
                    vector: base,
                    pattern: [Sc::X, Sc::Y, Sc::Z, Sc::W],
                },
            };
            (ctx.expressions.append(cut_expr, base_span), None)
        }
    }
}

pub(super) fn patch_comparison_type(
    flags: SamplingFlags,
    var: &mut crate::GlobalVariable,
    arena: &mut UniqueArena<crate::Type>,
) -> bool {
    if !flags.contains(SamplingFlags::COMPARISON) {
        return true;
    }
    // An image both compared against and sampled plainly becomes a depth image too; its plain
    // reads then give one value where the module expects four, which the caller must widen.
    let sampler = matches!(arena[var.ty].inner, crate::TypeInner::Sampler { .. });
    if flags == SamplingFlags::all() && sampler {
        return false;
    }

    log::debug!("Flipping comparison for {var:?}");
    let original_ty = &arena[var.ty];
    let original_ty_span = arena.get_span(var.ty);
    let ty_inner = match original_ty.inner {
        crate::TypeInner::Image {
            class: crate::ImageClass::Sampled { multi, .. },
            dim,
            arrayed,
        } => crate::TypeInner::Image {
            class: crate::ImageClass::Depth { multi },
            dim,
            arrayed,
        },
        crate::TypeInner::Sampler { .. } => crate::TypeInner::Sampler { comparison: true },
        ref other => unreachable!("Unexpected type for comparison mutation: {:?}", other),
    };

    let name = original_ty.name.clone();
    var.ty = arena.insert(
        crate::Type {
            name,
            inner: ty_inner,
        },
        original_ty_span,
    );
    true
}

impl<I: Iterator<Item = u32>> super::Frontend<I> {
    pub(super) fn parse_image_couple(&mut self) -> Result<(), Error> {
        let _result_type_id = self.next()?;
        let result_id = self.next()?;
        let image_id = self.next()?;
        let sampler_id = self.next()?;
        let image_lexp = self.lookup_expression.lookup(image_id)?;
        let sampler_lexp = self.lookup_expression.lookup(sampler_id)?;
        self.lookup_sampled_image.insert(
            result_id,
            LookupSampledImage {
                image: image_lexp.handle,
                sampler: sampler_lexp.handle,
            },
        );
        Ok(())
    }

    pub(super) fn parse_image_uncouple(&mut self, block_id: spirv::Word) -> Result<(), Error> {
        let result_type_id = self.next()?;
        let result_id = self.next()?;
        let sampled_image_id = self.next()?;
        self.lookup_expression.insert(
            result_id,
            LookupExpression {
                handle: self.lookup_sampled_image.lookup(sampled_image_id)?.image,
                type_id: result_type_id,
                block_id,
            },
        );
        Ok(())
    }

    pub(super) fn parse_image_write(
        &mut self,
        words_left: u16,
        ctx: &mut super::BlockContext,
        emitter: &mut crate::proc::Emitter,
        block: &mut crate::Block,
        body_idx: usize,
    ) -> Result<crate::Statement, Error> {
        let image_id = self.next()?;
        let coordinate_id = self.next()?;
        let value_id = self.next()?;

        let image_ops = if words_left != 0 { self.next()? } else { 0 };

        // Hints, non-private access and availability at workgroup scope, which every storage access
        // of the IR has; operands that change the texel's meaning or its visibility to other
        // invocations are refused.
        let kept = spirv::ImageOperands::NONTEMPORAL
            | spirv::ImageOperands::NON_PRIVATE_TEXEL
            | spirv::ImageOperands::MAKE_TEXEL_AVAILABLE;
        let ops = spirv::ImageOperands::from_bits_retain(image_ops);
        if !kept.contains(ops) {
            return Err(Error::UnsupportedImageWriteOperand(
                ops.difference(kept).bits(),
            ));
        }
        if ops.contains(spirv::ImageOperands::MAKE_TEXEL_AVAILABLE) {
            self.workgroup_scope(ctx.module, image_ops)?;
        }

        let image_lexp = self.lookup_expression.lookup(image_id)?;
        let image_ty = ctx.get_image_expr_ty(image_lexp.handle)?;

        let coord_lexp = self.lookup_expression.lookup(coordinate_id)?;
        let coord_handle =
            self.get_expr_handle(coordinate_id, coord_lexp, ctx, emitter, block, body_idx);
        let coord_type_handle = self.lookup_type.lookup(coord_lexp.type_id)?.handle;
        let (coordinate, array_index) = match ctx.module.types[image_ty].inner {
            crate::TypeInner::Image {
                dim,
                arrayed,
                class: _,
            } => extract_image_coordinates(
                dim,
                if arrayed {
                    ExtraCoordinate::ArrayLayer
                } else {
                    ExtraCoordinate::Garbage
                },
                coord_handle,
                coord_type_handle,
                ctx,
            ),
            _ => return Err(Error::InvalidImage(image_ty)),
        };

        let value_lexp = self.lookup_expression.lookup(value_id)?;
        let value = self.get_expr_handle(value_id, value_lexp, ctx, emitter, block, body_idx);
        let value_type = self.lookup_type.lookup(value_lexp.type_id)?.handle;

        // In hlsl etc, the write value may not be the vector 4.
        let expanded_value = match ctx.module.types[value_type].inner {
            crate::TypeInner::Scalar(_) => Some(crate::Expression::Splat {
                value,
                size: crate::VectorSize::Quad,
            }),
            crate::TypeInner::Vector { size, .. } => match size {
                crate::VectorSize::Bi => Some(crate::Expression::Swizzle {
                    size: crate::VectorSize::Quad,
                    vector: value,
                    pattern: [
                        crate::SwizzleComponent::X,
                        crate::SwizzleComponent::Y,
                        crate::SwizzleComponent::Y,
                        crate::SwizzleComponent::Y,
                    ],
                }),
                crate::VectorSize::Tri => Some(crate::Expression::Swizzle {
                    size: crate::VectorSize::Quad,
                    vector: value,
                    pattern: [
                        crate::SwizzleComponent::X,
                        crate::SwizzleComponent::Y,
                        crate::SwizzleComponent::Z,
                        crate::SwizzleComponent::Z,
                    ],
                }),
                crate::VectorSize::Quad => None,
            },
            _ => return Err(Error::InvalidVectorType(value_type)),
        };

        let value_patched = if let Some(s) = expanded_value {
            ctx.expressions.append(s, crate::Span::default())
        } else {
            value
        };

        Ok(crate::Statement::ImageStore {
            image: image_lexp.handle,
            coordinate,
            array_index,
            value: value_patched,
        })
    }

    pub(super) fn parse_image_load(
        &mut self,
        words_left: u16,
        ctx: &mut super::BlockContext,
        emitter: &mut crate::proc::Emitter,
        block: &mut crate::Block,
        block_id: spirv::Word,
        body_idx: usize,
    ) -> Result<(), Error> {
        let start = self.data_offset;
        let result_type_id = self.next()?;
        let result_id = self.next()?;
        let image_id = self.next()?;
        let coordinate_id = self.next()?;

        let mut image_ops = if words_left != 0 { self.next()? } else { 0 };

        let mut sample = None;
        let mut level = None;
        let mut offset = None;
        while image_ops != 0 {
            let bit = 1 << image_ops.trailing_zeros();
            match spirv::ImageOperands::from_bits_truncate(bit) {
                spirv::ImageOperands::LOD => {
                    let lod_expr = self.next()?;
                    let lod_lexp = self.lookup_expression.lookup(lod_expr)?;
                    let lod_handle =
                        self.get_expr_handle(lod_expr, lod_lexp, ctx, emitter, block, body_idx);
                    level = Some(lod_handle);
                }
                spirv::ImageOperands::SAMPLE => {
                    let sample_expr = self.next()?;
                    let sample_handle = self.lookup_expression.lookup(sample_expr)?.handle;
                    sample = Some(sample_handle);
                }
                spirv::ImageOperands::OFFSET | spirv::ImageOperands::CONST_OFFSET => {
                    let offset_expr = self.next()?;
                    let offset_lexp = self.lookup_expression.lookup(offset_expr)?;
                    let offset_handle = self.get_expr_handle(
                        offset_expr,
                        offset_lexp,
                        ctx,
                        emitter,
                        block,
                        body_idx,
                    );
                    offset = Some((offset_handle, offset_lexp.type_id));
                }
                // Every storage access of the IR is non-private in the Vulkan memory model's
                // sense, and visible at workgroup scope.
                spirv::ImageOperands::NONTEMPORAL | spirv::ImageOperands::NON_PRIVATE_TEXEL => {}
                spirv::ImageOperands::MAKE_TEXEL_VISIBLE => {
                    self.workgroup_scope(ctx.module, bit)?;
                }
                other => {
                    return Err(Error::UnsupportedImageLoadOperand(other.bits()));
                }
            }
            image_ops ^= bit;
        }

        // No need to call get_expr_handle here since only globals/arguments are
        // allowed as images and they are always in the root scope
        let image_lexp = self.lookup_expression.lookup(image_id)?;
        let image_ty = ctx.get_image_expr_ty(image_lexp.handle)?;

        let coord_lexp = self.lookup_expression.lookup(coordinate_id)?;
        let coord_handle =
            self.get_expr_handle(coordinate_id, coord_lexp, ctx, emitter, block, body_idx);
        let coord_type_handle = self.lookup_type.lookup(coord_lexp.type_id)?.handle;
        let (coordinate, array_index, is_depth) = match ctx.module.types[image_ty].inner {
            crate::TypeInner::Image {
                dim,
                arrayed,
                class,
            } => {
                let (coord, array_index) = extract_image_coordinates(
                    dim,
                    if arrayed {
                        ExtraCoordinate::ArrayLayer
                    } else {
                        ExtraCoordinate::Garbage
                    },
                    coord_handle,
                    coord_type_handle,
                    ctx,
                );
                (coord, array_index, class.is_depth())
            }
            _ => return Err(Error::InvalidImage(image_ty)),
        };
        // A load's texel offset is added to its coordinate (the IR's load has no offset).
        let coordinate = match offset {
            Some((offset, offset_type_id)) => {
                let span = self.span_from_with_op(start);
                let offset_kind = self.int_kind(ctx, offset_type_id)?;
                let coord_kind = self.int_kind(ctx, coord_lexp.type_id)?;
                let offset = Self::retype_int(ctx, offset, offset_kind, coord_kind, span);
                ctx.expressions.append(
                    crate::Expression::Binary {
                        op: crate::BinaryOperator::Add,
                        left: coordinate,
                        right: offset,
                    },
                    span,
                )
            }
            None => coordinate,
        };

        let image_load_expr = crate::Expression::ImageLoad {
            image: image_lexp.handle,
            coordinate,
            array_index,
            sample,
            level,
        };
        let image_load_handle = ctx
            .expressions
            .append(image_load_expr, self.span_from_with_op(start));

        let handle = if is_depth {
            let result_ty = self.lookup_type.lookup(result_type_id)?;
            // The return type of `OpImageRead` can be a scalar or vector.
            match ctx.module.types[result_ty.handle].inner {
                crate::TypeInner::Vector { size, .. } => {
                    let splat_expr = crate::Expression::Splat {
                        size,
                        value: image_load_handle,
                    };
                    ctx.expressions
                        .append(splat_expr, self.span_from_with_op(start))
                }
                _ => image_load_handle,
            }
        } else {
            image_load_handle
        };

        self.lookup_expression.insert(
            result_id,
            LookupExpression {
                handle,
                type_id: result_type_id,
                block_id,
            },
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn parse_image_sample(
        &mut self,
        words_left: u16,
        options: SamplingOptions,
        ctx: &mut super::BlockContext,
        emitter: &mut crate::proc::Emitter,
        block: &mut crate::Block,
        block_id: spirv::Word,
        body_idx: usize,
    ) -> Result<(), Error> {
        let start = self.data_offset;
        let result_type_id = self.next()?;
        let result_id = self.next()?;
        let sampled_image_id = self.next()?;
        let coordinate_id = self.next()?;
        let (component_id, dref_id) = match (options.gather, options.compare) {
            (true, false) => (Some(self.next()?), None),
            (_, true) => (None, Some(self.next()?)),
            (_, _) => (None, None),
        };
        let span = self.span_from_with_op(start);

        let mut image_ops = if words_left != 0 { self.next()? } else { 0 };

        let mut level = crate::SampleLevel::Auto;
        // The types of the level's operands, and the minimum level with its type.
        let mut level_types = Vec::new();
        let mut min_lod = None;
        let mut offset = None;
        let mut moved = None;
        while image_ops != 0 {
            let bit = 1 << image_ops.trailing_zeros();
            match spirv::ImageOperands::from_bits_truncate(bit) {
                spirv::ImageOperands::BIAS => {
                    let bias_expr = self.next()?;
                    let bias_lexp = self.lookup_expression.lookup(bias_expr)?;
                    let bias_handle =
                        self.get_expr_handle(bias_expr, bias_lexp, ctx, emitter, block, body_idx);
                    level = crate::SampleLevel::Bias(bias_handle);
                    level_types = alloc::vec![self.lookup_type.lookup(bias_lexp.type_id)?.handle];
                }
                spirv::ImageOperands::LOD => {
                    let lod_expr = self.next()?;
                    let lod_lexp = self.lookup_expression.lookup(lod_expr)?;
                    let lod_handle =
                        self.get_expr_handle(lod_expr, lod_lexp, ctx, emitter, block, body_idx);

                    let is_depth_image = {
                        let image_lexp = self.lookup_sampled_image.lookup(sampled_image_id)?;
                        let image_ty = ctx.get_image_expr_ty(image_lexp.image)?;
                        matches!(
                            ctx.module.types[image_ty].inner,
                            crate::TypeInner::Image {
                                class: crate::ImageClass::Depth { .. },
                                ..
                            }
                        )
                    };

                    level = if options.compare {
                        log::debug!("Assuming {lod_handle:?} is zero");
                        crate::SampleLevel::Zero
                    } else if is_depth_image {
                        log::debug!(
                            "Assuming level {lod_handle:?} converts losslessly to an integer"
                        );
                        let expr = crate::Expression::As {
                            expr: lod_handle,
                            kind: crate::ScalarKind::Sint,
                            convert: Some(4),
                        };
                        let s32_lod_handle = ctx.expressions.append(expr, span);
                        crate::SampleLevel::Exact(s32_lod_handle)
                    } else {
                        crate::SampleLevel::Exact(lod_handle)
                    };
                }
                spirv::ImageOperands::GRAD => {
                    let grad_x_expr = self.next()?;
                    let grad_x_lexp = self.lookup_expression.lookup(grad_x_expr)?;
                    let grad_x_handle = self.get_expr_handle(
                        grad_x_expr,
                        grad_x_lexp,
                        ctx,
                        emitter,
                        block,
                        body_idx,
                    );
                    let grad_ty = self.lookup_type.lookup(grad_x_lexp.type_id)?.handle;
                    level_types = alloc::vec![grad_ty, grad_ty];
                    let grad_y_expr = self.next()?;
                    let grad_y_lexp = self.lookup_expression.lookup(grad_y_expr)?;
                    let grad_y_handle = self.get_expr_handle(
                        grad_y_expr,
                        grad_y_lexp,
                        ctx,
                        emitter,
                        block,
                        body_idx,
                    );
                    level = if options.compare {
                        log::debug!(
                            "Assuming gradients {grad_x_handle:?} and {grad_y_handle:?} are not greater than 1"
                        );
                        crate::SampleLevel::Zero
                    } else {
                        crate::SampleLevel::Gradient {
                            x: grad_x_handle,
                            y: grad_y_handle,
                        }
                    };
                }
                spirv::ImageOperands::CONST_OFFSET => {
                    let offset_expr = self.next()?;
                    let offset_lexp = self.lookup_expression.lookup(offset_expr)?;
                    let offset_handle = self.get_expr_handle(
                        offset_expr,
                        offset_lexp,
                        ctx,
                        emitter,
                        block,
                        body_idx,
                    );
                    offset = Some(offset_handle);
                }
                // A gather picks its four texels from level 0, so a run-time offset moves the
                // coordinate by that many texels of level 0 (the IR's offset is a constant).
                spirv::ImageOperands::OFFSET if options.gather => {
                    let offset_expr = self.next()?;
                    let offset_lexp = self.lookup_expression.lookup(offset_expr)?;
                    let offset_handle = self.get_expr_handle(
                        offset_expr,
                        offset_lexp,
                        ctx,
                        emitter,
                        block,
                        body_idx,
                    );
                    moved = Some((offset_handle, offset_lexp.type_id));
                }
                // A comparison takes no gradients, through which the minimum level is taken.
                spirv::ImageOperands::MIN_LOD if !options.compare && !options.gather => {
                    let clamp_expr = self.next()?;
                    let clamp_lexp = self.lookup_expression.lookup(clamp_expr)?;
                    let clamp_handle =
                        self.get_expr_handle(clamp_expr, clamp_lexp, ctx, emitter, block, body_idx);
                    let clamp_ty = self.lookup_type.lookup(clamp_lexp.type_id)?.handle;
                    min_lod = Some((clamp_handle, clamp_ty));
                }
                // Skipping an operand (a run-time `Offset` among them) would change what is sampled.
                other => return Err(Error::UnsupportedImageSampleOperand(other.bits())),
            }
            image_ops ^= bit;
        }

        let si_lexp = self.lookup_sampled_image.lookup(sampled_image_id)?.clone();
        let coord_lexp = self.lookup_expression.lookup(coordinate_id)?;
        let coord_handle =
            self.get_expr_handle(coordinate_id, coord_lexp, ctx, emitter, block, body_idx);
        let coord_type_handle = self.lookup_type.lookup(coord_lexp.type_id)?.handle;

        let gather = match (options.gather, component_id) {
            (true, Some(component_id)) => {
                let component_lexp = self.lookup_expression.lookup(component_id)?;

                let component_value = match ctx.expressions[component_lexp.handle] {
                    // VUID-StandaloneSpirv-OpImageGather-04664:
                    // The “Component” operand of OpImageGather, and OpImageSparseGather must be the
                    // <id> of a constant instruction.
                    crate::Expression::Constant(const_handle) => {
                        let constant = &ctx.module.constants[const_handle];
                        match ctx.module.global_expressions[constant.init] {
                            // SPIR-V specification: "It must be a 32-bit integer type scalar."
                            crate::Expression::Literal(crate::Literal::U32(value)) => value,
                            crate::Expression::Literal(crate::Literal::I32(value)) => value as u32,
                            _ => {
                                log::error!(
                                    "Image gather component constant must be a 32-bit integer literal"
                                );
                                return Err(Error::InvalidOperand);
                            }
                        }
                    }
                    _ => {
                        log::error!("Image gather component must be a constant");
                        return Err(Error::InvalidOperand);
                    }
                };

                debug_assert_eq!(level, crate::SampleLevel::Auto);
                level = crate::SampleLevel::Zero;

                // SPIR-V specification: "Behavior is undefined if its value is not 0, 1, 2 or 3."
                match component_value {
                    0 => Some(crate::SwizzleComponent::X),
                    1 => Some(crate::SwizzleComponent::Y),
                    2 => Some(crate::SwizzleComponent::Z),
                    3 => Some(crate::SwizzleComponent::W),
                    other => {
                        log::error!("Invalid gather component operand: {other}");
                        return Err(Error::InvalidOperand);
                    }
                }
            }
            (true, None) => {
                debug_assert_eq!(level, crate::SampleLevel::Auto);
                level = crate::SampleLevel::Zero;

                Some(crate::SwizzleComponent::X)
            }
            (_, _) => None,
        };

        let sampling_bit = if options.compare {
            SamplingFlags::COMPARISON
        } else {
            SamplingFlags::REGULAR
        };

        let image_ty = match ctx.expressions[si_lexp.image] {
            crate::Expression::GlobalVariable(handle) => {
                if let Some(flags) = self.handle_sampling.get_mut(&handle) {
                    *flags |= sampling_bit;
                }

                ctx.module.global_variables[handle].ty
            }

            crate::Expression::FunctionArgument(i) => {
                ctx.parameter_sampling[i as usize] |= sampling_bit;
                ctx.arguments[i as usize].ty
            }

            crate::Expression::Access { base, .. } => match ctx.expressions[base] {
                crate::Expression::GlobalVariable(handle) => {
                    if let Some(flags) = self.handle_sampling.get_mut(&handle) {
                        *flags |= sampling_bit;
                    }

                    match ctx.module.types[ctx.module.global_variables[handle].ty].inner {
                        crate::TypeInner::BindingArray { base, .. } => base,
                        _ => return Err(Error::InvalidGlobalVar(ctx.expressions[base].clone())),
                    }
                }

                ref other => return Err(Error::InvalidGlobalVar(other.clone())),
            },

            ref other => return Err(Error::InvalidGlobalVar(other.clone())),
        };

        match ctx.expressions[si_lexp.sampler] {
            crate::Expression::GlobalVariable(handle) => {
                *self.handle_sampling.get_mut(&handle).unwrap() |= sampling_bit;
            }

            crate::Expression::FunctionArgument(i) => {
                ctx.parameter_sampling[i as usize] |= sampling_bit;
            }

            crate::Expression::Access { base, .. } => match ctx.expressions[base] {
                crate::Expression::GlobalVariable(handle) => {
                    *self.handle_sampling.get_mut(&handle).unwrap() |= sampling_bit;
                }

                ref other => return Err(Error::InvalidGlobalVar(other.clone())),
            },

            ref other => return Err(Error::InvalidGlobalVar(other.clone())),
        }

        let ((coordinate, array_index), depth_ref, is_depth, dim) =
            match ctx.module.types[image_ty].inner {
                crate::TypeInner::Image {
                    dim,
                    arrayed,
                    class,
                } => (
                    extract_image_coordinates(
                        dim,
                        if options.project {
                            ExtraCoordinate::Projection
                        } else if arrayed {
                            ExtraCoordinate::ArrayLayer
                        } else {
                            ExtraCoordinate::Garbage
                        },
                        coord_handle,
                        coord_type_handle,
                        ctx,
                    ),
                    {
                        match dref_id {
                            Some(id) => {
                                let expr_lexp = self.lookup_expression.lookup(id)?;
                                let mut expr = self
                                    .get_expr_handle(id, expr_lexp, ctx, emitter, block, body_idx);

                                if options.project {
                                    let required_size = dim.required_coordinate_size();
                                    let right = ctx.expressions.append(
                                        crate::Expression::AccessIndex {
                                            base: coord_handle,
                                            index: required_size.map_or(1, |size| size as u32),
                                        },
                                        crate::Span::default(),
                                    );
                                    expr = ctx.expressions.append(
                                        crate::Expression::Binary {
                                            op: crate::BinaryOperator::Divide,
                                            left: expr,
                                            right,
                                        },
                                        crate::Span::default(),
                                    )
                                };
                                Some(expr)
                            }
                            None => None,
                        }
                    },
                    class.is_depth(),
                    dim,
                ),
                _ => return Err(Error::InvalidImage(image_ty)),
            };

        if let Some(clamp) = min_lod {
            level = self.sample_clamp_gradients(
                ctx,
                emitter,
                block,
                si_lexp.image,
                si_lexp.sampler,
                coordinate,
                dim,
                (level, level_types),
                clamp,
                span,
            )?;
        }

        let coordinate = match moved {
            Some((offset, offset_type_id)) => self.gather_offset_coordinate(
                ctx,
                emitter,
                block,
                si_lexp.image,
                coordinate,
                offset,
                offset_type_id,
                span,
            )?,
            None => coordinate,
        };

        let expr = crate::Expression::ImageSample {
            image: si_lexp.image,
            sampler: si_lexp.sampler,
            gather,
            coordinate,
            array_index,
            offset,
            level,
            depth_ref,
            clamp_to_edge: false,
        };
        let image_sample_handle = ctx.expressions.append(expr, self.span_from_with_op(start));
        let handle = if is_depth && depth_ref.is_none() {
            let splat_expr = crate::Expression::Splat {
                size: crate::VectorSize::Quad,
                value: image_sample_handle,
            };
            ctx.expressions
                .append(splat_expr, self.span_from_with_op(start))
        } else {
            image_sample_handle
        };
        self.lookup_expression.insert(
            result_id,
            LookupExpression {
                handle,
                type_id: result_type_id,
                block_id,
            },
        );
        Ok(())
    }

    /// The coordinate moved by a gather's run-time offset in texels of level 0. Direct3D takes each
    /// component's low six bits, signed.
    #[allow(clippy::too_many_arguments)]
    fn gather_offset_coordinate(
        &self,
        ctx: &mut super::BlockContext,
        emitter: &mut crate::proc::Emitter,
        block: &mut crate::Block,
        image: Handle<crate::Expression>,
        coordinate: Handle<crate::Expression>,
        offset: Handle<crate::Expression>,
        offset_type_id: spirv::Word,
        span: crate::Span,
    ) -> Result<Handle<crate::Expression>, Error> {
        use crate::{BinaryOperator as B, Expression as E};
        let kind = self.int_kind(ctx, offset_type_id)?;
        let offset = Self::retype_int(ctx, offset, kind, Some(crate::ScalarKind::Sint), span);
        let size = match ctx.module.types[ctx.get_image_expr_ty(image)?].inner {
            crate::TypeInner::Image { dim, .. } => match dim {
                crate::ImageDimension::D2 => crate::VectorSize::Bi,
                _ => return Err(Error::UnsupportedImageSampleOperand(0x10)),
            },
            _ => return Err(Error::InvalidOperand),
        };
        // Literals are never emitted.
        block.extend(emitter.finish(ctx.expressions));
        let six = ctx
            .expressions
            .append(E::Literal(crate::Literal::U32(26)), span);
        let zero = ctx
            .expressions
            .append(E::Literal(crate::Literal::U32(0)), span);
        emitter.start(ctx.expressions);
        let mut add = |e: E| ctx.expressions.append(e, span);
        let six = add(E::Splat { size, value: six });
        let o = add(E::Binary {
            op: B::ShiftLeft,
            left: offset,
            right: six,
        });
        let o = add(E::Binary {
            op: B::ShiftRight,
            left: o,
            right: six,
        });
        let o = add(E::As {
            expr: o,
            kind: crate::ScalarKind::Float,
            convert: Some(4),
        });
        let texels = add(E::ImageQuery {
            image,
            query: crate::ImageQuery::Size { level: Some(zero) },
        });
        let texels = add(E::As {
            expr: texels,
            kind: crate::ScalarKind::Float,
            convert: Some(4),
        });
        let shift = add(E::Binary {
            op: B::Divide,
            left: o,
            right: texels,
        });
        Ok(add(E::Binary {
            op: B::Add,
            left: coordinate,
            right: shift,
        }))
    }

    pub(super) fn parse_image_query_size(
        &mut self,
        at_level: bool,
        ctx: &mut super::BlockContext,
        emitter: &mut crate::proc::Emitter,
        block: &mut crate::Block,
        block_id: spirv::Word,
        body_idx: usize,
    ) -> Result<(), Error> {
        let start = self.data_offset;
        let result_type_id = self.next()?;
        let result_id = self.next()?;
        let image_id = self.next()?;
        let level = if at_level {
            let level_id = self.next()?;
            let level_lexp = self.lookup_expression.lookup(level_id)?;
            Some(self.get_expr_handle(level_id, level_lexp, ctx, emitter, block, body_idx))
        } else {
            None
        };

        // No need to call get_expr_handle here since only globals/arguments are
        // allowed as images and they are always in the root scope
        //TODO: handle arrays and cubes
        let image_lexp = self.lookup_expression.lookup(image_id)?;

        let expr = crate::Expression::ImageQuery {
            image: image_lexp.handle,
            query: crate::ImageQuery::Size { level },
        };
        // SPIR-V gives an array's layer count as the size's last component.
        let image_ty = ctx.get_image_expr_ty(image_lexp.handle)?;
        let expr = match ctx.module.types[image_ty].inner {
            crate::TypeInner::Image {
                dim, arrayed: true, ..
            } => {
                let span = self.span_from_with_op(start);
                let size = ctx.expressions.append(expr, span);
                let layers = ctx.expressions.append(
                    crate::Expression::ImageQuery {
                        image: image_lexp.handle,
                        query: crate::ImageQuery::NumLayers,
                    },
                    span,
                );
                let mut components = match dim {
                    crate::ImageDimension::D1 => alloc::vec![size],
                    _ => (0..2)
                        .map(|index| {
                            ctx.expressions
                                .append(crate::Expression::AccessIndex { base: size, index }, span)
                        })
                        .collect(),
                };
                components.push(layers);
                let size = match components.len() {
                    2 => crate::VectorSize::Bi,
                    3 => crate::VectorSize::Tri,
                    _ => crate::VectorSize::Quad,
                };
                let ty = ctx.module.types.insert(
                    crate::Type {
                        name: None,
                        inner: crate::TypeInner::Vector {
                            size,
                            scalar: Scalar::U32,
                        },
                    },
                    span,
                );
                crate::Expression::Compose { ty, components }
            }
            _ => expr,
        };

        let result_type_handle = self.lookup_type.lookup(result_type_id)?.handle;
        let maybe_scalar_kind = ctx.module.types[result_type_handle].inner.scalar_kind();

        let expr = if maybe_scalar_kind == Some(crate::ScalarKind::Sint) {
            crate::Expression::As {
                expr: ctx.expressions.append(expr, self.span_from_with_op(start)),
                kind: crate::ScalarKind::Sint,
                convert: Some(4),
            }
        } else {
            expr
        };

        self.lookup_expression.insert(
            result_id,
            LookupExpression {
                handle: ctx.expressions.append(expr, self.span_from_with_op(start)),
                type_id: result_type_id,
                block_id,
            },
        );

        Ok(())
    }

    pub(super) fn parse_image_query_other(
        &mut self,
        query: crate::ImageQuery,
        ctx: &mut super::BlockContext,
        block_id: spirv::Word,
    ) -> Result<(), Error> {
        let start = self.data_offset;
        let result_type_id = self.next()?;
        let result_id = self.next()?;
        let image_id = self.next()?;

        // No need to call get_expr_handle here since only globals/arguments are
        // allowed as images and they are always in the root scope
        let image_lexp = self.lookup_expression.lookup(image_id)?.clone();

        let expr = crate::Expression::ImageQuery {
            image: image_lexp.handle,
            query,
        };

        let result_type_handle = self.lookup_type.lookup(result_type_id)?.handle;
        let maybe_scalar_kind = ctx.module.types[result_type_handle].inner.scalar_kind();

        let expr = if maybe_scalar_kind == Some(crate::ScalarKind::Sint) {
            crate::Expression::As {
                expr: ctx.expressions.append(expr, self.span_from_with_op(start)),
                kind: crate::ScalarKind::Sint,
                convert: Some(4),
            }
        } else {
            expr
        };

        self.lookup_expression.insert(
            result_id,
            LookupExpression {
                handle: ctx.expressions.append(expr, self.span_from_with_op(start)),
                type_id: result_type_id,
                block_id,
            },
        );

        Ok(())
    }

    /// `OpImageQueryLod`, which the IR lacks: a call of the `IMAGE_QUERY_LOD` function of its
    /// signature, made on first use, which the module's user replaces.
    pub(super) fn parse_image_query_lod(
        &mut self,
        ctx: &mut super::BlockContext,
        emitter: &mut crate::proc::Emitter,
        block: &mut crate::Block,
        block_id: spirv::Word,
        body_idx: usize,
    ) -> Result<(), Error> {
        let start = self.data_offset;
        let result_type_id = self.next()?;
        let result_id = self.next()?;
        let sampled_image_id = self.next()?;
        let coordinate_id = self.next()?;
        let span = self.span_from_with_op(start);
        let si = self.lookup_sampled_image.lookup(sampled_image_id)?.clone();
        let coord_lexp = self.lookup_expression.lookup(coordinate_id)?;
        let coordinate =
            self.get_expr_handle(coordinate_id, coord_lexp, ctx, emitter, block, body_idx);
        let signature = (
            super::IMAGE_QUERY_LOD,
            alloc::vec![
                ctx.get_image_expr_ty(si.image)?,
                ctx.get_image_expr_ty(si.sampler)?,
                self.lookup_type.lookup(coord_lexp.type_id)?.handle,
            ],
            self.lookup_type.lookup(result_type_id)?.handle,
        );
        let to = self.stand_in(ctx, signature, span)?;
        block.extend(emitter.finish(ctx.expressions));
        let function = self.add_call(ctx.function_id, to);
        let result = ctx
            .expressions
            .append(crate::Expression::CallResult(function), span);
        self.lookup_expression.insert(
            result_id,
            LookupExpression {
                handle: result,
                type_id: result_type_id,
                block_id,
            },
        );
        block.push(
            crate::Statement::Call {
                function,
                arguments: alloc::vec![si.image, si.sampler, coordinate],
                result: Some(result),
            },
            span,
        );
        emitter.start(ctx.expressions);
        Ok(())
    }

    /// The id of the stand-in function of `signature`, made on first use: its body is empty, and the
    /// module's user replaces each call of it.
    pub(super) fn stand_in(
        &mut self,
        ctx: &mut super::BlockContext,
        signature: StandInSignature,
        span: crate::Span,
    ) -> Result<spirv::Word, Error> {
        if let Some(&(_, id)) = self.stand_ins.iter().find(|(s, _)| *s == signature) {
            return Ok(id);
        }
        let id = self
            .id_bound
            .checked_add(self.stand_ins.len() as u32)
            .ok_or(Error::InvalidId(self.id_bound))?;
        let (name, ref arguments, result) = signature;
        let function = crate::Function {
            name: Some(name.into()),
            arguments: arguments
                .iter()
                .map(|&ty| crate::FunctionArgument {
                    name: None,
                    ty,
                    binding: None,
                })
                .collect(),
            result: Some(crate::FunctionResult {
                ty: result,
                binding: None,
            }),
            ..Default::default()
        };
        let handle = ctx.module.functions.append(function, span);
        self.lookup_function.insert(
            id,
            super::LookupFunction {
                handle,
                parameters_sampling: Vec::new(),
            },
        );
        self.stand_ins.push((signature, id));
        Ok(id)
    }

    /// The gradients a sample with a minimum level (`MinLod`, which the IR lacks) takes: the two
    /// columns, cut to the coordinate's size, of a call of the `IMAGE_SAMPLE_CLAMP` function of its
    /// signature, made on first use, which the module's user replaces.
    #[allow(clippy::too_many_arguments)]
    fn sample_clamp_gradients(
        &mut self,
        ctx: &mut super::BlockContext,
        emitter: &mut crate::proc::Emitter,
        block: &mut crate::Block,
        image: Handle<crate::Expression>,
        sampler: Handle<crate::Expression>,
        coordinate: Handle<crate::Expression>,
        dim: crate::ImageDimension,
        level: (crate::SampleLevel, Vec<Handle<crate::Type>>),
        clamp: (Handle<crate::Expression>, Handle<crate::Type>),
        span: crate::Span,
    ) -> Result<crate::SampleLevel, Error> {
        let f32_scalar = Scalar::F32;
        let size = dim.required_coordinate_size();
        let coordinate_ty = ctx.module.types.insert(
            crate::Type {
                name: None,
                inner: match size {
                    Some(size) => crate::TypeInner::Vector {
                        size,
                        scalar: f32_scalar,
                    },
                    None => crate::TypeInner::Scalar(f32_scalar),
                },
            },
            span,
        );
        let result_ty = ctx.module.types.insert(
            crate::Type {
                name: None,
                inner: crate::TypeInner::Matrix {
                    columns: crate::VectorSize::Bi,
                    rows: crate::VectorSize::Tri,
                    scalar: f32_scalar,
                },
            },
            span,
        );
        let mut arguments = alloc::vec![image, sampler, coordinate];
        let mut types = alloc::vec![
            ctx.get_image_expr_ty(image)?,
            ctx.get_image_expr_ty(sampler)?,
            coordinate_ty
        ];
        match level.0 {
            crate::SampleLevel::Auto => {}
            crate::SampleLevel::Bias(bias) => arguments.push(bias),
            crate::SampleLevel::Gradient { x, y } => arguments.extend([x, y]),
            _ => {
                return Err(Error::UnsupportedImageSampleOperand(
                    spirv::ImageOperands::MIN_LOD.bits(),
                ))
            }
        }
        types.extend(level.1);
        arguments.push(clamp.0);
        types.push(clamp.1);
        let to = self.stand_in(ctx, (super::IMAGE_SAMPLE_CLAMP, types, result_ty), span)?;
        block.extend(emitter.finish(ctx.expressions));
        let function = self.add_call(ctx.function_id, to);
        let result = ctx
            .expressions
            .append(crate::Expression::CallResult(function), span);
        block.push(
            crate::Statement::Call {
                function,
                arguments,
                result: Some(result),
            },
            span,
        );
        emitter.start(ctx.expressions);
        let [x, y] = [0, 1].map(|index| {
            let column = ctx.expressions.append(
                crate::Expression::AccessIndex {
                    base: result,
                    index,
                },
                span,
            );
            match size {
                Some(crate::VectorSize::Tri) => column,
                Some(size) => ctx.expressions.append(
                    crate::Expression::Swizzle {
                        size,
                        vector: column,
                        pattern: crate::SwizzleComponent::XYZW,
                    },
                    span,
                ),
                None => ctx.expressions.append(
                    crate::Expression::AccessIndex {
                        base: column,
                        index: 0,
                    },
                    span,
                ),
            }
        });
        Ok(crate::SampleLevel::Gradient { x, y })
    }
}
