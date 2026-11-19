//! Typed buffers read and written through storage buffers of words (`TexelBufferRoute::Storage`),
//! each element decoded from and encoded into the bound view's format as Direct3D converts (plan
//! 4.12), or read through a 2D texture of the view's format (`TexelBufferRoute::Texture`). Both keep
//! to the view's place and length from [`HelperValue::TexelBufferView`]: elements at or past the
//! length read zero (the defaults in components the format lacks) and take no writes, and atomic
//! operations on them do nothing and return zero. Elements narrower than a word are written by
//! compare-exchange on their word, so neighbours written at the same time are kept.

use alloc::vec;
use naga::{AtomicFunction, BinaryOperator as B, ScalarKind};
use wcontainers_gpu::{Format, TEXEL_BUFFER_TEXTURE_WIDTH, VertexFormat};

use super::pull::{Component, Kind, decode, layout, select};
use super::*;
use crate::build::{self, BYTE_SHIFT, Builder, Expr, VEC4, WORD, WORD_SHIFT};

/// The vertex format of the same bytes, whose layout and decoding a typed buffer element shares.
pub(crate) fn vertex_format(format: Format) -> Option<VertexFormat> {
    use Format as F;
    use VertexFormat as V;
    Some(match format {
        F::R8Unorm => V::Unorm8x1,
        F::R8Snorm => V::Snorm8x1,
        F::R8Uint => V::Uint8x1,
        F::R8Sint => V::Sint8x1,
        F::R16Unorm => V::Unorm16x1,
        F::R16Snorm => V::Snorm16x1,
        F::R16Uint => V::Uint16x1,
        F::R16Sint => V::Sint16x1,
        F::R16Float => V::Float16x1,
        F::Rg8Unorm => V::Unorm8x2,
        F::Rg8Snorm => V::Snorm8x2,
        F::Rg8Uint => V::Uint8x2,
        F::Rg8Sint => V::Sint8x2,
        F::R32Uint => V::Uint32x1,
        F::R32Sint => V::Sint32x1,
        F::R32Float => V::Float32x1,
        F::Rg16Unorm => V::Unorm16x2,
        F::Rg16Snorm => V::Snorm16x2,
        F::Rg16Uint => V::Uint16x2,
        F::Rg16Sint => V::Sint16x2,
        F::Rg16Float => V::Float16x2,
        F::Rgba8Unorm => V::Unorm8x4,
        F::Rgba8Snorm => V::Snorm8x4,
        F::Rgba8Uint => V::Uint8x4,
        F::Rgba8Sint => V::Sint8x4,
        F::Bgra8Unorm => V::Bgra8Unorm,
        F::Rgb10a2Unorm => V::Unorm10_10_10_2,
        F::Rgb10a2Uint => V::Uint10_10_10_2,
        F::Rg11b10Float => V::Float11_11_10,
        F::Rg32Uint => V::Uint32x2,
        F::Rg32Sint => V::Sint32x2,
        F::Rg32Float => V::Float32x2,
        F::Rgba16Unorm => V::Unorm16x4,
        F::Rgba16Snorm => V::Snorm16x4,
        F::Rgba16Uint => V::Uint16x4,
        F::Rgba16Sint => V::Sint16x4,
        F::Rgba16Float => V::Float16x4,
        F::Rgb32Uint => V::Uint32x3,
        F::Rgb32Sint => V::Sint32x3,
        F::Rgb32Float => V::Float32x3,
        F::Rgba32Uint => V::Uint32x4,
        F::Rgba32Sint => V::Sint32x4,
        F::Rgba32Float => V::Float32x4,
        _ => return None,
    })
}

/// The component an absent alpha reads as one in.
const ALPHA: u32 = VEC4 - 1;

/// Where a view's elements are.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum TexelPlace {
    /// A typed buffer's words, its view in [`HelperValue::TexelBufferView`].
    Words,
    /// A 2D texture of the format (`TexelBufferRoute::Texture`), the view as for `Words`.
    Texture,
    /// A storage texture's texels in a buffer of words (`VariantKey::texels_through_buffer`), laid
    /// out as [`HelperValue::TexelLayout`] of this storage slot says.
    Image(u32),
}

