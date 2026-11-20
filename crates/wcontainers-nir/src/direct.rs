//! Candidate B (plan 15.3): NIR read straight into a naga module, with no SPIR-V between. The bridge
//! takes the shader out of SSA (every value used outside its block in a register) and flattens it
//! (`bridge/flat.h`); this walks the flat form in order. NIR's values carry no type, so each value
//! keeps the scalar kind its instruction gives it and every use converts its bits to the kind it
//! needs.

use crate::ConvertError;
use crate::mesa::{Nir, NirShader};
use naga::{
    AddressSpace, BinaryOperator, Binding, Block, BuiltIn, Expression, Function, FunctionArgument,
    Handle, ImageClass, ImageDimension, Interpolation, Literal, LocalVariable, MathFunction,
    Module, RelationalFunction, SampleLevel, Sampling, Scalar, ScalarKind, ShaderStage, Span,
    Statement, StorageAccess, StorageFormat, Type, TypeInner, UnaryOperator, VectorSize,
};
use std::ffi::{CStr, c_char};

const NONE: u32 = u32::MAX;
/// A workgroup's size along x, y and z, as naga's entry points hold it.
type WorkgroupSize = [u32; VectorSize::Tri as usize];
const SOURCES: usize = 5;
/// A 32-bit scalar's bytes, the only width the route takes (64-bit operations are lowered).
const WIDTH: u8 = 4;
/// NIR's booleans are one bit wide.
const BOOL_BITS: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum FlatScalar {
    Float,
    Sint,
    Uint,
    Bool,
    Other,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum TypeKind {
    Scalar,
    Vector,
    Matrix,
    Array,
    Struct,
    Sampler,
    Texture,
    Image,
    Other,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum Dim {
    D1,
    D2,
    D3,
    Cube,
    Other,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct FlatType {
    kind: TypeKind,
    scalar: FlatScalar,
    bits: u32,
    components: u32,
    columns: u32,
    length: u32,
    element: u32,
    stride: u32,
    row_major: u32,
    dim: Dim,
    arrayed: u32,
    shadow: u32,
    multisampled: u32,
    sampled: FlatScalar,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct FlatField {
    ty: u32,
    offset: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum VarMode {
    Input,
    Output,
    Handle,
    UniformBuffer,
    StorageBuffer,
    Immediate,
    Workgroup,
    Private,
    Function,
    Other,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum FlatBuiltIn {
    None,
    Position,
    PointSize,
    ClipDistance,
    FragDepth,
    SampleMask,
    Other,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum FlatInterpolation {
    Perspective,
    Linear,
    Flat,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum Format {
    None,
    R32Float,
    R32Uint,
    R32Sint,
    Rg32Float,
    Rgba32Float,
    Rgba32Uint,
    Rgba32Sint,
    Rgba16Float,
    Rgba8Unorm,
    Rgba8Snorm,
    R8Unorm,
    Rg8Unorm,
    Other,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct FlatVar {
    mode: VarMode,
    ty: u32,
    builtin: FlatBuiltIn,
    location: u32,
    set: u32,
    binding: u32,
    interpolation: FlatInterpolation,
    centroid: u32,
    sample: u32,
    readable: u32,
    writable: u32,
    format: Format,
    initialised: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum Kind {
    Alu,
    Const,
    Undef,
    Deref,
    Intrinsic,
    Tex,
    Break,
    Continue,
    Return,
    If,
    Else,
    EndIf,
    Loop,
    EndLoop,
    Unsupported,
}

/// The ALU operations of `WC_ALU_OPS` in `bridge/flat.h`, in its order.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum Alu {
    Mov,
    Vec2,
    Vec3,
    Vec4,
    Fadd,
    Fsub,
    Fmul,
    Ffma,
    FfmaWeak,
    Flrp,
    Fdiv,
    Fneg,
    Fabs,
    Fsign,
    Fmin,
    Fmax,
    Ffloor,
    Fceil,
    Ffract,
    Ftrunc,
    FroundEven,
    Fsqrt,
    Frsq,
    Frcp,
    Fexp2,
    Flog2,
    Fpow,
    Fsin,
    Fcos,
    Fdot2,
    Fdot3,
    Fdot4,
    Flt,
    Fge,
    Feq,
    Fneu,
    Iadd,
    Isub,
    Imul,
    Ineg,
    Iabs,
    Imin,
    Imax,
    Umin,
    Umax,
    Idiv,
    Udiv,
    Irem,
    Umod,
    Ishl,
    Ishr,
    Ushr,
    Iand,
    Ior,
    Ixor,
    Inot,
    Ilt,
    Ige,
    Ieq,
    Ine,
    Ult,
    Uge,
    Bcsel,
    I2f32,
    U2f32,
    F2i32,
    F2u32,
    B2f32,
    B2i32,
    BitCount,
    UfindMsb,
    IfindMsb,
    FindLsb,
    BitfieldReverse,
    PackHalf2x16,
    UnpackHalf2x16,
    PackUnorm4x8,
    UnpackUnorm4x8,
    PackSnorm4x8,
    UnpackSnorm4x8,
    PackUnorm2x16,
    UnpackUnorm2x16,
    BallFequal2,
    BallFequal3,
    BallFequal4,
    BanyFnequal2,
    BanyFnequal3,
    BanyFnequal4,
    BallIequal2,
    BallIequal3,
    BallIequal4,
    BanyInequal2,
    BanyInequal3,
    BanyInequal4,
    Unsupported,
}

/// The intrinsics of `WC_INTRINSICS` in `bridge/flat.h`, in its order.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum Intrinsic {
    LoadDeref,
    StoreDeref,
    DeclReg,
    LoadReg,
    StoreReg,
    VulkanResourceIndex,
    LoadVulkanDescriptor,
    LoadVertexId,
    LoadInstanceId,
    LoadFrontFace,
    LoadFragCoord,
    LoadSampleId,
    LoadLocalInvocationId,
    LoadGlobalInvocationId,
    LoadWorkgroupId,
    LoadNumWorkgroups,
    LoadLocalInvocationIndex,
    Demote,
    Terminate,
    DemoteIf,
    TerminateIf,
    Barrier,
    DerefBufferArrayLength,
    Ddx,
    Ddy,
    DdxFine,
    DdyFine,
    DdxCoarse,
    DdyCoarse,
    Unsupported,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum Deref {
    Var,
    Array,
    Struct,
    Cast,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum TexOp {
    Sample,
    Bias,
    Level,
    Gradient,
    Load,
    Size,
    Levels,
    Gather,
}

/// A texture instruction's operands, by `enum wc_tex_src`.
#[derive(Clone, Copy)]
enum TexSrc {
    Coord,
    Texture,
    Sampler,
    Bias,
    Lod,
    Comparator,
    Offset,
    Ddx,
    Ddy,
    Sample,
}
const TEX_SRCS: usize = TexSrc::Sample as usize + 1;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct FlatSrc {
    def: u32,
    swizzle: [u8; VectorSize::MAX],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct FlatInstr {
    kind: Kind,
    op: u32,
    def: u32,
    components: u32,
    bits: u32,
    sources: u32,
    src: [FlatSrc; SOURCES],
    value: [u32; VectorSize::MAX],
    ty: u32,
    tex: [u32; TEX_SRCS],
    dim: Dim,
    arrayed: u32,
    shadow: u32,
    component: u32,
    name: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum FlatStage {
    Vertex,
    Fragment,
    Compute,
}

#[repr(C)]
struct Flat {
    stage: FlatStage,
    workgroup_size: WorkgroupSize,
    early_fragment_tests: u32,
    values: u32,
    types: *const FlatType,
    type_count: u32,
    fields: *const FlatField,
    field_count: u32,
    vars: *const FlatVar,
    var_count: u32,
    initialisers: *const u32,
    initialiser_count: u32,
    instrs: *const FlatInstr,
    instr_count: u32,
}

unsafe extern "C" {
    fn wc_nir_flatten(nir: *mut NirShader, out: *mut Flat) -> bool;
    fn wc_flat_free(flat: *mut Flat);
}

/// The flattened shader, given back to the bridge on drop.
struct Flattened(Flat);

impl Drop for Flattened {
    fn drop(&mut self) {
        // SAFETY: the arrays came from wc_nir_flatten and are freed once.
        unsafe { wc_flat_free(&mut self.0) };
    }
}

fn slice<'a, T>(data: *const T, count: u32) -> &'a [T] {
    if data.is_null() {
        &[]
    } else {
        // SAFETY: the bridge's arrays hold `count` entries until wc_flat_free.
        unsafe { std::slice::from_raw_parts(data, count as usize) }
    }
}

/// Why candidate B refused a shader: what of NIR it does not take.
fn unsupported(what: impl std::fmt::Display) -> ConvertError {
    ConvertError::Unsupported(what.to_string())
}

fn name(instr: &FlatInstr) -> String {
    if instr.name.is_null() {
        format!("{:?}", instr.kind)
    } else {
        // SAFETY: names are Mesa's static strings.
        unsafe { CStr::from_ptr(instr.name) }
            .to_string_lossy()
            .into_owned()
    }
}

/// A NIR value as the module has it.
#[derive(Clone, Copy, Debug)]
enum Value {
    /// An expression of this scalar kind.
    Expr(Handle<Expression>, ScalarKind),
    /// A constant, made at each use in the kind the use needs.
    Const([u32; VectorSize::MAX], u32, bool),
    /// A pointer to a value of the flat type.
    Pointer(Handle<Expression>, u32),
    /// A register (a local variable of u32 or bool components).
    Register(Handle<LocalVariable>, bool),
    /// A buffer resource by its set and binding, with its index in an array of them.
    Resource(u32, u32, u32),
}

/// An open `if` or loop.
enum Open {
    If(Handle<Expression>, Option<Block>),
    Loop,
}

struct Builder<'a> {
    stage: ShaderStage,
    types: &'a [FlatType],
    fields: &'a [FlatField],
    vars: &'a [FlatVar],
    module: Module,
    naga_types: Vec<Option<Handle<Type>>>,
    layouter: naga::proc::Layouter,
    function: Function,
    emitter: naga::proc::Emitter,
    blocks: Vec<Block>,
    open: Vec<Open>,
    values: Vec<Option<Value>>,
    counts: Vec<u32>,
    places: Vec<Option<Handle<Expression>>>,
    outputs: Vec<(Handle<LocalVariable>, Handle<Type>, Binding)>,
    builtins: Vec<(BuiltIn, Handle<Expression>)>,
}

/// Candidate B: the program of `nir` as a naga module. The shader is changed by the preparation.
pub fn module(nir: &mut Nir) -> Result<Module, ConvertError> {
    let mut flat = Flattened(Flat {
        stage: FlatStage::Vertex,
        workgroup_size: WorkgroupSize::default(),
        early_fragment_tests: 0,
        values: 0,
        types: std::ptr::null(),
        type_count: 0,
        fields: std::ptr::null(),
        field_count: 0,
        vars: std::ptr::null(),
        var_count: 0,
        initialisers: std::ptr::null(),
        initialiser_count: 0,
        instrs: std::ptr::null(),
        instr_count: 0,
    });
    // SAFETY: the shader is live; the bridge fills `flat` with arrays it owns.
    if !unsafe { wc_nir_flatten(nir.raw(), &mut flat.0) } {
        return Err(ConvertError::Mesa(FLATTEN));
    }
    let f = &flat.0;
    let stage = match f.stage {
        FlatStage::Vertex => ShaderStage::Vertex,
        FlatStage::Fragment => ShaderStage::Fragment,
        FlatStage::Compute => ShaderStage::Compute,
    };
    let mut b = Builder {
        stage,
        types: slice(f.types, f.type_count),
        fields: slice(f.fields, f.field_count),
        vars: slice(f.vars, f.var_count),
        module: Module::default(),
        naga_types: vec![None; f.type_count as usize],
        layouter: naga::proc::Layouter::default(),
        function: Function::default(),
        emitter: naga::proc::Emitter::default(),
        blocks: vec![Block::new()],
        open: Vec::new(),
        values: vec![None; f.values as usize],
        counts: vec![0; f.values as usize],
        places: vec![None; f.var_count as usize],
        outputs: Vec::new(),
        builtins: Vec::new(),
    };
    b.emitter.start(&b.function.expressions);
    b.variables()?;
    for instr in slice(f.instrs, f.instr_count) {
        b.instr(instr)?;
    }
    b.finish(f)
}

/// The step named when the bridge cannot flatten a shader.
pub const FLATTEN: &str = "wc_nir_flatten";

fn vector_size(n: u32) -> Result<VectorSize, ConvertError> {
    [VectorSize::Bi, VectorSize::Tri, VectorSize::Quad]
        .into_iter()
        .find(|s| *s as u32 == n)
        .ok_or_else(|| unsupported(format_args!("a vector of {n}")))
}

fn scalar_of(s: FlatScalar) -> Result<Scalar, ConvertError> {
    Ok(match s {
        FlatScalar::Float => Scalar::F32,
        FlatScalar::Sint => Scalar::I32,
        FlatScalar::Uint => Scalar::U32,
        FlatScalar::Bool => Scalar::BOOL,
        FlatScalar::Other => {
            return Err(unsupported(format_args!(
                "a scalar of another kind or size"
            )));
        }
    })
}

fn dimension(d: Dim) -> Result<ImageDimension, ConvertError> {
    Ok(match d {
        Dim::D1 => ImageDimension::D1,
        Dim::D2 => ImageDimension::D2,
        Dim::D3 => ImageDimension::D3,
        Dim::Cube => ImageDimension::Cube,
        Dim::Other => return Err(unsupported(format_args!("a texture of another dimension"))),
    })
}

fn storage_format(f: Format) -> Result<StorageFormat, ConvertError> {
    Ok(match f {
        Format::R32Float => StorageFormat::R32Float,
        Format::R32Uint => StorageFormat::R32Uint,
        Format::R32Sint => StorageFormat::R32Sint,
        Format::Rg32Float => StorageFormat::Rg32Float,
        Format::Rgba32Float => StorageFormat::Rgba32Float,
        Format::Rgba32Uint => StorageFormat::Rgba32Uint,
        Format::Rgba32Sint => StorageFormat::Rgba32Sint,
        Format::Rgba16Float => StorageFormat::Rgba16Float,
        Format::Rgba8Unorm => StorageFormat::Rgba8Unorm,
        Format::Rgba8Snorm => StorageFormat::Rgba8Snorm,
        Format::R8Unorm => StorageFormat::R8Unorm,
        Format::Rg8Unorm => StorageFormat::Rg8Unorm,
        Format::None => {
            return Err(unsupported(format_args!(
                "a storage image without a format"
            )));
        }
        Format::Other => {
            return Err(unsupported(format_args!(
                "a storage image of another format"
            )));
        }
    })
}

impl Builder<'_> {
    fn add_type(&mut self, inner: TypeInner) -> Handle<Type> {
        self.module
            .types
            .insert(Type { name: None, inner }, Span::UNDEFINED)
    }

    /// The layout of a type of the module, for types with no explicit layout.
    fn layout(&mut self, ty: Handle<Type>) -> Result<naga::proc::TypeLayout, ConvertError> {
        self.layouter
            .update(self.module.to_ctx())
            .map_err(unsupported)?;
        Ok(self.layouter[ty])
    }

    fn ty(&mut self, index: u32) -> Result<Handle<Type>, ConvertError> {
        if let Some(h) = self.naga_types[index as usize] {
            return Ok(h);
        }
        let t = self.types[index as usize];
        let inner = match t.kind {
            TypeKind::Scalar | TypeKind::Vector | TypeKind::Matrix
                if t.scalar != FlatScalar::Bool && t.bits != u32::from(WIDTH) * u8::BITS =>
            {
                return Err(unsupported(format_args!("{}-bit values", t.bits)));
            }
            TypeKind::Scalar => TypeInner::Scalar(scalar_of(t.scalar)?),
            TypeKind::Vector => TypeInner::Vector {
                size: vector_size(t.components)?,
                scalar: scalar_of(t.scalar)?,
            },
            TypeKind::Matrix if t.row_major != 0 => {
                return Err(unsupported(format_args!("a row-major matrix")));
            }
            TypeKind::Matrix => TypeInner::Matrix {
                columns: vector_size(t.columns)?,
                rows: vector_size(t.components)?,
                scalar: scalar_of(t.scalar)?,
            },
            TypeKind::Array => {
                let element = self.types[t.element as usize];
                if matches!(
                    element.kind,
                    TypeKind::Sampler | TypeKind::Texture | TypeKind::Image
                ) {
                    let base = self.ty(t.element)?;
                    TypeInner::BindingArray {
                        base,
                        size: std::num::NonZeroU32::new(t.length)
                            .map_or(naga::ArraySize::Dynamic, naga::ArraySize::Constant),
                    }
                } else {
                    let base = self.ty(t.element)?;
                    let stride = if t.stride == 0 {
                        let l = self.layout(base)?;
                        l.alignment.round_up(l.size)
                    } else {
                        t.stride
                    };
                    TypeInner::Array {
                        base,
                        size: std::num::NonZeroU32::new(t.length)
                            .map_or(naga::ArraySize::Dynamic, naga::ArraySize::Constant),
                        stride,
                    }
                }
            }
            TypeKind::Struct => {
                let mut members = Vec::new();
                let mut end = 0;
                for i in 0..t.length {
                    let field = self.fields[(t.element + i) as usize];
                    let ty = self.ty(field.ty)?;
                    let layout = self.layout(ty)?;
                    let offset = u32::try_from(field.offset)
                        .unwrap_or_else(|_| layout.alignment.round_up(end));
                    end = offset + layout.size;
                    members.push(naga::StructMember {
                        name: None,
                        ty,
                        binding: None,
                        offset,
                    });
                }
                let alignment = members
                    .iter()
                    .map(|m| self.layouter[m.ty].alignment)
                    .max()
                    .unwrap_or(naga::proc::Alignment::ONE);
                TypeInner::Struct {
                    members,
                    span: alignment.round_up(end),
                }
            }
            TypeKind::Sampler => TypeInner::Sampler {
                comparison: t.shadow != 0,
            },
            TypeKind::Texture => TypeInner::Image {
                dim: dimension(t.dim)?,
                arrayed: t.arrayed != 0,
                class: if t.shadow != 0 {
                    ImageClass::Depth {
                        multi: t.multisampled != 0,
                    }
                } else {
                    ImageClass::Sampled {
                        kind: scalar_of(t.sampled)?.kind,
                        multi: t.multisampled != 0,
                    }
                },
            },
            TypeKind::Image | TypeKind::Other => {
                return Err(unsupported(format_args!("a type of another kind")));
            }
        };
        let h = self.add_type(inner);
        self.naga_types[index as usize] = Some(h);
        Ok(h)
    }

    /// The scalar kind of a flat type's components.
    fn kind_of(&self, index: u32) -> ScalarKind {
        let mut t = self.types[index as usize];
        while matches!(t.kind, TypeKind::Array | TypeKind::Matrix) {
            t = self.types[t.element as usize];
        }
        scalar_of(t.scalar).map_or(ScalarKind::Uint, |s| s.kind)
    }

    fn storage_image(&mut self, var: &FlatVar) -> Result<Handle<Type>, ConvertError> {
        let mut t = self.types[var.ty as usize];
        let count = (t.kind == TypeKind::Array).then_some(t.length);
        if count.is_some() {
            t = self.types[t.element as usize];
        }
        let mut access = StorageAccess::empty();
        access.set(StorageAccess::LOAD, var.readable != 0);
        access.set(StorageAccess::STORE, var.writable != 0);
        let image = self.add_type(TypeInner::Image {
            dim: dimension(t.dim)?,
            arrayed: t.arrayed != 0,
            class: ImageClass::Storage {
                format: storage_format(var.format)?,
                access,
            },
        });
        Ok(match count {
            Some(n) => self.add_type(TypeInner::BindingArray {
                base: image,
                size: std::num::NonZeroU32::new(n)
                    .map_or(naga::ArraySize::Dynamic, naga::ArraySize::Constant),
            }),
            None => image,
        })
    }

    fn add(&mut self, e: Expression) -> Handle<Expression> {
        if e.needs_pre_emit() {
            if let Some((s, span)) = self.emitter.finish(&self.function.expressions) {
                self.blocks.last_mut().expect("a block").push(s, span);
            }
            let h = self.function.expressions.append(e, Span::UNDEFINED);
            self.emitter.start(&self.function.expressions);
            h
        } else {
            self.function.expressions.append(e, Span::UNDEFINED)
        }
    }

    /// Ends the running emit range into the current block, and starts the next.
    fn flush(&mut self) {
        if let Some((s, span)) = self.emitter.finish(&self.function.expressions) {
            self.blocks.last_mut().expect("a block").push(s, span);
        }
        self.emitter.start(&self.function.expressions);
    }

    fn statement(&mut self, s: Statement) {
        self.flush();
        self.blocks
            .last_mut()
            .expect("a block")
            .push(s, Span::UNDEFINED);
    }

    fn builtin(&mut self, builtin: BuiltIn, inner: TypeInner) -> Handle<Expression> {
        if let Some(&(_, h)) = self.builtins.iter().find(|(b, _)| *b == builtin) {
            return h;
        }
        let ty = self.add_type(inner);
        let index = u32::try_from(self.function.arguments.len()).expect("few arguments");
        self.function.arguments.push(FunctionArgument {
            name: None,
            ty,
            binding: Some(Binding::BuiltIn(builtin)),
        });
        let h = self.add(Expression::FunctionArgument(index));
        self.builtins.push((builtin, h));
        h
    }

    fn location(var: &FlatVar) -> Binding {
        let (interpolation, sampling) = match var.interpolation {
            FlatInterpolation::Flat => (Interpolation::Flat, None),
            FlatInterpolation::Linear => (Interpolation::Linear, Some(Sampling::Center)),
            FlatInterpolation::Perspective => (Interpolation::Perspective, Some(Sampling::Center)),
        };
        let sampling = sampling.map(|s| {
            if var.sample != 0 {
                Sampling::Sample
            } else if var.centroid != 0 {
                Sampling::Centroid
            } else {
                s
            }
        });
        Binding::Location {
            location: var.location,
            interpolation: Some(interpolation),
            sampling,
            blend_src: None,
            per_primitive: false,
        }
    }

    fn stage_binding(&self, var: &FlatVar) -> Result<Binding, ConvertError> {
        Ok(match var.builtin {
            FlatBuiltIn::None => {
                if self.stage == ShaderStage::Vertex && var.mode == VarMode::Input
                    || self.stage == ShaderStage::Fragment && var.mode == VarMode::Output
                {
                    Binding::Location {
                        location: var.location,
                        interpolation: None,
                        sampling: None,
                        blend_src: None,
                        per_primitive: false,
                    }
                } else {
                    Self::location(var)
                }
            }
            FlatBuiltIn::Position => Binding::BuiltIn(BuiltIn::Position { invariant: false }),
            FlatBuiltIn::PointSize => Binding::BuiltIn(BuiltIn::PointSize),
            FlatBuiltIn::ClipDistance => Binding::BuiltIn(BuiltIn::ClipDistances),
            FlatBuiltIn::FragDepth => Binding::BuiltIn(BuiltIn::FragDepth),
            FlatBuiltIn::SampleMask => Binding::BuiltIn(BuiltIn::SampleMask),
            FlatBuiltIn::Other => {
                return Err(unsupported(format_args!(
                    "a stage value of another built-in"
                )));
            }
        })
    }

    /// Every variable as a global, an entry point argument copied into a local, or a local.
    fn variables(&mut self) -> Result<(), ConvertError> {
        for (index, var) in self.vars.iter().enumerate() {
            let place = match var.mode {
                VarMode::Input | VarMode::Output => {
                    let t = self.types[var.ty as usize];
                    if !matches!(t.kind, TypeKind::Scalar | TypeKind::Vector)
                        && var.builtin != FlatBuiltIn::ClipDistance
                    {
                        return Err(unsupported(format_args!(
                            "a stage value that is an array or a block"
                        )));
                    }
                    let ty = self.ty(var.ty)?;
                    let binding = self.stage_binding(var)?;
                    let local = self.function.local_variables.append(
                        LocalVariable {
                            name: None,
                            ty,
                            init: None,
                        },
                        Span::UNDEFINED,
                    );
                    let pointer = self.add(Expression::LocalVariable(local));
                    if var.mode == VarMode::Input {
                        let at = u32::try_from(self.function.arguments.len()).expect("few");
                        self.function.arguments.push(FunctionArgument {
                            name: None,
                            ty,
                            binding: Some(binding),
                        });
                        let value = self.add(Expression::FunctionArgument(at));
                        self.statement(Statement::Store { pointer, value });
                    } else {
                        self.outputs.push((local, ty, binding));
                    }
                    pointer
                }
                VarMode::Function => {
                    let ty = self.ty(var.ty)?;
                    let local = self.function.local_variables.append(
                        LocalVariable {
                            name: None,
                            ty,
                            init: None,
                        },
                        Span::UNDEFINED,
                    );
                    if var.initialised != NONE {
                        return Err(unsupported(format_args!(
                            "a local variable with an initialiser"
                        )));
                    }
                    self.add(Expression::LocalVariable(local))
                }
                VarMode::Other => {
                    return Err(unsupported(format_args!("a variable of another mode")));
                }
                _ => {
                    let space = match var.mode {
                        VarMode::UniformBuffer => AddressSpace::Uniform,
                        VarMode::StorageBuffer => {
                            let mut access = StorageAccess::empty();
                            access.set(StorageAccess::LOAD, var.readable != 0);
                            access.set(StorageAccess::STORE, var.writable != 0);
                            AddressSpace::Storage { access }
                        }
                        VarMode::Immediate => AddressSpace::Immediate,
                        VarMode::Workgroup => AddressSpace::WorkGroup,
                        VarMode::Private => AddressSpace::Private,
                        _ => AddressSpace::Handle,
                    };
                    if var.initialised != NONE {
                        return Err(unsupported(format_args!(
                            "a global variable with an initialiser"
                        )));
                    }
                    let is_image = {
                        let mut t = self.types[var.ty as usize];
                        if t.kind == TypeKind::Array {
                            t = self.types[t.element as usize];
                        }
                        t.kind == TypeKind::Image
                    };
                    let ty = if is_image {
                        self.storage_image(var)?
                    } else {
                        let block = self.ty(var.ty)?;
                        let t = self.types[var.ty as usize];
                        if t.kind == TypeKind::Array
                            && matches!(space, AddressSpace::Uniform | AddressSpace::Storage { .. })
                        {
                            let element = self.ty(t.element)?;
                            self.add_type(TypeInner::BindingArray {
                                base: element,
                                size: std::num::NonZeroU32::new(t.length)
                                    .map_or(naga::ArraySize::Dynamic, naga::ArraySize::Constant),
                            })
                        } else {
                            block
                        }
                    };
                    let binding = matches!(
                        space,
                        AddressSpace::Uniform | AddressSpace::Storage { .. } | AddressSpace::Handle
                    )
                    .then_some(naga::ResourceBinding {
                        group: var.set,
                        binding: var.binding,
                    });
                    let global = self.module.global_variables.append(
                        naga::GlobalVariable {
                            name: None,
                            space,
                            binding,
                            ty,
                            init: None,
                            memory_decorations: naga::MemoryDecorations::empty(),
                        },
                        Span::UNDEFINED,
                    );
                    self.add(Expression::GlobalVariable(global))
                }
            };
            self.places[index] = Some(place);
        }
        Ok(())
    }

    fn value(&self, def: u32) -> Result<Value, ConvertError> {
        self.values
            .get(def as usize)
            .copied()
            .flatten()
            .ok_or_else(|| unsupported(format_args!("a value used before it is made")))
    }

    fn set(&mut self, instr: &FlatInstr, v: Value) {
        self.values[instr.def as usize] = Some(v);
        self.counts[instr.def as usize] = instr.components;
    }

    fn literal(kind: ScalarKind, bits: u32) -> Literal {
        match kind {
            ScalarKind::Float => Literal::F32(f32::from_bits(bits)),
            ScalarKind::Sint => Literal::I32(bits.cast_signed()),
            ScalarKind::Bool => Literal::Bool(bits != 0),
            _ => Literal::U32(bits),
        }
    }

    /// The whole value of `def`, its bits in `kind`.
    fn whole(
        &mut self,
        def: u32,
        kind: ScalarKind,
    ) -> Result<(Handle<Expression>, u32), ConvertError> {
        let components = self.components(def)?;
        let e = match self.value(def)? {
            Value::Expr(e, k) => self.convert(e, k, kind),
            Value::Const(values, n, is_bool) => {
                let kind = if is_bool { ScalarKind::Bool } else { kind };
                let parts: Vec<_> = values[..n as usize]
                    .iter()
                    .map(|&v| self.add(Expression::Literal(Self::literal(kind, v))))
                    .collect();
                if n == 1 {
                    parts[0]
                } else {
                    let ty = self.add_type(TypeInner::Vector {
                        size: vector_size(n)?,
                        scalar: Scalar {
                            kind,
                            width: Self::width(kind),
                        },
                    });
                    self.add(Expression::Compose {
                        ty,
                        components: parts,
                    })
                }
            }
            Value::Register(local, is_bool) => {
                let pointer = self.add(Expression::LocalVariable(local));
                let e = self.add(Expression::Load { pointer });
                let k = if is_bool {
                    ScalarKind::Bool
                } else {
                    ScalarKind::Uint
                };
                self.convert(e, k, kind)
            }
            Value::Pointer(e, _) => e,
            Value::Resource(..) => {
                return Err(unsupported(format_args!(
                    "a resource index used as a value"
                )));
            }
        };
        Ok((e, components))
    }

    fn width(kind: ScalarKind) -> u8 {
        if kind == ScalarKind::Bool {
            naga::BOOL_WIDTH
        } else {
            WIDTH
        }
    }

    fn components(&self, def: u32) -> Result<u32, ConvertError> {
        Ok(match self.value(def)? {
            Value::Const(_, n, _) => n,
            _ => self.counts[def as usize],
        })
    }

    /// The bits of `e` (of kind `from`) as `to`.
    fn convert(
        &mut self,
        e: Handle<Expression>,
        from: ScalarKind,
        to: ScalarKind,
    ) -> Handle<Expression> {
        if from == to || to == ScalarKind::Bool || from == ScalarKind::Bool {
            e
        } else {
            self.add(Expression::As {
                expr: e,
                kind: to,
                convert: None,
            })
        }
    }

    /// Source `s` of `instr`, `count` components of it by its swizzle, as `kind`.
    fn src(
        &mut self,
        instr: &FlatInstr,
        s: usize,
        count: u32,
        kind: ScalarKind,
    ) -> Result<Handle<Expression>, ConvertError> {
        let src = instr.src[s];
        let (e, have) = self.whole(src.def, kind)?;
        let swizzle = &src.swizzle[..count as usize];
        Ok(if have == 1 {
            if count == 1 {
                e
            } else {
                self.add(Expression::Splat {
                    size: vector_size(count)?,
                    value: e,
                })
            }
        } else if count == 1 {
            self.add(Expression::AccessIndex {
                base: e,
                index: u32::from(swizzle[0]),
            })
        } else if count == have
            && swizzle
                .iter()
                .enumerate()
                .all(|(i, &c)| usize::from(c) == i)
        {
            e
        } else {
            let mut pattern = [naga::SwizzleComponent::X; VectorSize::MAX];
            for (p, &c) in pattern.iter_mut().zip(swizzle) {
                *p = naga::SwizzleComponent::XYZW[usize::from(c)];
            }
            self.add(Expression::Swizzle {
                size: vector_size(count)?,
                vector: e,
                pattern,
            })
        })
    }
}

/// What an ALU operation is in naga: its source kind, and how its result is made.
enum AluForm {
    Binary(BinaryOperator),
    Unary(UnaryOperator),
    Math(MathFunction),
    /// A conversion to the kind, with the value converted.
    Cast(ScalarKind),
    /// A comparison of each component, then all or any of them.
    Reduce(BinaryOperator, RelationalFunction),
}

fn alu_form(op: Alu) -> Option<(ScalarKind, AluForm)> {
    use AluForm as F;
    use BinaryOperator as B;
    use MathFunction as M;
    use ScalarKind::{Bool, Float, Sint, Uint};
    Some(match op {
        Alu::Fadd => (Float, F::Binary(B::Add)),
        Alu::Fsub => (Float, F::Binary(B::Subtract)),
        Alu::Fmul => (Float, F::Binary(B::Multiply)),
        Alu::Fdiv => (Float, F::Binary(B::Divide)),
        Alu::Flt => (Float, F::Binary(B::Less)),
        Alu::Fge => (Float, F::Binary(B::GreaterEqual)),
        Alu::Feq => (Float, F::Binary(B::Equal)),
        Alu::Fneu => (Float, F::Binary(B::NotEqual)),
        Alu::Fneg => (Float, F::Unary(UnaryOperator::Negate)),
        Alu::Fabs => (Float, F::Math(M::Abs)),
        Alu::Fsign => (Float, F::Math(M::Sign)),
        Alu::Fmin => (Float, F::Math(M::Min)),
        Alu::Fmax => (Float, F::Math(M::Max)),
        Alu::Ffloor => (Float, F::Math(M::Floor)),
        Alu::Fceil => (Float, F::Math(M::Ceil)),
        Alu::Ffract => (Float, F::Math(M::Fract)),
        Alu::Ftrunc => (Float, F::Math(M::Trunc)),
        Alu::FroundEven => (Float, F::Math(M::Round)),
        Alu::Fsqrt => (Float, F::Math(M::Sqrt)),
        Alu::Frsq => (Float, F::Math(M::InverseSqrt)),
        Alu::Fexp2 => (Float, F::Math(M::Exp2)),
        Alu::Flog2 => (Float, F::Math(M::Log2)),
        Alu::Fpow => (Float, F::Math(M::Pow)),
        Alu::Fsin => (Float, F::Math(M::Sin)),
        Alu::Fcos => (Float, F::Math(M::Cos)),
        Alu::Ffma | Alu::FfmaWeak => (Float, F::Math(M::Fma)),
        Alu::Flrp => (Float, F::Math(M::Mix)),
        Alu::Iadd => (Sint, F::Binary(B::Add)),
        Alu::Isub => (Sint, F::Binary(B::Subtract)),
        Alu::Imul => (Sint, F::Binary(B::Multiply)),
        Alu::Idiv => (Sint, F::Binary(B::Divide)),
        Alu::Irem => (Sint, F::Binary(B::Modulo)),
        Alu::Udiv => (Uint, F::Binary(B::Divide)),
        Alu::Umod => (Uint, F::Binary(B::Modulo)),
        Alu::Ineg => (Sint, F::Unary(UnaryOperator::Negate)),
        Alu::Iabs => (Sint, F::Math(M::Abs)),
        Alu::Imin => (Sint, F::Math(M::Min)),
        Alu::Imax => (Sint, F::Math(M::Max)),
        Alu::Umin => (Uint, F::Math(M::Min)),
        Alu::Umax => (Uint, F::Math(M::Max)),
        Alu::Ishl => (Sint, F::Binary(B::ShiftLeft)),
        Alu::Ishr => (Sint, F::Binary(B::ShiftRight)),
        Alu::Ushr => (Uint, F::Binary(B::ShiftRight)),
        Alu::Iand => (Uint, F::Binary(B::And)),
        Alu::Ior => (Uint, F::Binary(B::InclusiveOr)),
        Alu::Ixor => (Uint, F::Binary(B::ExclusiveOr)),
        Alu::Inot => (Uint, F::Unary(UnaryOperator::BitwiseNot)),
        Alu::Ilt => (Sint, F::Binary(B::Less)),
        Alu::Ige => (Sint, F::Binary(B::GreaterEqual)),
        Alu::Ieq => (Sint, F::Binary(B::Equal)),
        Alu::Ine => (Sint, F::Binary(B::NotEqual)),
        Alu::Ult => (Uint, F::Binary(B::Less)),
        Alu::Uge => (Uint, F::Binary(B::GreaterEqual)),
        Alu::I2f32 => (Sint, F::Cast(Float)),
        Alu::U2f32 => (Uint, F::Cast(Float)),
        Alu::F2i32 => (Float, F::Cast(Sint)),
        Alu::F2u32 => (Float, F::Cast(Uint)),
        Alu::B2f32 => (Bool, F::Cast(Float)),
        Alu::B2i32 => (Bool, F::Cast(Sint)),
        Alu::BitCount => (Uint, F::Math(M::CountOneBits)),
        Alu::UfindMsb => (Uint, F::Math(M::FirstLeadingBit)),
        Alu::IfindMsb => (Sint, F::Math(M::FirstLeadingBit)),
        Alu::FindLsb => (Uint, F::Math(M::FirstTrailingBit)),
        Alu::BitfieldReverse => (Uint, F::Math(M::ReverseBits)),
        Alu::PackHalf2x16 => (Float, F::Math(M::Pack2x16float)),
        Alu::UnpackHalf2x16 => (Uint, F::Math(M::Unpack2x16float)),
        Alu::PackUnorm4x8 => (Float, F::Math(M::Pack4x8unorm)),
        Alu::UnpackUnorm4x8 => (Uint, F::Math(M::Unpack4x8unorm)),
        Alu::PackSnorm4x8 => (Float, F::Math(M::Pack4x8snorm)),
        Alu::UnpackSnorm4x8 => (Uint, F::Math(M::Unpack4x8snorm)),
        Alu::PackUnorm2x16 => (Float, F::Math(M::Pack2x16unorm)),
        Alu::UnpackUnorm2x16 => (Uint, F::Math(M::Unpack2x16unorm)),
        Alu::BallFequal2 | Alu::BallFequal3 | Alu::BallFequal4 => {
            (Float, F::Reduce(B::Equal, RelationalFunction::All))
        }
        Alu::BanyFnequal2 | Alu::BanyFnequal3 | Alu::BanyFnequal4 => {
            (Float, F::Reduce(B::NotEqual, RelationalFunction::Any))
        }
        Alu::BallIequal2 | Alu::BallIequal3 | Alu::BallIequal4 => {
            (Sint, F::Reduce(B::Equal, RelationalFunction::All))
        }
        Alu::BanyInequal2 | Alu::BanyInequal3 | Alu::BanyInequal4 => {
            (Sint, F::Reduce(B::NotEqual, RelationalFunction::Any))
        }
        _ => return None,
    })
}

/// The width of an all-or-any comparison, from its place among its three sizes.
fn reduce_size(op: Alu) -> u32 {
    let first = match op {
        Alu::BallFequal2 | Alu::BallFequal3 | Alu::BallFequal4 => Alu::BallFequal2,
        Alu::BanyFnequal2 | Alu::BanyFnequal3 | Alu::BanyFnequal4 => Alu::BanyFnequal2,
        Alu::BallIequal2 | Alu::BallIequal3 | Alu::BallIequal4 => Alu::BallIequal2,
        _ => Alu::BanyInequal2,
    };
    op as u32 - first as u32 + VectorSize::Bi as u32
}

/// The components each source of an operation takes when they are not the result's.
fn source_count(op: Alu, components: u32) -> u32 {
    match op {
        Alu::Vec2 | Alu::Vec3 | Alu::Vec4 => 1,
        Alu::Fdot2 => VectorSize::Bi as u32,
        Alu::Fdot3 => VectorSize::Tri as u32,
        Alu::Fdot4 => VectorSize::Quad as u32,
        Alu::PackHalf2x16 | Alu::PackUnorm2x16 => VectorSize::Bi as u32,
        Alu::PackUnorm4x8 | Alu::PackSnorm4x8 => VectorSize::Quad as u32,
        Alu::UnpackHalf2x16 | Alu::UnpackUnorm4x8 | Alu::UnpackSnorm4x8 | Alu::UnpackUnorm2x16 => 1,
        Alu::BallFequal2
        | Alu::BallFequal3
        | Alu::BallFequal4
        | Alu::BanyFnequal2
        | Alu::BanyFnequal3
        | Alu::BanyFnequal4
        | Alu::BallIequal2
        | Alu::BallIequal3
        | Alu::BallIequal4
        | Alu::BanyInequal2
        | Alu::BanyInequal3
        | Alu::BanyInequal4 => reduce_size(op),
        _ => components,
    }
}

fn alu_op(raw: u32) -> Alu {
    if raw < Alu::Unsupported as u32 {
        // SAFETY: Alu is repr(u32) with every value below Unsupported a variant, in the bridge's order.
        unsafe { std::mem::transmute::<u32, Alu>(raw) }
    } else {
        Alu::Unsupported
    }
}

fn intrinsic_op(raw: u32) -> Intrinsic {
    if raw < Intrinsic::Unsupported as u32 {
        // SAFETY: as alu_op, for Intrinsic.
        unsafe { std::mem::transmute::<u32, Intrinsic>(raw) }
    } else {
        Intrinsic::Unsupported
    }
}

impl Builder<'_> {
    fn instr(&mut self, i: &FlatInstr) -> Result<(), ConvertError> {
        match i.kind {
            Kind::Alu => self.alu(i),
            Kind::Const => {
                self.set(i, Value::Const(i.value, i.components, i.bits == BOOL_BITS));
                Ok(())
            }
            Kind::Undef => {
                let kind = if i.bits == BOOL_BITS {
                    ScalarKind::Bool
                } else {
                    ScalarKind::Uint
                };
                let inner = if i.components == 1 {
                    TypeInner::Scalar(Scalar {
                        kind,
                        width: Self::width(kind),
                    })
                } else {
                    TypeInner::Vector {
                        size: vector_size(i.components)?,
                        scalar: Scalar {
                            kind,
                            width: Self::width(kind),
                        },
                    }
                };
                let ty = self.add_type(inner);
                let e = self.add(Expression::ZeroValue(ty));
                self.set(i, Value::Expr(e, kind));
                Ok(())
            }
            Kind::Deref => self.deref(i),
            Kind::Intrinsic => self.intrinsic(i),
            Kind::Tex => self.tex(i),
            Kind::Break => {
                self.statement(Statement::Break);
                Ok(())
            }
            Kind::Continue => {
                self.statement(Statement::Continue);
                Ok(())
            }
            Kind::Return => {
                self.ret();
                Ok(())
            }
            Kind::If => {
                let (condition, _) = self.whole(i.src[0].def, ScalarKind::Bool)?;
                self.flush();
                self.open.push(Open::If(condition, None));
                self.blocks.push(Block::new());
                Ok(())
            }
            Kind::Else => {
                self.flush();
                let accept = self.blocks.pop().expect("an open block");
                if let Some(Open::If(_, slot)) = self.open.last_mut() {
                    *slot = Some(accept);
                }
                self.blocks.push(Block::new());
                Ok(())
            }
            Kind::EndIf => {
                self.flush();
                let reject = self.blocks.pop().expect("an open block");
                let Some(Open::If(condition, Some(accept))) = self.open.pop() else {
                    return Err(unsupported(format_args!("an if the walk did not open")));
                };
                self.statement(Statement::If {
                    condition,
                    accept,
                    reject,
                });
                Ok(())
            }
            Kind::Loop => {
                self.flush();
                self.open.push(Open::Loop);
                self.blocks.push(Block::new());
                Ok(())
            }
            Kind::EndLoop => {
                self.flush();
                let body = self.blocks.pop().expect("an open block");
                self.open.pop();
                self.statement(Statement::Loop {
                    body,
                    continuing: Block::new(),
                    break_if: None,
                });
                Ok(())
            }
            Kind::Unsupported => Err(unsupported(name(i))),
        }
    }

    fn scalar_type(
        &mut self,
        kind: ScalarKind,
        components: u32,
    ) -> Result<Handle<Type>, ConvertError> {
        let scalar = Scalar {
            kind,
            width: Self::width(kind),
        };
        Ok(if components == 1 {
            self.add_type(TypeInner::Scalar(scalar))
        } else {
            self.add_type(TypeInner::Vector {
                size: vector_size(components)?,
                scalar,
            })
        })
    }

    fn alu(&mut self, i: &FlatInstr) -> Result<(), ConvertError> {
        let op = alu_op(i.op);
        let n = i.components;
        let (e, kind) = match op {
            Alu::Mov => {
                let kind = self.natural(i.src[0].def);
                (self.src(i, 0, n, kind)?, kind)
            }
            Alu::Vec2 | Alu::Vec3 | Alu::Vec4 => {
                let kind = self.natural(i.src[0].def);
                let parts = (0..n as usize)
                    .map(|s| self.src(i, s, 1, kind))
                    .collect::<Result<Vec<_>, _>>()?;
                let ty = self.scalar_type(kind, n)?;
                (
                    self.add(Expression::Compose {
                        ty,
                        components: parts,
                    }),
                    kind,
                )
            }
            Alu::Bcsel => {
                let kind = self.natural(i.src[1].def);
                let condition = self.src(i, 0, n, ScalarKind::Bool)?;
                let accept = self.src(i, 1, n, kind)?;
                let reject = self.src(i, 2, n, kind)?;
                (
                    self.add(Expression::Select {
                        condition,
                        accept,
                        reject,
                    }),
                    kind,
                )
            }
            Alu::Frcp => {
                let x = self.src(i, 0, n, ScalarKind::Float)?;
                let one = self.add(Expression::Literal(Literal::F32(1.0)));
                let one = if n == 1 {
                    one
                } else {
                    self.add(Expression::Splat {
                        size: vector_size(n)?,
                        value: one,
                    })
                };
                let e = self.add(Expression::Binary {
                    op: BinaryOperator::Divide,
                    left: one,
                    right: x,
                });
                (e, ScalarKind::Float)
            }
            Alu::Fdot2 | Alu::Fdot3 | Alu::Fdot4 => {
                let count = source_count(op, n);
                let a = self.src(i, 0, count, ScalarKind::Float)?;
                let b = self.src(i, 1, count, ScalarKind::Float)?;
                let e = self.add(Expression::Math {
                    fun: MathFunction::Dot,
                    arg: a,
                    arg1: Some(b),
                    arg2: None,
                    arg3: None,
                });
                (e, ScalarKind::Float)
            }
            _ => {
                let (source_kind, form) = alu_form(op).ok_or_else(|| unsupported(name(i)))?;
                let boolean = i.bits == BOOL_BITS
                    && matches!(op, Alu::Iand | Alu::Ior | Alu::Ixor | Alu::Inot);
                let source_kind = if boolean {
                    ScalarKind::Bool
                } else {
                    source_kind
                };
                let count = |_: usize| source_count(op, n);
                let a = self.src(i, 0, count(0), source_kind)?;
                match form {
                    AluForm::Binary(o) => {
                        let right_kind = match o {
                            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
                                ScalarKind::Uint
                            }
                            _ => source_kind,
                        };
                        let b = self.src(i, 1, count(1), right_kind)?;
                        let comparison = matches!(
                            o,
                            BinaryOperator::Less
                                | BinaryOperator::GreaterEqual
                                | BinaryOperator::Equal
                                | BinaryOperator::NotEqual
                        );
                        let e = self.add(Expression::Binary {
                            op: o,
                            left: a,
                            right: b,
                        });
                        (
                            e,
                            if comparison {
                                ScalarKind::Bool
                            } else {
                                source_kind
                            },
                        )
                    }
                    AluForm::Unary(o) => {
                        let o = if boolean {
                            UnaryOperator::LogicalNot
                        } else {
                            o
                        };
                        (self.add(Expression::Unary { op: o, expr: a }), source_kind)
                    }
                    AluForm::Math(fun) => {
                        let inputs = i.sources;
                        let arg1 = if inputs > 1 {
                            Some(self.src(i, 1, count(1), source_kind)?)
                        } else {
                            None
                        };
                        let arg2 = if inputs > 2 {
                            Some(self.src(i, 2, count(2), source_kind)?)
                        } else {
                            None
                        };
                        let e = self.add(Expression::Math {
                            fun,
                            arg: a,
                            arg1,
                            arg2,
                            arg3: None,
                        });
                        let kind = match fun {
                            MathFunction::Pack2x16float
                            | MathFunction::Pack4x8unorm
                            | MathFunction::Pack4x8snorm
                            | MathFunction::Pack2x16unorm => ScalarKind::Uint,
                            MathFunction::Unpack2x16float
                            | MathFunction::Unpack4x8unorm
                            | MathFunction::Unpack4x8snorm
                            | MathFunction::Unpack2x16unorm => ScalarKind::Float,
                            _ => source_kind,
                        };
                        (e, kind)
                    }
                    AluForm::Cast(to) => (
                        self.add(Expression::As {
                            expr: a,
                            kind: to,
                            convert: Some(WIDTH),
                        }),
                        to,
                    ),
                    AluForm::Reduce(o, fun) => {
                        let b = self.src(i, 1, count(1), source_kind)?;
                        let each = self.add(Expression::Binary {
                            op: o,
                            left: a,
                            right: b,
                        });
                        (
                            self.add(Expression::Relational {
                                fun,
                                argument: each,
                            }),
                            ScalarKind::Bool,
                        )
                    }
                }
            }
        };
        self.set(i, Value::Expr(e, kind));
        Ok(())
    }

    /// The kind a value already has (constants and registers take the use's; u32 here).
    fn natural(&self, def: u32) -> ScalarKind {
        match self.value(def) {
            Ok(Value::Expr(_, k)) => k,
            Ok(Value::Const(_, _, true) | Value::Register(_, true)) => ScalarKind::Bool,
            _ => ScalarKind::Uint,
        }
    }

    fn pointer(&self, def: u32) -> Result<(Handle<Expression>, u32), ConvertError> {
        match self.value(def)? {
            Value::Pointer(e, ty) => Ok((e, ty)),
            _ => Err(unsupported(format_args!(
                "a pointer that is not a variable's"
            ))),
        }
    }

    fn deref(&mut self, i: &FlatInstr) -> Result<(), ConvertError> {
        // SAFETY: the bridge writes only Deref's values here.
        let kind = unsafe { std::mem::transmute::<u32, Deref>(i.op) };
        let e = match kind {
            Deref::Var => self.places[i.value[0] as usize]
                .ok_or_else(|| unsupported(format_args!("a variable the module does not have")))?,
            Deref::Array => {
                let (base, _) = self.pointer(i.src[0].def)?;
                match self.value(i.src[1].def)? {
                    Value::Const(v, _, _) => {
                        self.add(Expression::AccessIndex { base, index: v[0] })
                    }
                    _ => {
                        let index = self.src(i, 1, 1, ScalarKind::Uint)?;
                        self.add(Expression::Access { base, index })
                    }
                }
            }
            Deref::Struct => {
                let (base, _) = self.pointer(i.src[0].def)?;
                self.add(Expression::AccessIndex {
                    base,
                    index: i.value[0],
                })
            }
            Deref::Cast => {
                let Value::Resource(set, binding, index) = self.value(i.src[0].def)? else {
                    return Err(unsupported(format_args!(
                        "a cast of something other than a buffer"
                    )));
                };
                let global = self
                    .module
                    .global_variables
                    .iter()
                    .find(|(_, g)| {
                        g.binding
                            == Some(naga::ResourceBinding {
                                group: set,
                                binding,
                            })
                    })
                    .map(|(h, g)| (h, g.ty))
                    .ok_or_else(|| {
                        unsupported(format_args!("a buffer the module does not declare"))
                    })?;
                let base = self.add(Expression::GlobalVariable(global.0));
                if matches!(
                    self.module.types[global.1].inner,
                    TypeInner::BindingArray { .. }
                ) {
                    let index = match self.value(index)? {
                        Value::Const(v, _, _) => self.add(Expression::Literal(Literal::U32(v[0]))),
                        _ => self.whole(index, ScalarKind::Uint)?.0,
                    };
                    self.add(Expression::Access { base, index })
                } else {
                    base
                }
            }
        };
        self.set(i, Value::Pointer(e, i.ty));
        Ok(())
    }

    fn ret(&mut self) {
        let value = if self.outputs.is_empty() || self.stage == ShaderStage::Compute {
            None
        } else {
            let outputs = self.outputs.clone();
            let mut members = Vec::new();
            let mut parts = Vec::new();
            let mut end = 0;
            for (local, ty, binding) in outputs {
                let pointer = self.add(Expression::LocalVariable(local));
                if self.stage == ShaderStage::Vertex
                    && matches!(binding, Binding::BuiltIn(BuiltIn::Position { .. }))
                {
                    self.flip_y(pointer);
                }
                parts.push(self.add(Expression::Load { pointer }));
                let layout = self.layout(ty).expect("a stage value lays out");
                let offset = layout.alignment.round_up(end);
                end = offset + layout.size;
                members.push(naga::StructMember {
                    name: None,
                    ty,
                    binding: Some(binding),
                    offset,
                });
            }
            let ty = self.add_type(TypeInner::Struct {
                members,
                span: naga::proc::Alignment::SIXTEEN.round_up(end),
            });
            self.function.result = Some(naga::FunctionResult { ty, binding: None });
            Some(self.add(Expression::Compose {
                ty,
                components: parts,
            }))
        };
        self.statement(Statement::Return { value });
    }

    /// The common form's clip space has y upwards (WGSL's), Vulkan's downwards: the position's y
    /// is negated on its way out, as naga's SPIR-V reader does for candidate A.
    fn flip_y(&mut self, position: Handle<Expression>) {
        let pointer = self.add(Expression::AccessIndex {
            base: position,
            index: naga::SwizzleComponent::Y as u32,
        });
        let y = self.add(Expression::Load { pointer });
        let value = self.add(Expression::Unary {
            op: UnaryOperator::Negate,
            expr: y,
        });
        self.statement(Statement::Store { pointer, value });
    }

    fn finish(mut self, f: &Flat) -> Result<Module, ConvertError> {
        if !self.open.is_empty() {
            return Err(unsupported(format_args!(
                "control flow the walk did not close"
            )));
        }
        self.ret();
        let _ = self.emitter.finish(&self.function.expressions);
        self.function.body = self.blocks.pop().expect("the body");
        self.module.entry_points.push(naga::EntryPoint {
            name: wcontainers_shaders::ENTRY_POINT.into(),
            stage: self.stage,
            early_depth_test: (f.early_fragment_tests != 0).then_some(naga::EarlyDepthTest::Force),
            workgroup_size: if self.stage == ShaderStage::Compute {
                f.workgroup_size
            } else {
                WorkgroupSize::default()
            },
            workgroup_size_overrides: None,
            function: self.function,
            mesh_info: None,
            task_payload: None,
            incoming_ray_payload: None,
        });
        Ok(self.module)
    }
}

impl Builder<'_> {
    fn system_value(
        &mut self,
        i: &FlatInstr,
        builtin: BuiltIn,
        kind: ScalarKind,
    ) -> Result<(), ConvertError> {
        let inner = if i.components == 1 {
            TypeInner::Scalar(Scalar {
                kind,
                width: Self::width(kind),
            })
        } else {
            TypeInner::Vector {
                size: vector_size(i.components)?,
                scalar: Scalar {
                    kind,
                    width: Self::width(kind),
                },
            }
        };
        let e = self.builtin(builtin, inner);
        self.set(i, Value::Expr(e, kind));
        Ok(())
    }

    /// Stores `value` (of `components`) through `pointer` where `mask` says, the whole value at once
    /// when it says all.
    fn store(
        &mut self,
        pointer: Handle<Expression>,
        value: Handle<Expression>,
        components: u32,
        mask: u32,
    ) {
        let all = (1 << components) - 1;
        if components == 1 || mask & all == all {
            self.statement(Statement::Store { pointer, value });
            return;
        }
        for c in (0..components).filter(|c| mask & (1 << c) != 0) {
            let pointer = self.add(Expression::AccessIndex {
                base: pointer,
                index: c,
            });
            let value = self.add(Expression::AccessIndex {
                base: value,
                index: c,
            });
            self.statement(Statement::Store { pointer, value });
        }
    }

    fn intrinsic(&mut self, i: &FlatInstr) -> Result<(), ConvertError> {
        use ScalarKind::{Bool, Float, Uint};
        match intrinsic_op(i.op) {
            Intrinsic::LoadDeref => {
                let (pointer, ty) = self.pointer(i.src[0].def)?;
                let kind = self.kind_of(ty);
                let e = self.add(Expression::Load { pointer });
                self.set(i, Value::Expr(e, kind));
            }
            Intrinsic::StoreDeref => {
                let (pointer, ty) = self.pointer(i.src[0].def)?;
                let kind = self.kind_of(ty);
                let (value, components) = self.whole(i.src[1].def, kind)?;
                self.store(pointer, value, components, i.value[2]);
            }
            Intrinsic::DeclReg => {
                let boolean = i.bits == BOOL_BITS;
                let ty = self.scalar_type(if boolean { Bool } else { Uint }, i.components)?;
                let local = self.function.local_variables.append(
                    LocalVariable {
                        name: None,
                        ty,
                        init: None,
                    },
                    Span::UNDEFINED,
                );
                self.set(i, Value::Register(local, boolean));
            }
            Intrinsic::LoadReg => {
                let Value::Register(local, boolean) = self.value(i.src[0].def)? else {
                    return Err(unsupported(format_args!(
                        "a register load from no register"
                    )));
                };
                let pointer = self.add(Expression::LocalVariable(local));
                let e = self.add(Expression::Load { pointer });
                self.set(i, Value::Expr(e, if boolean { Bool } else { Uint }));
            }
            Intrinsic::StoreReg => {
                let target = self.value(i.src[1].def)?;
                let Value::Register(local, boolean) = target else {
                    return Err(unsupported(format_args!("a register store to {target:?}")));
                };
                let kind = if boolean { Bool } else { Uint };
                let (value, components) = self.whole(i.src[0].def, kind)?;
                let pointer = self.add(Expression::LocalVariable(local));
                self.store(pointer, value, components, i.value[2]);
            }
            Intrinsic::VulkanResourceIndex => {
                self.set(i, Value::Resource(i.value[0], i.value[1], i.src[0].def));
            }
            Intrinsic::LoadVulkanDescriptor => {
                let v = self.value(i.src[0].def)?;
                self.set(i, v);
            }
            Intrinsic::LoadVertexId => self.system_value(i, BuiltIn::VertexIndex, Uint)?,
            Intrinsic::LoadInstanceId => self.system_value(i, BuiltIn::InstanceIndex, Uint)?,
            Intrinsic::LoadFrontFace => self.system_value(i, BuiltIn::FrontFacing, Bool)?,
            Intrinsic::LoadFragCoord => {
                self.system_value(i, BuiltIn::Position { invariant: false }, Float)?;
            }
            Intrinsic::LoadSampleId => self.system_value(i, BuiltIn::SampleIndex, Uint)?,
            Intrinsic::LoadLocalInvocationId => {
                self.system_value(i, BuiltIn::LocalInvocationId, Uint)?;
            }
            Intrinsic::LoadGlobalInvocationId => {
                self.system_value(i, BuiltIn::GlobalInvocationId, Uint)?;
            }
            Intrinsic::LoadWorkgroupId => self.system_value(i, BuiltIn::WorkGroupId, Uint)?,
            Intrinsic::LoadNumWorkgroups => self.system_value(i, BuiltIn::NumWorkGroups, Uint)?,
            Intrinsic::LoadLocalInvocationIndex => {
                self.system_value(i, BuiltIn::LocalInvocationIndex, Uint)?;
            }
            Intrinsic::Demote | Intrinsic::Terminate => self.statement(Statement::Kill),
            Intrinsic::DemoteIf | Intrinsic::TerminateIf => {
                let (condition, _) = self.whole(i.src[0].def, Bool)?;
                let mut accept = Block::new();
                accept.push(Statement::Kill, Span::UNDEFINED);
                self.statement(Statement::If {
                    condition,
                    accept,
                    reject: Block::new(),
                });
            }
            Intrinsic::Barrier => {
                let mut flags = naga::Barrier::empty();
                let [execution, shared, storage, ..] = i.value;
                flags.set(naga::Barrier::WORK_GROUP, shared != 0);
                flags.set(naga::Barrier::STORAGE, storage != 0);
                if execution != 0 {
                    self.statement(Statement::ControlBarrier(flags));
                } else if !flags.is_empty() {
                    self.statement(Statement::MemoryBarrier(flags));
                }
            }
            Intrinsic::DerefBufferArrayLength => {
                let (pointer, _) = self.pointer(i.src[0].def)?;
                let e = self.add(Expression::ArrayLength(pointer));
                self.set(i, Value::Expr(e, Uint));
            }
            Intrinsic::Ddx
            | Intrinsic::Ddy
            | Intrinsic::DdxFine
            | Intrinsic::DdyFine
            | Intrinsic::DdxCoarse
            | Intrinsic::DdyCoarse => {
                use naga::{DerivativeAxis as X, DerivativeControl as C};
                let (axis, ctrl) = match intrinsic_op(i.op) {
                    Intrinsic::Ddx => (X::X, C::None),
                    Intrinsic::Ddy => (X::Y, C::None),
                    Intrinsic::DdxFine => (X::X, C::Fine),
                    Intrinsic::DdyFine => (X::Y, C::Fine),
                    Intrinsic::DdxCoarse => (X::X, C::Coarse),
                    _ => (X::Y, C::Coarse),
                };
                let (expr, _) = self.whole(i.src[0].def, Float)?;
                let e = self.add(Expression::Derivative { axis, ctrl, expr });
                self.set(i, Value::Expr(e, Float));
            }
            Intrinsic::Unsupported => return Err(unsupported(name(i))),
        }
        Ok(())
    }

    fn tex_src(&mut self, i: &FlatInstr, at: TexSrc) -> Option<u32> {
        let def = i.tex[at as usize];
        (def != NONE).then_some(def)
    }

    fn tex(&mut self, i: &FlatInstr) -> Result<(), ConvertError> {
        use ScalarKind::{Float, Sint, Uint};
        // SAFETY: the bridge writes only TexOp's values here.
        let op = unsafe { std::mem::transmute::<u32, TexOp>(i.op) };
        let texture = self.tex_src(i, TexSrc::Texture).ok_or_else(|| {
            unsupported(format_args!(
                "a texture instruction without a texture deref"
            ))
        })?;
        let (image, _) = self.pointer(texture)?;
        let dim = dimension(i.dim)?;
        let space = match dim {
            ImageDimension::D1 => 1,
            ImageDimension::D2 => VectorSize::Bi as u32,
            ImageDimension::D3 | ImageDimension::Cube => VectorSize::Tri as u32,
        };
        let arrayed = i.arrayed != 0;
        let result_kind = scalar_of(match i.ty {
            t if t == FlatScalar::Float as u32 => FlatScalar::Float,
            t if t == FlatScalar::Sint as u32 => FlatScalar::Sint,
            _ => FlatScalar::Uint,
        })?
        .kind;
        // The coordinate split into the position and the layer.
        let coordinate =
            |b: &mut Self,
             kind: ScalarKind|
             -> Result<(Handle<Expression>, Option<Handle<Expression>>), ConvertError> {
                let def = b.tex_src(i, TexSrc::Coord).ok_or_else(|| {
                    unsupported(format_args!("a texture instruction without coordinates"))
                })?;
                let mut instr = *i;
                instr.src[0] = FlatSrc {
                    def,
                    swizzle: std::array::from_fn(|c| u8::try_from(c).expect("a component")),
                };
                let position = b.src(&instr, 0, space, kind)?;
                let layer = if arrayed {
                    instr.src[0].swizzle[0] = u8::try_from(space).expect("a component");
                    let layer = b.src(&instr, 0, 1, kind)?;
                    Some(b.convert_value(layer, kind, Sint))
                } else {
                    None
                };
                Ok((position, layer))
            };
        let scalar_src = |b: &mut Self,
                          at: TexSrc,
                          kind: ScalarKind|
         -> Result<Option<Handle<Expression>>, ConvertError> {
            match b.tex_src(i, at) {
                Some(def) => Ok(Some(b.whole(def, kind)?.0)),
                None => Ok(None),
            }
        };
        if i.shadow != 0 {
            self.make_depth(image);
        }
        let e = match op {
            TexOp::Sample | TexOp::Bias | TexOp::Level | TexOp::Gradient | TexOp::Gather => {
                let sampler = self
                    .tex_src(i, TexSrc::Sampler)
                    .ok_or_else(|| unsupported(format_args!("a sampling without a sampler")))?;
                let (sampler, _) = self.pointer(sampler)?;
                let (coordinate, array_index) = coordinate(self, Float)?;
                let depth_ref = scalar_src(self, TexSrc::Comparator, Float)?;
                let level = match op {
                    TexOp::Gather => SampleLevel::Zero,
                    TexOp::Sample => SampleLevel::Auto,
                    TexOp::Bias => SampleLevel::Bias(
                        scalar_src(self, TexSrc::Bias, Float)?
                            .ok_or_else(|| unsupported(format_args!("a bias")))?,
                    ),
                    TexOp::Level => match self.tex_src(i, TexSrc::Lod).map(|d| self.value(d)) {
                        Some(Ok(Value::Const(v, _, _))) if v[0] == 0 => SampleLevel::Zero,
                        _ if depth_ref.is_some() => {
                            return Err(unsupported(format_args!(
                                "a comparison at a level other than 0"
                            )));
                        }
                        _ => SampleLevel::Exact(
                            scalar_src(self, TexSrc::Lod, Float)?
                                .ok_or_else(|| unsupported(format_args!("a level")))?,
                        ),
                    },
                    _ => {
                        let x = self
                            .tex_src(i, TexSrc::Ddx)
                            .ok_or_else(|| unsupported(format_args!("a gradient")))?;
                        let y = self
                            .tex_src(i, TexSrc::Ddy)
                            .ok_or_else(|| unsupported(format_args!("a gradient")))?;
                        SampleLevel::Gradient {
                            x: self.whole(x, Float)?.0,
                            y: self.whole(y, Float)?.0,
                        }
                    }
                };
                let offset = match self.tex_src(i, TexSrc::Offset) {
                    None => None,
                    Some(def) => match self.value(def)? {
                        Value::Const(v, n, _) => {
                            Some(self.constant_vector(&v[..n as usize], Sint)?)
                        }
                        _ => {
                            return Err(unsupported(format_args!(
                                "an offset known only at run time"
                            )));
                        }
                    },
                };
                let gather = (op == TexOp::Gather)
                    .then(|| naga::SwizzleComponent::XYZW[i.component as usize % VectorSize::MAX]);
                self.add(Expression::ImageSample {
                    image,
                    sampler,
                    gather,
                    coordinate,
                    array_index,
                    offset,
                    level,
                    depth_ref,
                    clamp_to_edge: false,
                })
            }
            TexOp::Load => {
                let (coordinate, array_index) = coordinate(self, Sint)?;
                let sample = scalar_src(self, TexSrc::Sample, Sint)?;
                let level = if sample.is_some() {
                    None
                } else {
                    scalar_src(self, TexSrc::Lod, Sint)?
                };
                self.add(Expression::ImageLoad {
                    image,
                    coordinate,
                    array_index,
                    sample,
                    level,
                })
            }
            TexOp::Size => {
                let level = scalar_src(self, TexSrc::Lod, Sint)?;
                let size = self.add(Expression::ImageQuery {
                    image,
                    query: naga::ImageQuery::Size { level },
                });
                if arrayed {
                    let layers = self.add(Expression::ImageQuery {
                        image,
                        query: naga::ImageQuery::NumLayers,
                    });
                    let parts = if space == 1 {
                        vec![size, layers]
                    } else {
                        let mut parts: Vec<_> = (0..space)
                            .map(|c| {
                                self.add(Expression::AccessIndex {
                                    base: size,
                                    index: c,
                                })
                            })
                            .collect();
                        parts.push(layers);
                        parts
                    };
                    let ty = self.scalar_type(Uint, space + 1)?;
                    self.add(Expression::Compose {
                        ty,
                        components: parts,
                    })
                } else {
                    size
                }
            }
            TexOp::Levels => self.add(Expression::ImageQuery {
                image,
                query: naga::ImageQuery::NumLevels,
            }),
        };
        let kind = match op {
            TexOp::Size | TexOp::Levels => Uint,
            _ => result_kind,
        };
        let shadow = i.shadow != 0 && op != TexOp::Gather;
        let e = if !shadow && i.components == 1 && !matches!(op, TexOp::Size | TexOp::Levels) {
            self.add(Expression::AccessIndex { base: e, index: 0 })
        } else {
            e
        };
        self.set(i, Value::Expr(e, kind));
        Ok(())
    }

    /// A texture sampled with a comparison is a depth texture in the common form; NIR's texture
    /// types (split from combined image samplers) do not say so, its sampling does.
    fn make_depth(&mut self, image: Handle<Expression>) {
        let global = match self.function.expressions[image] {
            Expression::GlobalVariable(g) => Some(g),
            Expression::Access { base, .. } => match self.function.expressions[base] {
                Expression::GlobalVariable(g) => Some(g),
                _ => None,
            },
            _ => None,
        };
        let Some(global) = global else { return };
        let ty = self.module.global_variables[global].ty;
        let depth = |inner: &TypeInner| match *inner {
            TypeInner::Image {
                dim,
                arrayed,
                class: ImageClass::Sampled { multi, .. },
            } => Some(TypeInner::Image {
                dim,
                arrayed,
                class: ImageClass::Depth { multi },
            }),
            _ => None,
        };
        let new = match self.module.types[ty].inner {
            TypeInner::BindingArray { base, size } => {
                depth(&self.module.types[base].inner).map(|inner| {
                    let base = self.add_type(inner);
                    self.add_type(TypeInner::BindingArray { base, size })
                })
            }
            ref inner => depth(inner).map(|inner| self.add_type(inner)),
        };
        if let Some(new) = new {
            self.module.global_variables[global].ty = new;
        }
    }

    fn convert_value(
        &mut self,
        e: Handle<Expression>,
        from: ScalarKind,
        to: ScalarKind,
    ) -> Handle<Expression> {
        if from == to {
            e
        } else {
            self.add(Expression::As {
                expr: e,
                kind: to,
                convert: Some(WIDTH),
            })
        }
    }

    /// A constant vector of the module, usable where naga wants a constant expression.
    fn constant_vector(
        &mut self,
        values: &[u32],
        kind: ScalarKind,
    ) -> Result<Handle<Expression>, ConvertError> {
        let parts: Vec<_> = values
            .iter()
            .map(|&v| {
                self.module
                    .global_expressions
                    .append(Expression::Literal(Self::literal(kind, v)), Span::UNDEFINED)
            })
            .collect();
        let n = u32::try_from(values.len()).expect("a few components");
        let ty = self.scalar_type(kind, n)?;
        let init = if n == 1 {
            parts[0]
        } else {
            self.module.global_expressions.append(
                Expression::Compose {
                    ty,
                    components: parts,
                },
                Span::UNDEFINED,
            )
        };
        let constant = self.module.constants.append(
            naga::Constant {
                name: None,
                ty,
                init,
            },
            Span::UNDEFINED,
        );
        Ok(self.add(Expression::Constant(constant)))
    }
}