/// A format's kind, components and element size.
pub(crate) type Layout = (Kind, Vec<Component>, u32);

/// One typed buffer: its words, the bound view's format, and the functions made for it so far.
pub(crate) struct TexelBuffer {
    var: Handle<GlobalVariable>,
    name: String,
    kind: Kind,
    components: Vec<Component>,
    size: u32,
    /// The scalar of atomic words, where the words are atomic.
    atomic: Option<Scalar>,
    place: TexelPlace,
    stage: ShaderStage,
    set: u32,
    binding: u32,
    load: Option<Handle<Function>>,
    store: Option<Handle<Function>>,
    length: Option<Handle<Function>>,
    at: Option<Handle<Function>>,
    size_of: Option<Handle<Function>>,
    atomics: Vec<(u64, Handle<Function>)>,
}

impl TexelBuffer {
    /// The bound view's format as the declaration (of scalar kind `declared`) reads it.
    pub(crate) fn layout(format: Format, declared: ScalarKind) -> Result<Layout> {
        let v =
            vertex_format(format).ok_or(ConvertError::Unsupported("TypedBufferFormatAbsent"))?;
        let (kind, components, size) = layout(v);
        if natural(kind).kind != declared {
            return Err(ConvertError::Unsupported("TypedBufferFormatKind"));
        }
        Ok((kind, components, size))
    }

    /// The words' type: atomic where an atomic operation uses them (of `atomic`) or where elements
    /// narrower than a word are written.
    pub(crate) fn words(module: &mut Module, atomic: Option<Scalar>) -> Handle<Type> {
        let base = match atomic {
            Some(s) => module.types.insert(
                Type {
                    name: None,
                    inner: TypeInner::Atomic(s),
                },
                Span::UNDEFINED,
            ),
            None => scalar(module, Scalar::U32),
        };
        build::runtime_array(module, base, WORD)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn declared(
        var: Handle<GlobalVariable>,
        (kind, components, size): Layout,
        atomic: Option<Scalar>,
        place: TexelPlace,
        stage: ShaderStage,
        set: u32,
        binding: u32,
    ) -> TexelBuffer {
        TexelBuffer {
            var,
            name: format!("texels_{set}_{binding}"),
            kind,
            components,
            size,
            atomic,
            place,
            stage,
            set,
            binding,
            load: None,
            store: None,
            length: None,
            at: None,
            size_of: None,
            atomics: Vec::new(),
        }
    }

    /// The scalar loads give and stores take.
    pub(crate) fn scalar(&self) -> Scalar {
        natural(self.kind)
    }

    /// Whether elements are narrower than a word, so writes need atomic words.
    pub(crate) fn narrow(size: u32) -> bool {
        !size.is_multiple_of(WORD)
    }

    /// `(offset, length)` of the view, read from the helper constants.
    fn view(
        &self,
        module: &mut Module,
        reflection: &mut Reflection,
        b: &mut Builder,
    ) -> (Expr, Expr) {
        if let TexelPlace::Image(_) = self.place {
            // The texels from the buffer's start; as many as the layout holds.
            let [pitch, rows, _, _, layers] = self.texel_layout(module, reflection, b);
            let n = b.mul(pitch, rows);
            let n = b.mul(n, layers);
            let zero = b.u32(0);
            return (zero, n);
        }
        let vec2u = vector(module, 2, Scalar::U32);
        let member = helper_once(
            module,
            reflection,
            self.stage,
            HelperValue::TexelBufferView {
                set: self.set,
                binding: self.binding,
            },
            vec2u,
        );
        let v = b.raw(|f, body| load_helper(f, body, member));
        (b.field(v, 0), b.field(v, 1))
    }

    /// A storage texture's layout: row pitch in texels, rows per layer, width, height, and layers or
    /// depth.
    fn texel_layout(
        &self,
        module: &mut Module,
        reflection: &mut Reflection,
        b: &mut Builder,
    ) -> [Expr; 5] {
        let TexelPlace::Image(slot) = self.place else {
            unreachable!("a layout belongs to a storage texture");
        };
        let vec4u = build::vec4(module, Scalar::U32);
        let stride = module.types[vec4u].inner.size(module.to_ctx());
        let two = module.types.insert(
            Type {
                name: None,
                inner: TypeInner::Array {
                    base: vec4u,
                    size: naga::ArraySize::Constant(core::num::NonZeroU32::new(2).unwrap()),
                    stride,
                },
            },
            Span::UNDEFINED,
        );
        let member = helper_once(
            module,
            reflection,
            self.stage,
            HelperValue::TexelLayout { slot },
            two,
        );
        let v = b.raw(|f, body| load_helper(f, body, member));
        let first = b.field(v, 0);
        let second = b.field(v, 1);
        [
            b.field(first, 0),
            b.field(first, 1),
            b.field(first, 2),
            b.field(first, 3),
            b.field(second, 0),
        ]
    }

    /// `texels_<set>_<binding>_at(x, y, z) -> u32`: the texel's index in the buffer, or past every
    /// index where the coordinate is outside the view (z is the layer or the depth).
    pub(crate) fn at(
        &mut self,
        module: &mut Module,
        reflection: &mut Reflection,
    ) -> Handle<Function> {
        if let Some(f) = self.at {
            return f;
        }
        let u32_ty = scalar(module, Scalar::U32);
        let mut b = Builder::new(&format!("{}_at", self.name));
        for name in ["x", "y", "z"] {
            b.f.arguments.push(FunctionArgument {
                name: Some(name.into()),
                ty: u32_ty,
                binding: None,
            });
        }
        b.f.result = Some(FunctionResult {
            ty: u32_ty,
            binding: None,
        });
        let [x, y, z] = [0, 1, 2].map(|i| b.argument(i));
        let [pitch, rows, width, height, layers] = self.texel_layout(module, reflection, &mut b);
        let in_x = b.binary(B::Less, x, width);
        let in_y = b.binary(B::Less, y, height);
        let in_z = b.binary(B::Less, z, layers);
        let inside = b.binary(B::LogicalAnd, in_x, in_y);
        let inside = b.binary(B::LogicalAnd, inside, in_z);
        let row = b.mul(z, rows);
        let row = b.add(row, y);
        let at = b.mul(row, pitch);
        let at = b.add(at, x);
        let none = b.u32(u32::MAX);
        let index = b.select(inside, at, none);
        b.ret(Some(index));
        let f = module.functions.append(b.finish(), Span::UNDEFINED);
        self.at = Some(f);
        f
    }

    /// `texels_<set>_<binding>_size() -> vec3<u32>`: a storage texture's width, height, and layers or
    /// depth.
    pub(crate) fn size(
        &mut self,
        module: &mut Module,
        reflection: &mut Reflection,
    ) -> Handle<Function> {
        if let Some(f) = self.size_of {
            return f;
        }
        let vec3u = vector(module, naga::VectorSize::Tri as u8, Scalar::U32);
        let mut b = Builder::new(&format!("{}_size", self.name));
        b.f.result = Some(FunctionResult {
            ty: vec3u,
            binding: None,
        });
        let [_, _, width, height, layers] = self.texel_layout(module, reflection, &mut b);
        let v = b.compose(vec3u, vec![width, height, layers]);
        b.ret(Some(v));
        let f = module.functions.append(b.finish(), Span::UNDEFINED);
        self.size_of = Some(f);
        f
    }

    /// A word as u32, from a pointer to it.
    fn word(&self, b: &mut Builder, p: Expr) -> Expr {
        let w = b.load(p);
        match self.atomic {
            Some(s) if s.kind != ScalarKind::Uint => b.bitcast(w, ScalarKind::Uint),
            _ => w,
        }
    }

    /// `texels_<set>_<binding>_load(index) -> vec4` of the format's scalar.
    pub(crate) fn load(
        &mut self,
        module: &mut Module,
        reflection: &mut Reflection,
    ) -> Handle<Function> {
        if let Some(f) = self.load {
            return f;
        }
        let natural = self.scalar();
        let out = build::vec4(module, natural);
        let u32_ty = scalar(module, Scalar::U32);
        let mut b = Builder::new(&format!("{}_load", self.name));
        b.f.arguments.push(FunctionArgument {
            name: Some("index".into()),
            ty: u32_ty,
            binding: None,
        });
        b.f.result = Some(FunctionResult {
            ty: out,
            binding: None,
        });
        let index = b.argument(0);
        let (offset, length) = self.view(module, reflection, &mut b);
        let inside = b.binary(B::Less, index, length);
        let size = b.u32(self.size);
        if self.place == TexelPlace::Texture {
            let first = b.binary(B::Divide, offset, size);
            let element = b.add(first, index);
            let mask = b.u32(TEXEL_BUFFER_TEXTURE_WIDTH - 1);
            let x = b.binary(B::And, element, mask);
            let by = b.u32(TEXEL_BUFFER_TEXTURE_WIDTH.trailing_zeros());
            let y = b.binary(B::ShiftRight, element, by);
            let vec2u = vector(module, 2, Scalar::U32);
            let coordinate = b.compose(vec2u, vec![x, y]);
            let image = b.global(self.var);
            let level = b.u32(0);
            let read = b.expr(Expression::ImageLoad {
                image,
                coordinate,
                array_index: None,
                sample: None,
                level: Some(level),
            });
            let zero = self.past_length(&mut b, out, natural);
            let value = b.select(inside, read, zero);
            b.ret(Some(value));
            let f = module.functions.append(b.finish(), Span::UNDEFINED);
            self.load = Some(f);
            return f;
        }
        let moved = b.mul(index, size);
        let address = b.add(offset, moved);
        let words = b.global(self.var);
        let mut values = Vec::new();
        for &(byte, shift, bits) in &self.components {
            let at = b.u32(byte);
            let a = b.add(address, at);
            let by = b.u32(WORD_SHIFT);
            let i = b.binary(B::ShiftRight, a, by);
            let p = b.at(words, i);
            let mut raw = self.word(&mut b, p);
            if bits < u32::BITS {
                let byte_mask = b.u32(WORD - 1);
                let in_word = b.binary(B::And, a, byte_mask);
                let to_bits = b.u32(BYTE_SHIFT);
                let bit = b.binary(B::ShiftLeft, in_word, to_bits);
                let field = b.u32(shift);
                let from = b.add(bit, field);
                let shifted = b.binary(B::ShiftRight, raw, from);
                let mask = b.u32(((1u64 << bits) - 1) as u32);
                raw = b.binary(B::And, shifted, mask);
            }
            let kind = self.kind;
            values.push(b.raw(|f, body| decode(f, body, kind, raw, bits)));
        }
        for i in values.len() as u32..VEC4 {
            values.push(b.expr(Expression::Literal(literal(natural, (i == ALPHA) as u32))));
        }
        let read = b.compose(out, values);
        let zero = self.past_length(&mut b, out, natural);
        let value = b.select(inside, read, zero);
        b.ret(Some(value));
        let f = module.functions.append(b.finish(), Span::UNDEFINED);
        self.load = Some(f);
        f
    }

    /// What an element past the length reads: zero in the format's components, the defaults in the
    /// others.
    fn past_length(&self, b: &mut Builder, ty: Handle<Type>, natural: Scalar) -> Expr {
        let zeros = (0..VEC4)
            .map(|i| {
                let missing = i as usize >= self.components.len();
                b.expr(Expression::Literal(literal(
                    natural,
                    (missing && i == ALPHA) as u32,
                )))
            })
            .collect();
        b.compose(ty, zeros)
    }

    /// `texels_<set>_<binding>_length() -> u32`, the view's element count.
    pub(crate) fn length(
        &mut self,
        module: &mut Module,
        reflection: &mut Reflection,
    ) -> Handle<Function> {
        if let Some(f) = self.length {
            return f;
        }
        let u32_ty = scalar(module, Scalar::U32);
        let mut b = Builder::new(&format!("{}_length", self.name));
        b.f.result = Some(FunctionResult {
            ty: u32_ty,
            binding: None,
        });
        let (_, length) = self.view(module, reflection, &mut b);
        b.ret(Some(length));
        let f = module.functions.append(b.finish(), Span::UNDEFINED);
        self.length = Some(f);
        f
    }

    /// `texels_<set>_<binding>_store(index, value: vec4)` of the format's scalar.
    pub(crate) fn store(
        &mut self,
        module: &mut Module,
        reflection: &mut Reflection,
    ) -> Handle<Function> {
        if let Some(f) = self.store {
            return f;
        }
        let natural = self.scalar();
        let value_ty = build::vec4(module, natural);
        let u32_ty = scalar(module, Scalar::U32);
        let mut b = Builder::new(&format!("{}_store", self.name));
        for (name, ty) in [("index", u32_ty), ("value", value_ty)] {
            b.f.arguments.push(FunctionArgument {
                name: Some(name.into()),
                ty,
                binding: None,
            });
        }
        let index = b.argument(0);
        let value = b.argument(1);
        let (offset, length) = self.view(module, reflection, &mut b);
        let inside = b.binary(B::Less, index, length);
        b.open();
        let size = b.u32(self.size);
        let moved = b.mul(index, size);
        let address = b.add(offset, moved);
        let by = b.u32(WORD_SHIFT);
        let first = b.binary(B::ShiftRight, address, by);
        let words = b.global(self.var);
        // Each word's bits, from the components lying in it.
        let count = self.size.div_ceil(WORD);
        let mut parts: Vec<Option<Expr>> = vec![None; count as usize];
        for (c, &(byte, shift, bits)) in self.components.iter().enumerate() {
            let v = b.field(value, c as u32);
            let kind = self.kind;
            let mut raw = b.raw(|f, body| encode(f, body, kind, v, bits));
            let at = ((byte % WORD) << BYTE_SHIFT) + shift;
            if at != 0 {
                let by = b.u32(at);
                raw = b.binary(B::ShiftLeft, raw, by);
            }
            let w = (byte / WORD) as usize;
            parts[w] = Some(match parts[w] {
                Some(p) => b.binary(B::InclusiveOr, p, raw),
                None => raw,
            });
        }
        if Self::narrow(self.size) {
            // One word holds the element: its bits replaced by compare-exchange.
            let byte_mask = b.u32(WORD - 1);
            let in_word = b.binary(B::And, address, byte_mask);
            let to_bits = b.u32(BYTE_SHIFT);
            let bit = b.binary(B::ShiftLeft, in_word, to_bits);
            let bits = parts[0].expect("a component");
            let bits = b.binary(B::ShiftLeft, bits, bit);
            let ones = b.u32(((1u64 << (self.size << BYTE_SHIFT)) - 1) as u32);
            let mask = b.binary(B::ShiftLeft, ones, bit);
            let keep = b.expr(Expression::Unary {
                op: naga::UnaryOperator::BitwiseNot,
                expr: mask,
            });
            let p = b.at(words, first);
            let exchanged = module.generate_predeclared_type(
                naga::PredeclaredType::AtomicCompareExchangeWeakResult(Scalar::U32),
            );
            // Each try compares with the word the last exchange gave, never with a plain load,
            // which a cache may keep stale.
            let first_seen = b.load(p);
            let u32_ty = scalar(module, Scalar::U32);
            let seen = b.local(u32_ty, Some(first_seen));
            b.open();
            let old = b.load(seen);
            let kept = b.binary(B::And, old, keep);
            let new = b.binary(B::InclusiveOr, kept, bits);
            let result = b.expr(Expression::AtomicResult {
                ty: exchanged,
                comparison: true,
            });
            b.stmt(Statement::Atomic {
                pointer: p,
                fun: AtomicFunction::Exchange { compare: Some(old) },
                value: new,
                result: Some(result),
            });
            let now = b.field(result, 0);
            b.store(seen, now);
            let done = b.field(result, 1);
            // A helper lane's exchange does nothing and gives no defined value: it leaves.
            let flag = naga::front::spv::helper_invocation_flag(module, Span::UNDEFINED);
            let flag = b.global(flag);
            let helper = b.load(flag);
            let done = b.binary(B::LogicalOr, done, helper);
            b.when(done, |b| b.stmt(Statement::Break));
            let body = b.close();
            b.stmt(Statement::Loop {
                body,
                continuing: Block::new(),
                break_if: None,
            });
        } else {
            for (w, part) in parts.into_iter().enumerate() {
                let Some(mut bits) = part else { continue };
                let k = b.u32(w as u32);
                let i = b.add(first, k);
                let p = b.at(words, i);
                if let Some(s) = self.atomic
                    && s.kind != ScalarKind::Uint
                {
                    bits = b.bitcast(bits, s.kind);
                }
                b.store(p, bits);
            }
        }
        let accept = b.close();
        b.stmt(Statement::If {
            condition: inside,
            accept,
            reject: Block::new(),
        });
        let f = module.functions.append(b.finish(), Span::UNDEFINED);
        self.store = Some(f);
        f
    }

    /// `texels_<set>_<binding>_atomic_<n>(index, value[, compare]) -> scalar`: `fun` on the element's
    /// word, giving the value before; nothing and zero past the view's length. Only 32-bit integer
    /// formats take atomics, as in Direct3D.
    pub(crate) fn atomic(
        &mut self,
        module: &mut Module,
        reflection: &mut Reflection,
        fun: AtomicFunction,
    ) -> Result<Handle<Function>> {
        let s = match self.atomic {
            Some(s) if self.size == WORD && self.components.len() == 1 => s,
            _ => return Err(ConvertError::Unsupported("TypedBufferAtomicFormat")),
        };
        let code = atomic_code(fun);
        if let Some(&(_, f)) = self.atomics.iter().find(|(c, _)| *c == code) {
            return Ok(f);
        }
        // Float views exchange their bits as u32 words.
        let natural = self.scalar();
        let ty = scalar(module, natural);
        let u32_ty = scalar(module, Scalar::U32);
        let compared = matches!(fun, AtomicFunction::Exchange { compare: Some(_) });
        let mut b = Builder::new(&format!("{}_atomic_{code}", self.name));
        b.f.arguments.push(FunctionArgument {
            name: Some("index".into()),
            ty: u32_ty,
            binding: None,
        });
        let mut names = vec!["value"];
        if compared {
            names.push("compare");
        }
        for name in names {
            b.f.arguments.push(FunctionArgument {
                name: Some(name.into()),
                ty,
                binding: None,
            });
        }
        b.f.result = Some(FunctionResult { ty, binding: None });
        let index = b.argument(0);
        let as_word = |b: &mut Builder, v: Expr| match natural == s {
            true => v,
            false => b.bitcast(v, s.kind),
        };
        let value = b.argument(1);
        let value = as_word(&mut b, value);
        let fun = match fun {
            AtomicFunction::Exchange { compare: Some(_) } => {
                let c = b.argument(2);
                AtomicFunction::Exchange {
                    compare: Some(as_word(&mut b, c)),
                }
            }
            other => other,
        };
        let zero = b.expr(Expression::Literal(literal(natural, 0)));
        let out = b.local(ty, Some(zero));
        let (offset, length) = self.view(module, reflection, &mut b);
        let inside = b.binary(B::Less, index, length);
        b.open();
        let by = b.u32(WORD_SHIFT);
        let first = b.binary(B::ShiftRight, offset, by);
        let i = b.add(first, index);
        let words = b.global(self.var);
        let p = b.at(words, i);
        let rty = if compared {
            module.generate_predeclared_type(
                naga::PredeclaredType::AtomicCompareExchangeWeakResult(s),
            )
        } else {
            scalar(module, s)
        };
        let result = b.expr(Expression::AtomicResult {
            ty: rty,
            comparison: compared,
        });
        b.stmt(Statement::Atomic {
            pointer: p,
            fun,
            value,
            result: Some(result),
        });
        let old = if compared { b.field(result, 0) } else { result };
        let old = match natural == s {
            true => old,
            false => b.bitcast(old, natural.kind),
        };
        b.store(out, old);
        let accept = b.close();
        b.stmt(Statement::If {
            condition: inside,
            accept,
            reject: Block::new(),
        });
        let r = b.load(out);
        b.ret(Some(r));
        let f = module.functions.append(b.finish(), Span::UNDEFINED);
        self.atomics.push((code, f));
        Ok(f)
    }
}

fn atomic_code(fun: AtomicFunction) -> u64 {
    use AtomicFunction as A;
    match fun {
        A::Add => 0,
        A::Subtract => 1,
        A::And => 2,
        A::ExclusiveOr => 3,
        A::InclusiveOr => 4,
        A::Min => 5,
        A::Max => 6,
        A::Exchange { compare: None } => 7,
        A::Exchange { compare: Some(_) } => 8,
    }
}

/// `v` as a literal of scalar `s`.
fn literal(s: Scalar, v: u32) -> Literal {
    match s.kind {
        ScalarKind::Float => Literal::F32(v as f32),
        ScalarKind::Sint => Literal::I32(v as i32),
        _ => Literal::U32(v),
    }
}

/// The scalar a format's components are read as.
fn natural(kind: Kind) -> Scalar {
    match kind {
        Kind::Uint => Scalar::U32,
        Kind::Sint => Scalar::I32,
        _ => Scalar::F32,
    }
}

/// One component's `bits` raw bits from its value, as Direct3D converts (functional specification
/// 3.2.2 and 3.2.3): integers clamped to the format's range; normalised values with NaN as zero,
/// clamped, scaled and rounded half away from zero; floats rounded toward zero, keeping NaN and
/// infinities, past the largest finite value giving it, an unsigned format's negatives zero.
fn encode(f: &mut Function, body: &mut Block, kind: Kind, v: Expr, bits: u32) -> Expr {
    let lit = |f: &mut Function, l: Literal| {
        f.expressions
            .append(Expression::Literal(l), Span::UNDEFINED)
    };
    let mask = |f: &mut Function, body: &mut Block, x: Expr| {
        if bits == u32::BITS {
            return x;
        }
        let m = lit(f, Literal::U32(((1u64 << bits) - 1) as u32));
        binary(f, body, B::And, x, m)
    };
    let to_u32 = |f: &mut Function, body: &mut Block, x: Expr, convert: Option<u8>| {
        push(
            f,
            body,
            Expression::As {
                expr: x,
                kind: ScalarKind::Uint,
                convert,
            },
        )
    };
    match kind {
        Kind::Uint if bits == u32::BITS => v,
        Kind::Uint => {
            let max = lit(f, Literal::U32(((1u64 << bits) - 1) as u32));
            math(f, body, MathFunction::Min, [v, max])
        }
        Kind::Sint => {
            let v = if bits == u32::BITS {
                v
            } else {
                let lo = lit(f, Literal::I32(-(1 << (bits - 1))));
                let hi = lit(f, Literal::I32((1 << (bits - 1)) - 1));
                push(
                    f,
                    body,
                    Expression::Math {
                        fun: MathFunction::Clamp,
                        arg: v,
                        arg1: Some(lo),
                        arg2: Some(hi),
                        arg3: None,
                    },
                )
            };
            let u = to_u32(f, body, v, None);
            mask(f, body, u)
        }
        Kind::Unorm | Kind::Snorm => {
            let signed = kind == Kind::Snorm;
            let zero = lit(f, Literal::F32(0.0));
            let one = lit(f, Literal::F32(1.0));
            let low = lit(f, Literal::F32(if signed { -1.0 } else { 0.0 }));
            // NaN fails both comparisons and becomes zero.
            let above = binary(f, body, B::GreaterEqual, v, low);
            let c = select(f, body, above, v, low);
            let below = binary(f, body, B::LessEqual, c, one);
            let c = select(f, body, below, c, one);
            let number = binary(f, body, B::Equal, v, v);
            let c = select(f, body, number, c, zero);
            let n = if signed { bits - 1 } else { bits };
            let scale = lit(f, Literal::F32(((1u64 << n) - 1) as f32));
            let c = binary(f, body, B::Multiply, c, scale);
            let half = lit(f, Literal::F32(0.5));
            if signed {
                let minus = lit(f, Literal::F32(-0.5));
                let negative = binary(f, body, B::Less, c, zero);
                let h = select(f, body, negative, minus, half);
                let c = binary(f, body, B::Add, c, h);
                let i = push(
                    f,
                    body,
                    Expression::As {
                        expr: c,
                        kind: ScalarKind::Sint,
                        convert: Some(Scalar::I32.width),
                    },
                );
                let u = to_u32(f, body, i, None);
                mask(f, body, u)
            } else {
                let c = binary(f, body, B::Add, c, half);
                to_u32(f, body, c, Some(Scalar::U32.width))
            }
        }
        Kind::Float if bits == u32::BITS => to_u32(f, body, v, None),
        Kind::Float => small_float(f, body, v, bits),
    }
}

/// The bits of a float of five exponent bits and `bits - 5` mantissa bits (11- and 10-bit floats,
/// unsigned) or of a half float (`bits` 16), from an f32, rounded toward zero.
fn small_float(f: &mut Function, body: &mut Block, v: Expr, bits: u32) -> Expr {
    let lit = |f: &mut Function, x: u32| {
        f.expressions
            .append(Expression::Literal(Literal::U32(x)), Span::UNDEFINED)
    };
    let m = if bits == 16 { 10 } else { bits - 5 };
    let u = push(
        f,
        body,
        Expression::As {
            expr: v,
            kind: ScalarKind::Uint,
            convert: None,
        },
    );
    let abs_mask = lit(f, 0x7fff_ffff);
    let a = binary(f, body, B::And, u, abs_mask);
    let twenty_three = lit(f, 23);
    let e = binary(f, body, B::ShiftRight, a, twenty_three);
    let drop = lit(f, 23 - m);
    let fraction = binary(f, body, B::ShiftRight, a, drop);
    let mantissa_mask = lit(f, (1 << m) - 1);
    let fraction = binary(f, body, B::And, fraction, mantissa_mask);
    let top = lit(f, 0x1f << m);
    // NaN: the exponent's ones and the top mantissa bits, made quiet.
    let quiet = lit(f, 1 << (m - 1));
    let nan = binary(f, body, B::InclusiveOr, top, quiet);
    let nan = binary(f, body, B::InclusiveOr, nan, fraction);
    // The largest finite value.
    let largest = lit(f, (0x1e << m) | ((1 << m) - 1));
    // Normal: the exponent rebiased from 127 to 15.
    let rebias = lit(f, 112);
    let ne = binary(f, body, B::Subtract, e, rebias);
    let by = lit(f, m);
    let ne = binary(f, body, B::ShiftLeft, ne, by);
    let normal = binary(f, body, B::InclusiveOr, ne, fraction);
    // Denormal: the full mantissa shifted down, toward zero.
    let mantissa_all = lit(f, 0x7f_ffff);
    let full = binary(f, body, B::And, a, mantissa_all);
    let implicit = lit(f, 0x80_0000);
    let full = binary(f, body, B::InclusiveOr, full, implicit);
    let base = lit(f, 136 - m);
    let e_small = math(f, body, MathFunction::Min, [e, base]);
    let shift = binary(f, body, B::Subtract, base, e_small);
    let thirty_one = lit(f, 31);
    let shift = math(f, body, MathFunction::Min, [shift, thirty_one]);
    let denormal = binary(f, body, B::ShiftRight, full, shift);
    let infinity = lit(f, 0x7f80_0000);
    let is_nan = binary(f, body, B::Greater, a, infinity);
    let is_inf = binary(f, body, B::Equal, a, infinity);
    let past = lit(f, 143);
    let is_big = binary(f, body, B::GreaterEqual, e, past);
    let low = lit(f, 113);
    let is_small = binary(f, body, B::Less, e, low);
    let r = select(f, body, is_small, denormal, normal);
    let r = select(f, body, is_big, largest, r);
    let r = select(f, body, is_inf, top, r);
    let r = select(f, body, is_nan, nan, r);
    let thirty_one = lit(f, 31);
    let sign = binary(f, body, B::ShiftRight, u, thirty_one);
    if bits == 16 {
        let fifteen = lit(f, 15);
        let sign = binary(f, body, B::ShiftLeft, sign, fifteen);
        return binary(f, body, B::InclusiveOr, r, sign);
    }
    // No sign: negative values (not NaN) are zero.
    let one = lit(f, 1);
    let negative = binary(f, body, B::Equal, sign, one);
    let not_nan = push(
        f,
        body,
        Expression::Unary {
            op: naga::UnaryOperator::LogicalNot,
            expr: is_nan,
        },
    );
    let negative = binary(f, body, B::LogicalAnd, negative, not_nan);
    let zero = lit(f, 0);
    select(f, body, negative, zero, r)
}
