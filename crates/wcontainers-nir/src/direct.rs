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
use wcontainers_gpu::{
    ReflectedBinding, Reflection, ShaderModule, SlotKind, SourceStage, StageExtras, VariantKey,
};
use wcontainers_shaders::cache;

const NONE: u32 = u32::MAX;
/// A workgroup's size along x, y and z, as naga's entry points hold it.
type WorkgroupSize = [u32; VectorSize::Tri as usize];
const SOURCES: usize = 5;
/// The source of an image store holding the value (after the image, coordinate and sample).
const IMAGE_STORE_VALUE: usize = 3;
/// The source of a compare-and-swap holding the value compared (after the pointer); the value
/// stored follows it, and is the only one of the other atomics.
const ATOMIC_COMPARE: usize = 1;
/// A 32-bit scalar's bytes, the only width the route takes (64-bit operations are lowered).
const WIDTH: u8 = 4;
/// NIR's booleans are one bit wide.
const BOOL_BITS: u32 = 1;
/// What a 16-bit shift takes of its count, as NIR's 16-bit shifts do.
const NARROW_SHIFT_MASK: u32 = u16::BITS - 1;

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
    Buffer,
    Subpass,
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
    R8Unorm,
    R8Snorm,
    R8Uint,
    R8Sint,
    R16Uint,
    R16Sint,
    R16Float,
    R16Unorm,
    R16Snorm,
    Rg8Unorm,
    Rg8Snorm,
    Rg8Uint,
    Rg8Sint,
    R32Uint,
    R32Sint,
    R32Float,
    Rg16Uint,
    Rg16Sint,
    Rg16Float,
    Rg16Unorm,
    Rg16Snorm,
    Rgba8Unorm,
    Rgba8Snorm,
    Rgba8Uint,
    Rgba8Sint,
    Rgb10a2Uint,
    Rgb10a2Unorm,
    Rg11b10Ufloat,
    Rg32Uint,
    Rg32Sint,
    Rg32Float,
    Rgba16Uint,
    Rgba16Sint,
    Rgba16Float,
    Rgba16Unorm,
    Rgba16Snorm,
    Rgba32Uint,
    Rgba32Sint,
    Rgba32Float,
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
    Imod,
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
    Fmod,
    Frem,
    U2u16,
    U2u32,
    B2b1,
    Isign,
    UbitfieldExtract,
    IbitfieldExtract,
    BitfieldInsert,
    FrexpExp,
    FrexpSig,
    B2b32,
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
    LoadPointCoord,
    ImageDerefLoad,
    ImageDerefStore,
    ImageDerefSize,
    DerefAtomic,
    DerefAtomicSwap,
    LoadStage,
    StoreStage,
    DemoteSamples,
    LoadSampleMaskIn,
    Unsupported,
}

/// An atomic operation, by `enum wc_atomic_op`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code, reason = "the bridge makes every value")]
enum Atomic {
    Add,
    Imin,
    Umin,
    Imax,
    Umax,
    And,
    Or,
    Xor,
    Exchange,
    CompareExchange,
    Other,
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
    /// A column of a row-major matrix: the pointer to its rows, the column's index, and the flat
    /// type of the matrix.
    RowMajorColumn(Handle<Expression>, Handle<Expression>, u32),
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
    /// Struct members (flat struct type, field) and variables atomics act on: naga's atomics act
    /// only on atomic types.
    atomic_fields: Vec<(u32, u32)>,
    atomic_vars: Vec<u32>,
    /// Of those, the ones a signed minimum or maximum reaches, whose words are signed: naga's
    /// atomics take the operation's signedness from the word's type.
    signed_atomic_fields: Vec<(u32, u32)>,
    signed_atomic_vars: Vec<u32>,
    /// The derefs into those signed words, whose loads, stores and atomics take signed values.
    signed_derefs: std::collections::HashSet<u32>,
    /// The samples `demote_samples` has left, ANDed into the sample mask output on return.
    kept_samples: Option<Handle<LocalVariable>>,
    /// What only the shaders crate's rewrites make fit for the common form.
    rewrites: Rewrites,
}

/// What route B leaves for the shaders crate's rewrites (plan 15.4): each storage slot declared
/// without a format (given a placeholder of its texel's kind until the variant names the bound
/// view's), and whether a gather takes an offset known only at run time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rewrites {
    pub formatless: Vec<u32>,
    pub runtime_gather_offsets: bool,
}

/// Route B's module and what it needs of the rewrites.
pub struct Routed {
    pub module: Module,
    pub rewrites: Rewrites,
}

/// Candidate B: the program of `nir` as a naga module, for a program that needs none of the
/// shaders crate's rewrites (refused by name otherwise). The shader is changed by the preparation.
pub fn module(nir: &mut Nir) -> Result<Module, ConvertError> {
    let Routed { module, rewrites } = route(nir)?;
    if !rewrites.formatless.is_empty() {
        return Err(unsupported(format_args!(
            "a storage image without a format"
        )));
    }
    if rewrites.runtime_gather_offsets {
        return Err(unsupported(format_args!(
            "an offset known only at run time"
        )));
    }
    Ok(module)
}

/// Candidate B: the program of `nir` as a naga module, with what it leaves for the shaders crate's
/// rewrites ([`rewrite`]). The shader is changed by the preparation.
pub fn route(nir: &mut Nir) -> Result<Routed, ConvertError> {
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
        atomic_fields: Vec::new(),
        atomic_vars: Vec::new(),
        signed_atomic_fields: Vec::new(),
        signed_atomic_vars: Vec::new(),
        signed_derefs: std::collections::HashSet::new(),
        kept_samples: None,
        rewrites: Rewrites::default(),
    };
    let instrs = slice(f.instrs, f.instr_count);
    b.mark_atomics(instrs);
    b.emitter.start(&b.function.expressions);
    b.variables()?;
    if instrs
        .iter()
        .any(|i| i.kind == Kind::Intrinsic && intrinsic_op(i.op) == Intrinsic::DemoteSamples)
    {
        b.keep_samples();
    }
    for instr in instrs {
        b.instr(instr)?;
    }
    let rewrites = std::mem::take(&mut b.rewrites);
    Ok(Routed {
        module: b.finish(f)?,
        rewrites,
    })
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
        Dim::Buffer => return Err(unsupported(format_args!("a texel buffer"))),
        Dim::Subpass => return Err(unsupported(format_args!("a subpass input"))),
        Dim::Other => return Err(unsupported(format_args!("a texture of another dimension"))),
    })
}

/// The components of a position in an image of `dim`, its layer apart.
fn coordinate_size(dim: ImageDimension) -> u32 {
    match dim {
        ImageDimension::D1 => 1,
        ImageDimension::D2 => VectorSize::Bi as u32,
        ImageDimension::D3 | ImageDimension::Cube => VectorSize::Tri as u32,
    }
}

/// The components of an image's size, its layers apart (a cube's is its faces' width and height).
fn size_components(dim: ImageDimension) -> u32 {
    match dim {
        ImageDimension::Cube => VectorSize::Bi as u32,
        other => coordinate_size(other),
    }
}

/// The dimension and arrayedness a storage image takes in the common form: a cube or cube array
/// is a 2D array of its faces (the common form has no storage cubes; Vulkan and OpenGL address a
/// storage cube's faces as layers).
fn storage_dimension(t: FlatType) -> Result<(ImageDimension, bool), ConvertError> {
    Ok(match dimension(t.dim)? {
        ImageDimension::Cube => (ImageDimension::D2, true),
        other => (other, t.arrayed != 0),
    })
}

fn storage_format(f: Format) -> Result<StorageFormat, ConvertError> {
    Ok(match f {
        Format::R8Unorm => StorageFormat::R8Unorm,
        Format::R8Snorm => StorageFormat::R8Snorm,
        Format::R8Uint => StorageFormat::R8Uint,
        Format::R8Sint => StorageFormat::R8Sint,
        Format::R16Uint => StorageFormat::R16Uint,
        Format::R16Sint => StorageFormat::R16Sint,
        Format::R16Float => StorageFormat::R16Float,
        Format::R16Unorm => StorageFormat::R16Unorm,
        Format::R16Snorm => StorageFormat::R16Snorm,
        Format::Rg8Unorm => StorageFormat::Rg8Unorm,
        Format::Rg8Snorm => StorageFormat::Rg8Snorm,
        Format::Rg8Uint => StorageFormat::Rg8Uint,
        Format::Rg8Sint => StorageFormat::Rg8Sint,
        Format::R32Uint => StorageFormat::R32Uint,
        Format::R32Sint => StorageFormat::R32Sint,
        Format::R32Float => StorageFormat::R32Float,
        Format::Rg16Uint => StorageFormat::Rg16Uint,
        Format::Rg16Sint => StorageFormat::Rg16Sint,
        Format::Rg16Float => StorageFormat::Rg16Float,
        Format::Rg16Unorm => StorageFormat::Rg16Unorm,
        Format::Rg16Snorm => StorageFormat::Rg16Snorm,
        Format::Rgba8Unorm => StorageFormat::Rgba8Unorm,
        Format::Rgba8Snorm => StorageFormat::Rgba8Snorm,
        Format::Rgba8Uint => StorageFormat::Rgba8Uint,
        Format::Rgba8Sint => StorageFormat::Rgba8Sint,
        Format::Rgb10a2Uint => StorageFormat::Rgb10a2Uint,
        Format::Rgb10a2Unorm => StorageFormat::Rgb10a2Unorm,
        Format::Rg11b10Ufloat => StorageFormat::Rg11b10Ufloat,
        Format::Rg32Uint => StorageFormat::Rg32Uint,
        Format::Rg32Sint => StorageFormat::Rg32Sint,
        Format::Rg32Float => StorageFormat::Rg32Float,
        Format::Rgba16Uint => StorageFormat::Rgba16Uint,
        Format::Rgba16Sint => StorageFormat::Rgba16Sint,
        Format::Rgba16Float => StorageFormat::Rgba16Float,
        Format::Rgba16Unorm => StorageFormat::Rgba16Unorm,
        Format::Rgba16Snorm => StorageFormat::Rgba16Snorm,
        Format::Rgba32Uint => StorageFormat::Rgba32Uint,
        Format::Rgba32Sint => StorageFormat::Rgba32Sint,
        Format::Rgba32Float => StorageFormat::Rgba32Float,
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
                // naga has no row-major matrices: the rows, as an array of vectors in memory;
                // loads gather the columns.
                let row = self.add_type(TypeInner::Vector {
                    size: vector_size(t.columns)?,
                    scalar: scalar_of(t.scalar)?,
                });
                let stride = self.element_stride(row, t.stride)?;
                TypeInner::Array {
                    base: row,
                    size: naga::ArraySize::Constant(
                        std::num::NonZeroU32::new(t.components)
                            .ok_or_else(|| unsupported(format_args!("a matrix without rows")))?,
                    ),
                    stride,
                }
            }
            TypeKind::Matrix => TypeInner::Matrix {
                columns: vector_size(t.columns)?,
                rows: vector_size(t.components)?,
                scalar: scalar_of(t.scalar)?,
            },
            TypeKind::Array => {
                let element = self.types[t.element as usize];
                if element.kind == TypeKind::Array && self.is_resource(t.element) {
                    return Err(unsupported(format_args!(
                        "an array of arrays of textures, samplers or images"
                    )));
                }
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
                    let stride = self.element_stride(base, t.stride)?;
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
                    if self.is_resource(field.ty) {
                        return Err(unsupported(format_args!(
                            "a structure holding textures, samplers or images"
                        )));
                    }
                    let ty = if self.atomic_fields.contains(&(index, i)) {
                        let signed = self.signed_atomic_fields.contains(&(index, i));
                        self.atomic_ty(field.ty, signed)?
                    } else {
                        self.ty(field.ty)?
                    };
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

    /// An array's stride: the type's explicit one, or its element's size rounded to its alignment.
    fn element_stride(
        &mut self,
        element: Handle<Type>,
        explicit: u32,
    ) -> Result<u32, ConvertError> {
        Ok(if explicit == 0 {
            let l = self.layout(element)?;
            l.alignment.round_up(l.size)
        } else {
            explicit
        })
    }

    /// The atomic form of a flat scalar type, or of an array of them; of a word, `signed` makes it
    /// a signed one.
    fn atomic_ty(&mut self, index: u32, signed: bool) -> Result<Handle<Type>, ConvertError> {
        let t = self.types[index as usize];
        match t.kind {
            TypeKind::Scalar => {
                let scalar = match scalar_of(t.scalar)? {
                    Scalar::U32 if signed => Scalar::I32,
                    scalar => scalar,
                };
                Ok(self.add_type(TypeInner::Atomic(scalar)))
            }
            TypeKind::Array => {
                let base = self.atomic_ty(t.element, signed)?;
                let stride = self.element_stride(base, t.stride)?;
                Ok(self.add_type(TypeInner::Array {
                    base,
                    size: std::num::NonZeroU32::new(t.length)
                        .map_or(naga::ArraySize::Dynamic, naga::ArraySize::Constant),
                    stride,
                }))
            }
            kind => Err(unsupported(format_args!("an atomic on a {kind:?}"))),
        }
    }

    /// Marks the struct members and variables the program's atomics act on, by walking each
    /// atomic's deref back through its array indices.
    fn mark_atomics(&mut self, instrs: &[FlatInstr]) {
        let derefs: std::collections::HashMap<u32, &FlatInstr> = instrs
            .iter()
            .filter(|i| i.kind == Kind::Deref)
            .map(|i| (i.def, i))
            .collect();
        let atomics = instrs.iter().filter(|i| {
            i.kind == Kind::Intrinsic
                && matches!(
                    intrinsic_op(i.op),
                    Intrinsic::DerefAtomic | Intrinsic::DerefAtomicSwap
                )
        });
        for atomic in atomics {
            let signed = matches!(
                atomic_op(atomic.value[VectorSize::MAX - 1]),
                Atomic::Imin | Atomic::Imax
            );
            let mut at = derefs.get(&atomic.src[0].def).copied();
            while let Some(d) = at {
                match deref_kind(d.op) {
                    Deref::Array => at = derefs.get(&d.src[0].def).copied(),
                    Deref::Struct => {
                        if let Some(parent) = derefs.get(&d.src[0].def) {
                            self.atomic_fields.push((parent.ty, d.value[0]));
                            if signed {
                                self.signed_atomic_fields.push((parent.ty, d.value[0]));
                            }
                        }
                        break;
                    }
                    Deref::Var => {
                        self.atomic_vars.push(d.value[0]);
                        if signed {
                            self.signed_atomic_vars.push(d.value[0]);
                        }
                        break;
                    }
                    Deref::Cast => break,
                }
            }
        }
        if self.signed_atomic_vars.is_empty() && self.signed_atomic_fields.is_empty() {
            return;
        }
        for d in derefs.values() {
            let mut at = Some(*d);
            while let Some(step) = at {
                let signed = match deref_kind(step.op) {
                    Deref::Array => {
                        at = derefs.get(&step.src[0].def).copied();
                        continue;
                    }
                    Deref::Struct => derefs.get(&step.src[0].def).is_some_and(|parent| {
                        self.signed_atomic_fields
                            .contains(&(parent.ty, step.value[0]))
                    }),
                    Deref::Var => self.signed_atomic_vars.contains(&step.value[0]),
                    Deref::Cast => false,
                };
                if signed {
                    self.signed_derefs.insert(d.def);
                }
                break;
            }
        }
    }

    /// The scalar kind of the values a pointer from deref `def` of flat type `ty` holds: a
    /// signed atomic word's are signed.
    fn pointer_kind(&self, def: u32, ty: u32) -> ScalarKind {
        match self.kind_of(ty) {
            ScalarKind::Uint if self.signed_derefs.contains(&def) => ScalarKind::Sint,
            kind => kind,
        }
    }

    /// Whether a flat type is a texture, sampler or image, or arrays of them.
    fn is_resource(&self, index: u32) -> bool {
        let mut t = self.types[index as usize];
        while t.kind == TypeKind::Array {
            t = self.types[t.element as usize];
        }
        matches!(
            t.kind,
            TypeKind::Sampler | TypeKind::Texture | TypeKind::Image
        )
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
        // An image only measured is declared written, which every storage format takes (a
        // storage image is read or written in the common form).
        access.set(StorageAccess::STORE, var.writable != 0 || var.readable == 0);
        let (dim, arrayed) = storage_dimension(t)?;
        let format = if var.format == Format::None {
            let first = var
                .binding
                .checked_sub(SlotKind::Storage.first_binding())
                .ok_or_else(|| unsupported(format_args!("a storage image out of its range")))?;
            self.rewrites
                .formatless
                .extend((0..count.unwrap_or(1)).map(|k| first + k));
            match scalar_of(t.sampled)?.kind {
                ScalarKind::Sint => StorageFormat::R32Sint,
                ScalarKind::Uint => StorageFormat::R32Uint,
                _ => StorageFormat::R32Float,
            }
        } else {
            storage_format(var.format)?
        };
        let image = self.add_type(TypeInner::Image {
            dim,
            arrayed,
            class: ImageClass::Storage { format, access },
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
            self.add_unemitted(e)
        } else {
            self.function.expressions.append(e, Span::UNDEFINED)
        }
    }

    /// An expression outside every emit range: one naga does not emit (a variable, a literal, a
    /// statement's result).
    fn add_unemitted(&mut self, e: Expression) -> Handle<Expression> {
        if let Some((s, span)) = self.emitter.finish(&self.function.expressions) {
            self.blocks.last_mut().expect("a block").push(s, span);
        }
        let h = self.function.expressions.append(e, Span::UNDEFINED);
        self.emitter.start(&self.function.expressions);
        h
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
            FlatBuiltIn::ClipDistance if self.stage == ShaderStage::Fragment => {
                return Err(unsupported(format_args!(
                    "clip distances read by a fragment program"
                )));
            }
            FlatBuiltIn::ClipDistance => Binding::BuiltIn(BuiltIn::ClipDistances),
            FlatBuiltIn::FragDepth => Binding::BuiltIn(BuiltIn::FragDepth),
            FlatBuiltIn::SampleMask => Binding::BuiltIn(BuiltIn::SampleMask),
            FlatBuiltIn::Other => {
                return Err(unsupported(format_args!(
                    "a stage value of another built-in (Mesa's slot {})",
                    var.location
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
                            // The common form has no write-only buffers: one is read-write. So
                            // is every buffer of a compute program, where WebGPU allows it: the
                            // API may bind one buffer at a read-only and a written block of one
                            // dispatch, which WebGPU refuses unless both are read-write.
                            let mut access = StorageAccess::LOAD;
                            access.set(
                                StorageAccess::STORE,
                                var.writable != 0 || self.stage == ShaderStage::Compute,
                            );
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
                    } else if self
                        .atomic_vars
                        .contains(&u32::try_from(index).expect("few"))
                    {
                        let signed = self
                            .signed_atomic_vars
                            .contains(&u32::try_from(index).expect("few"));
                        self.atomic_ty(var.ty, signed)?
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

    /// One constant component; an infinite or NaN float is its bits made a float, as the common
    /// form's literals are finite.
    fn scalar_constant(&mut self, kind: ScalarKind, bits: u32) -> Handle<Expression> {
        if kind == ScalarKind::Float && !f32::from_bits(bits).is_finite() {
            let word = self.add(Expression::Literal(Literal::U32(bits)));
            self.add(Expression::As {
                expr: word,
                kind,
                convert: None,
            })
        } else {
            self.add(Expression::Literal(Self::literal(kind, bits)))
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
                    .map(|&v| self.scalar_constant(kind, v))
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
            Value::Resource(..) | Value::RowMajorColumn(..) => {
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
    /// A boolean as NIR's 32-bit one: true as every bit set, the negated conversion to a signed
    /// integer.
    Bool32,
    /// A comparison of each component, then all or any of them.
    Reduce(BinaryOperator, RelationalFunction),
    /// The logical not of `Binary`: an unordered float comparison, true where either side is NaN,
    /// as the not of its ordered opposite (naga's float comparisons are ordered).
    NotBinary(BinaryOperator),
    /// The logical not of `Reduce`: `bany_fnequal` as not all equal, unordered like `fneu`.
    NotReduce(BinaryOperator, RelationalFunction),
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
        Alu::Fneu => (Float, F::NotBinary(B::Equal)),
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
        Alu::Frem => (Float, F::Binary(B::Modulo)),
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
        // A 32-bit boolean word (OpenGL's default-block booleans) as a boolean: not zero.
        Alu::B2b1 => (Uint, F::Cast(Bool)),
        Alu::B2b32 => (Bool, F::Bool32),
        Alu::BitCount => (Uint, F::Math(M::CountOneBits)),
        Alu::UfindMsb => (Uint, F::Math(M::FirstLeadingBit)),
        Alu::IfindMsb => (Sint, F::Math(M::FirstLeadingBit)),
        Alu::Isign => (Sint, F::Math(M::Sign)),
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
            (Float, F::NotReduce(B::Equal, RelationalFunction::All))
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

fn deref_kind(raw: u32) -> Deref {
    // SAFETY: the bridge writes only Deref's values as a deref's operation.
    unsafe { std::mem::transmute::<u32, Deref>(raw) }
}

fn atomic_op(raw: u32) -> Atomic {
    if raw < Atomic::Other as u32 {
        // SAFETY: as alu_op, for Atomic.
        unsafe { std::mem::transmute::<u32, Atomic>(raw) }
    } else {
        Atomic::Other
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
        if i.bits == u16::BITS || matches!(op, Alu::U2u16 | Alu::U2u32) {
            return self.narrow(i);
        }
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
            Alu::Fmod => {
                // NIR's fmod takes the sign of the divisor: a - b * floor(a / b).
                let a = self.src(i, 0, n, ScalarKind::Float)?;
                let b = self.src(i, 1, n, ScalarKind::Float)?;
                let quotient = self.add(Expression::Binary {
                    op: BinaryOperator::Divide,
                    left: a,
                    right: b,
                });
                let floor = self.add(Expression::Math {
                    fun: MathFunction::Floor,
                    arg: quotient,
                    arg1: None,
                    arg2: None,
                    arg3: None,
                });
                let product = self.add(Expression::Binary {
                    op: BinaryOperator::Multiply,
                    left: b,
                    right: floor,
                });
                let e = self.add(Expression::Binary {
                    op: BinaryOperator::Subtract,
                    left: a,
                    right: product,
                });
                (e, ScalarKind::Float)
            }
            Alu::Imod => {
                // NIR's imod takes the sign of the divisor: the remainder, plus the divisor where
                // the two signs differ and the remainder is not zero.
                let a = self.src(i, 0, n, ScalarKind::Sint)?;
                let b = self.src(i, 1, n, ScalarKind::Sint)?;
                let remainder = self.add(Expression::Binary {
                    op: BinaryOperator::Modulo,
                    left: a,
                    right: b,
                });
                let zero = self.add(Expression::Literal(Literal::I32(0)));
                let zero = if n == 1 {
                    zero
                } else {
                    self.add(Expression::Splat {
                        size: vector_size(n)?,
                        value: zero,
                    })
                };
                let signs = self.add(Expression::Binary {
                    op: BinaryOperator::ExclusiveOr,
                    left: remainder,
                    right: b,
                });
                let signs_differ = self.add(Expression::Binary {
                    op: BinaryOperator::Less,
                    left: signs,
                    right: zero,
                });
                let nonzero = self.add(Expression::Binary {
                    op: BinaryOperator::NotEqual,
                    left: remainder,
                    right: zero,
                });
                let condition = self.add(Expression::Binary {
                    op: BinaryOperator::And,
                    left: signs_differ,
                    right: nonzero,
                });
                let adjusted = self.add(Expression::Binary {
                    op: BinaryOperator::Add,
                    left: remainder,
                    right: b,
                });
                let e = self.add(Expression::Select {
                    condition,
                    accept: adjusted,
                    reject: remainder,
                });
                (e, ScalarKind::Sint)
            }
            Alu::UbitfieldExtract | Alu::IbitfieldExtract | Alu::BitfieldInsert => {
                let kind = if op == Alu::IbitfieldExtract {
                    ScalarKind::Sint
                } else {
                    ScalarKind::Uint
                };
                // The common form's offset and count are scalars: one operation a component.
                let parts = (0..n)
                    .map(|c| self.bitfield(i, op, c, kind))
                    .collect::<Result<Vec<_>, _>>()?;
                (self.composed(kind, parts)?, kind)
            }
            Alu::FrexpExp | Alu::FrexpSig => {
                let x = self.src(i, 0, n, ScalarKind::Float)?;
                let size = if n == 1 { None } else { Some(vector_size(n)?) };
                self.module
                    .generate_predeclared_type(naga::PredeclaredType::FrexpResult {
                        size,
                        scalar: Scalar::F32,
                    });
                let result = self.add(Expression::Math {
                    fun: MathFunction::Frexp,
                    arg: x,
                    arg1: None,
                    arg2: None,
                    arg3: None,
                });
                // The result's members: the significand, then the exponent.
                let (index, kind) = if op == Alu::FrexpSig {
                    (0, ScalarKind::Float)
                } else {
                    (1, ScalarKind::Sint)
                };
                (
                    self.add(Expression::AccessIndex {
                        base: result,
                        index,
                    }),
                    kind,
                )
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
                let negated = matches!(form, AluForm::NotBinary(_) | AluForm::NotReduce(..));
                let (e, kind) = match form {
                    AluForm::Binary(o) | AluForm::NotBinary(o) => {
                        let right_kind = match o {
                            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
                                ScalarKind::Uint
                            }
                            _ => source_kind,
                        };
                        let b = self.src(i, 1, count(1), right_kind)?;
                        // The common form has no exclusive or of booleans: they differ.
                        let o = if boolean && o == BinaryOperator::ExclusiveOr {
                            BinaryOperator::NotEqual
                        } else {
                            o
                        };
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
                            convert: Some(if to == ScalarKind::Bool {
                                naga::BOOL_WIDTH
                            } else {
                                WIDTH
                            }),
                        }),
                        to,
                    ),
                    AluForm::Bool32 => {
                        let one = self.add(Expression::As {
                            expr: a,
                            kind: ScalarKind::Sint,
                            convert: Some(WIDTH),
                        });
                        let all = self.add(Expression::Unary {
                            op: UnaryOperator::Negate,
                            expr: one,
                        });
                        (all, ScalarKind::Sint)
                    }
                    AluForm::Reduce(o, fun) | AluForm::NotReduce(o, fun) => {
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
                };
                if negated {
                    let not = self.add(Expression::Unary {
                        op: UnaryOperator::LogicalNot,
                        expr: e,
                    });
                    (not, kind)
                } else {
                    (e, kind)
                }
            }
        };
        self.set(i, Value::Expr(e, kind));
        Ok(())
    }

    /// Component `c` of `op`'s result, a bitfield extract or insert of `kind`.
    fn bitfield(
        &mut self,
        i: &FlatInstr,
        op: Alu,
        c: u32,
        kind: ScalarKind,
    ) -> Result<Handle<Expression>, ConvertError> {
        let base = self.src_component(i, 0, c, kind)?;
        let insert = op == Alu::BitfieldInsert;
        let fun = if insert {
            MathFunction::InsertBits
        } else {
            MathFunction::ExtractBits
        };
        // An insert's bits are of the base's kind; every offset and count is a u32.
        let mut args = Vec::new();
        for s in 1..i.sources as usize {
            let k = if insert && s == 1 {
                kind
            } else {
                ScalarKind::Uint
            };
            args.push(self.src_component(i, s, c, k)?);
        }
        let mut args = args.into_iter();
        Ok(self.add(Expression::Math {
            fun,
            arg: base,
            arg1: args.next(),
            arg2: args.next(),
            arg3: args.next(),
        }))
    }

    /// Component `c` of source `s` of `instr` by its swizzle, as `kind`.
    fn src_component(
        &mut self,
        instr: &FlatInstr,
        s: usize,
        c: u32,
        kind: ScalarKind,
    ) -> Result<Handle<Expression>, ConvertError> {
        let src = instr.src[s];
        let (e, have) = self.whole(src.def, kind)?;
        Ok(if have == 1 {
            e
        } else {
            self.add(Expression::AccessIndex {
                base: e,
                index: u32::from(src.swizzle[c as usize]),
            })
        })
    }

    /// One value of `kind` from its scalar components.
    fn composed(
        &mut self,
        kind: ScalarKind,
        parts: Vec<Handle<Expression>>,
    ) -> Result<Handle<Expression>, ConvertError> {
        if let [one] = parts[..] {
            return Ok(one);
        }
        let ty = self.scalar_type(kind, parts.len() as u32)?;
        Ok(self.add(Expression::Compose {
            ty,
            components: parts,
        }))
    }

    /// The kind a value already has (constants and registers take the use's; u32 here).
    fn natural(&self, def: u32) -> ScalarKind {
        match self.value(def) {
            Ok(Value::Expr(_, k)) => k,
            Ok(Value::Const(_, _, true) | Value::Register(_, true)) => ScalarKind::Bool,
            _ => ScalarKind::Uint,
        }
    }

    fn is_row_major(&self, ty: u32) -> bool {
        let t = self.types[ty as usize];
        t.kind == TypeKind::Matrix && t.row_major != 0
    }

    /// Column `column` of the row-major matrix of flat type `ty` whose rows `rows` points at.
    fn row_major_column(
        &mut self,
        rows: Handle<Expression>,
        column: Handle<Expression>,
        ty: u32,
    ) -> Result<Handle<Expression>, ConvertError> {
        let t = self.types[ty as usize];
        let parts = (0..t.components)
            .map(|r| {
                let row = self.add(Expression::AccessIndex {
                    base: rows,
                    index: r,
                });
                let pointer = self.add(Expression::Access {
                    base: row,
                    index: column,
                });
                self.add(Expression::Load { pointer })
            })
            .collect();
        let ty = self.scalar_type(ScalarKind::Float, t.components)?;
        Ok(self.add(Expression::Compose {
            ty,
            components: parts,
        }))
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
        let kind = deref_kind(i.op);
        let e = match kind {
            Deref::Var => self.places[i.value[0] as usize]
                .ok_or_else(|| unsupported(format_args!("a variable the module does not have")))?,
            Deref::Array => {
                if let Value::RowMajorColumn(rows, column, _) = self.value(i.src[0].def)? {
                    let row = self.src(i, 1, 1, ScalarKind::Uint)?;
                    let row = self.add(Expression::Access {
                        base: rows,
                        index: row,
                    });
                    let e = self.add(Expression::Access {
                        base: row,
                        index: column,
                    });
                    self.set(i, Value::Pointer(e, i.ty));
                    return Ok(());
                }
                let (base, ty) = self.pointer(i.src[0].def)?;
                if self.is_row_major(ty) {
                    let column = self.src(i, 1, 1, ScalarKind::Uint)?;
                    self.set(i, Value::RowMajorColumn(base, column, ty));
                    return Ok(());
                }
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
        let value = if self.outputs.is_empty() && self.kept_samples.is_none()
            || self.stage == ShaderStage::Compute
        {
            None
        } else {
            let mut outputs = self.outputs.clone();
            let sample_mask = Binding::BuiltIn(BuiltIn::SampleMask);
            let kept = self.kept_samples.map(|kept| {
                let pointer = self.add(Expression::LocalVariable(kept));
                (kept, self.add(Expression::Load { pointer }))
            });
            if let Some((kept, _)) = kept
                && !outputs.iter().any(|o| o.2 == sample_mask)
            {
                let ty = self.function.local_variables[kept].ty;
                outputs.push((kept, ty, sample_mask.clone()));
            }
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
                let mut part = self.add(Expression::Load { pointer });
                // The program's own mask, where it writes one, keeps only what is not demoted.
                if let Some((kept, kept_value)) = kept
                    && binding == sample_mask
                    && local != kept
                {
                    part = self.add(Expression::Binary {
                        op: BinaryOperator::And,
                        left: part,
                        right: kept_value,
                    });
                }
                parts.push(part);
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

    /// The samples `demote_samples` leaves, all of them at the start.
    fn keep_samples(&mut self) {
        let ty = self.add_type(TypeInner::Scalar(Scalar::U32));
        let local = self.function.local_variables.append(
            LocalVariable {
                name: None,
                ty,
                init: None,
            },
            Span::UNDEFINED,
        );
        let pointer = self.add(Expression::LocalVariable(local));
        let value = self.add(Expression::Literal(Literal::U32(u32::MAX)));
        self.statement(Statement::Store { pointer, value });
        self.kept_samples = Some(local);
    }

    /// The 16-bit integer operations of Mesa's alpha-to-coverage pass, carried in 32-bit words
    /// holding the zero-extended value (plan 15.4): the same bits as 16-bit operations.
    fn narrow(&mut self, i: &FlatInstr) -> Result<(), ConvertError> {
        let n = i.components;
        let op = alu_op(i.op);
        let a = self.src(i, 0, n, ScalarKind::Uint)?;
        let binary = |b: &mut Self, op, left, right| b.add(Expression::Binary { op, left, right });
        let e = match op {
            Alu::Mov | Alu::U2u32 => a,
            Alu::U2u16 => self.low_bits(a, n)?,
            Alu::Iand | Alu::Ior | Alu::Imul => {
                let b = self.src(i, 1, n, ScalarKind::Uint)?;
                let o = match op {
                    Alu::Iand => BinaryOperator::And,
                    Alu::Ior => BinaryOperator::InclusiveOr,
                    _ => BinaryOperator::Multiply,
                };
                let e = binary(self, o, a, b);
                if op == Alu::Imul {
                    self.low_bits(e, n)?
                } else {
                    e
                }
            }
            Alu::Ishl | Alu::Ushr => {
                let count = self.src(i, 1, n, ScalarKind::Uint)?;
                let mask = self.splat_u32(NARROW_SHIFT_MASK, n)?;
                let count = binary(self, BinaryOperator::And, count, mask);
                if op == Alu::Ishl {
                    let e = binary(self, BinaryOperator::ShiftLeft, a, count);
                    self.low_bits(e, n)?
                } else {
                    binary(self, BinaryOperator::ShiftRight, a, count)
                }
            }
            _ => {
                return Err(unsupported(format_args!("a {}-bit {}", u16::BITS, name(i))));
            }
        };
        self.set(i, Value::Expr(e, ScalarKind::Uint));
        Ok(())
    }

    /// `value`'s low 16 bits, each of its `n` components.
    fn low_bits(
        &mut self,
        value: Handle<Expression>,
        n: u32,
    ) -> Result<Handle<Expression>, ConvertError> {
        let mask = self.splat_u32(u32::from(u16::MAX), n)?;
        Ok(self.add(Expression::Binary {
            op: BinaryOperator::And,
            left: value,
            right: mask,
        }))
    }

    fn splat_u32(&mut self, value: u32, n: u32) -> Result<Handle<Expression>, ConvertError> {
        let value = self.add(Expression::Literal(Literal::U32(value)));
        Ok(if n == 1 {
            value
        } else {
            self.add(Expression::Splat {
                size: vector_size(n)?,
                value,
            })
        })
    }

    /// The local a stage value's slot is kept in, and its flat type.
    fn stage_value(&self, i: &FlatInstr) -> Result<(Handle<Expression>, u32), ConvertError> {
        let var = i.value[0] as usize;
        let place = self.places.get(var).copied().flatten();
        match (place, self.vars.get(var)) {
            (Some(pointer), Some(v)) => Ok((pointer, v.ty)),
            _ => Err(unsupported(format_args!(
                "a stage value the module does not have"
            ))),
        }
    }

    /// `count` components of `whole` (a value of the flat type `ty`) from `first` on: a vector's
    /// swizzle, or an array's elements gathered into a vector.
    fn part(
        &mut self,
        whole: Handle<Expression>,
        ty: u32,
        first: u32,
        count: u32,
    ) -> Result<Handle<Expression>, ConvertError> {
        let t = self.types[ty as usize];
        if t.kind == TypeKind::Scalar || (first == 0 && count == t.components) {
            return Ok(whole);
        }
        if count == 1 {
            return Ok(self.add(Expression::AccessIndex {
                base: whole,
                index: first,
            }));
        }
        if t.kind == TypeKind::Array {
            let components = (first..first + count)
                .map(|index| self.add(Expression::AccessIndex { base: whole, index }))
                .collect();
            let ty = self.scalar_type(self.kind_of(ty), count)?;
            return Ok(self.add(Expression::Compose { ty, components }));
        }
        let mut pattern = [naga::SwizzleComponent::X; VectorSize::MAX];
        for (p, c) in pattern.iter_mut().zip(first..first + count) {
            *p = naga::SwizzleComponent::XYZW[c as usize];
        }
        Ok(self.add(Expression::Swizzle {
            size: vector_size(count)?,
            vector: whole,
            pattern,
        }))
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
                if let Value::RowMajorColumn(rows, column, ty) = self.value(i.src[0].def)? {
                    let e = self.row_major_column(rows, column, ty)?;
                    self.set(i, Value::Expr(e, Float));
                    return Ok(());
                }
                let (pointer, ty) = self.pointer(i.src[0].def)?;
                if self.is_row_major(ty) {
                    let t = self.types[ty as usize];
                    let columns = (0..t.columns)
                        .map(|c| {
                            let c = self.add(Expression::Literal(Literal::U32(c)));
                            self.row_major_column(pointer, c, ty)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let matrix = self.add_type(TypeInner::Matrix {
                        columns: vector_size(t.columns)?,
                        rows: vector_size(t.components)?,
                        scalar: scalar_of(t.scalar)?,
                    });
                    let e = self.add(Expression::Compose {
                        ty: matrix,
                        components: columns,
                    });
                    self.set(i, Value::Expr(e, Float));
                    return Ok(());
                }
                let kind = self.pointer_kind(i.src[0].def, ty);
                let e = self.add(Expression::Load { pointer });
                self.set(i, Value::Expr(e, kind));
            }
            Intrinsic::StoreDeref => {
                let (pointer, ty) = self.pointer(i.src[0].def)?;
                if self.is_row_major(ty) {
                    return Err(unsupported(format_args!("a store into a row-major matrix")));
                }
                let kind = self.pointer_kind(i.src[0].def, ty);
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
            Intrinsic::LoadSampleMaskIn => self.system_value(i, BuiltIn::SampleMask, Uint)?,
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
            Intrinsic::LoadPointCoord => self.system_value(i, BuiltIn::PointCoord, Float)?,
            Intrinsic::ImageDerefLoad => {
                let (image, t) = self.image(i)?;
                let (coordinate, array_index) = self.image_coordinate(i, t)?;
                let sample = if t.multisampled != 0 {
                    Some(self.src(i, 2, 1, ScalarKind::Sint)?)
                } else {
                    None
                };
                let e = self.add(Expression::ImageLoad {
                    image,
                    coordinate,
                    array_index,
                    sample,
                    level: None,
                });
                let kind = scalar_of(t.sampled)?.kind;
                let e = self.first(e, VectorSize::Quad as u32, i.components)?;
                self.set(i, Value::Expr(e, kind));
            }
            Intrinsic::ImageDerefStore => {
                let (image, t) = self.image(i)?;
                if t.multisampled != 0 {
                    return Err(unsupported(format_args!(
                        "a store into a multisampled storage image"
                    )));
                }
                let (coordinate, array_index) = self.image_coordinate(i, t)?;
                let kind = scalar_of(t.sampled)?.kind;
                let (value, components) = self.whole(i.src[IMAGE_STORE_VALUE].def, kind)?;
                if components != VectorSize::Quad as u32 {
                    return Err(unsupported(format_args!(
                        "an image store of {components} components"
                    )));
                }
                self.statement(Statement::ImageStore {
                    image,
                    coordinate,
                    array_index,
                    value,
                });
            }
            Intrinsic::ImageDerefSize => {
                let (image, t) = self.image(i)?;
                let e = if dimension(t.dim)? == ImageDimension::Cube {
                    self.storage_cube_size(image, t.arrayed != 0)?
                } else {
                    self.image_size(image, dimension(t.dim)?, t.arrayed != 0, None)?
                };
                self.set(i, Value::Expr(e, Uint));
            }
            Intrinsic::DerefAtomic | Intrinsic::DerefAtomicSwap => self.atomic(i)?,
            Intrinsic::LoadStage => {
                let (pointer, ty) = self.stage_value(i)?;
                let whole = self.add(Expression::Load { pointer });
                let e = self.part(whole, ty, i.component, i.components)?;
                self.set(i, Value::Expr(e, self.kind_of(ty)));
            }
            Intrinsic::StoreStage => {
                let (pointer, ty) = self.stage_value(i)?;
                let (value, components) = self.whole(i.src[0].def, self.kind_of(ty))?;
                if self.types[ty as usize].kind == TypeKind::Scalar {
                    self.statement(Statement::Store { pointer, value });
                    return Ok(());
                }
                for c in (0..components).filter(|c| i.value[2] & (1 << c) != 0) {
                    let pointer = self.add(Expression::AccessIndex {
                        base: pointer,
                        index: i.component + c,
                    });
                    let value = if components == 1 {
                        value
                    } else {
                        self.add(Expression::AccessIndex {
                            base: value,
                            index: c,
                        })
                    };
                    self.statement(Statement::Store { pointer, value });
                }
            }
            Intrinsic::DemoteSamples => {
                let kept = self
                    .kept_samples
                    .expect("made where the program demotes samples");
                let pointer = self.add(Expression::LocalVariable(kept));
                let left = self.add(Expression::Load { pointer });
                let demoted = self.src(i, 0, 1, Uint)?;
                let right = self.add(Expression::Unary {
                    op: UnaryOperator::BitwiseNot,
                    expr: demoted,
                });
                let value = self.add(Expression::Binary {
                    op: BinaryOperator::And,
                    left,
                    right,
                });
                self.statement(Statement::Store { pointer, value });
            }
            Intrinsic::Unsupported => return Err(unsupported(name(i))),
        }
        Ok(())
    }

    /// The storage image an image intrinsic acts on, with its flat type.
    fn image(&self, i: &FlatInstr) -> Result<(Handle<Expression>, FlatType), ConvertError> {
        let (image, ty) = self.pointer(i.src[0].def)?;
        Ok((image, self.types[ty as usize]))
    }

    /// An image intrinsic's coordinate (its second source, four components in NIR) as the
    /// position and the layer.
    fn image_coordinate(
        &mut self,
        i: &FlatInstr,
        t: FlatType,
    ) -> Result<(Handle<Expression>, Option<Handle<Expression>>), ConvertError> {
        let (dim, arrayed) = storage_dimension(t)?;
        let space = coordinate_size(dim);
        let position = self.src(i, 1, space, ScalarKind::Sint)?;
        let layer = if arrayed {
            let mut instr = *i;
            instr.src[1].swizzle[0] = u8::try_from(space).expect("a component");
            Some(self.src(&instr, 1, 1, ScalarKind::Sint)?)
        } else {
            None
        };
        Ok((position, layer))
    }

    /// The first `want` components of `e`, a vector of `have`.
    fn first(
        &mut self,
        e: Handle<Expression>,
        have: u32,
        want: u32,
    ) -> Result<Handle<Expression>, ConvertError> {
        Ok(if want == have {
            e
        } else if want == 1 {
            self.add(Expression::AccessIndex { base: e, index: 0 })
        } else {
            self.add(Expression::Swizzle {
                size: vector_size(want)?,
                vector: e,
                pattern: naga::SwizzleComponent::XYZW,
            })
        })
    }

    /// An image's size, with its layer count after it when it is arrayed.
    fn image_size(
        &mut self,
        image: Handle<Expression>,
        dim: ImageDimension,
        arrayed: bool,
        level: Option<Handle<Expression>>,
    ) -> Result<Handle<Expression>, ConvertError> {
        let space = size_components(dim);
        let size = self.add(Expression::ImageQuery {
            image,
            query: naga::ImageQuery::Size { level },
        });
        if !arrayed {
            return Ok(size);
        }
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
        let ty = self.scalar_type(ScalarKind::Uint, space + 1)?;
        Ok(self.add(Expression::Compose {
            ty,
            components: parts,
        }))
    }

    /// A storage cube's size, held as a 2D array of its faces (`storage_dimension`): its faces'
    /// size, and for a cube array the cubes after it.
    fn storage_cube_size(
        &mut self,
        image: Handle<Expression>,
        arrayed: bool,
    ) -> Result<Handle<Expression>, ConvertError> {
        let size = self.add(Expression::ImageQuery {
            image,
            query: naga::ImageQuery::Size { level: None },
        });
        if !arrayed {
            return Ok(size);
        }
        let layers = self.add(Expression::ImageQuery {
            image,
            query: naga::ImageQuery::NumLayers,
        });
        let faces = self.add(Expression::Literal(Literal::U32(crate::CUBE_FACES)));
        let cubes = self.add(Expression::Binary {
            op: BinaryOperator::Divide,
            left: layers,
            right: faces,
        });
        let space = size_components(ImageDimension::Cube);
        let mut parts: Vec<_> = (0..space)
            .map(|c| {
                self.add(Expression::AccessIndex {
                    base: size,
                    index: c,
                })
            })
            .collect();
        parts.push(cubes);
        let ty = self.scalar_type(ScalarKind::Uint, space + 1)?;
        Ok(self.add(Expression::Compose {
            ty,
            components: parts,
        }))
    }

    /// An atomic operation on a buffer's or workgroup variable's scalar (`mark_atomics` made its
    /// type atomic).
    fn atomic(&mut self, i: &FlatInstr) -> Result<(), ConvertError> {
        use naga::AtomicFunction as F;
        let op = atomic_op(i.value[VectorSize::MAX - 1]);
        let (pointer, ty) = self.pointer(i.src[0].def)?;
        let kind = self.pointer_kind(i.src[0].def, ty);
        let needs = |want: ScalarKind| {
            if kind == want {
                Ok(())
            } else {
                Err(unsupported(format_args!("an atomic {op:?} on a {kind:?}")))
            }
        };
        let fun = match op {
            Atomic::Add => F::Add,
            Atomic::And => F::And,
            Atomic::Or => F::InclusiveOr,
            Atomic::Xor => F::ExclusiveOr,
            Atomic::Imin | Atomic::Umin | Atomic::Imax | Atomic::Umax => {
                needs(if matches!(op, Atomic::Imin | Atomic::Imax) {
                    ScalarKind::Sint
                } else {
                    ScalarKind::Uint
                })?;
                if matches!(op, Atomic::Imin | Atomic::Umin) {
                    F::Min
                } else {
                    F::Max
                }
            }
            Atomic::Exchange => F::Exchange { compare: None },
            Atomic::CompareExchange => F::Exchange {
                compare: Some(self.whole(i.src[ATOMIC_COMPARE].def, kind)?.0),
            },
            Atomic::Other => {
                return Err(unsupported(format_args!("an atomic of another operation")));
            }
        };
        let compare = op == Atomic::CompareExchange;
        let value_at = if compare {
            ATOMIC_COMPARE + 1
        } else {
            ATOMIC_COMPARE
        };
        let (value, _) = self.whole(i.src[value_at].def, kind)?;
        let scalar = Scalar { kind, width: WIDTH };
        let ty = if compare {
            self.module.generate_predeclared_type(
                naga::PredeclaredType::AtomicCompareExchangeWeakResult(scalar),
            )
        } else {
            self.add_type(TypeInner::Scalar(scalar))
        };
        let result = self.add_unemitted(Expression::AtomicResult {
            ty,
            comparison: compare,
        });
        self.statement(Statement::Atomic {
            pointer,
            fun,
            value,
            result: Some(result),
        });
        let e = if compare {
            self.add(Expression::AccessIndex {
                base: result,
                index: 0,
            })
        } else {
            result
        };
        self.set(i, Value::Expr(e, kind));
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
        let space = coordinate_size(dim);
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
                if result_kind != Float && op != TexOp::Gather {
                    return Err(unsupported(format_args!(
                        "sampling an integer texture (the common form only loads and gathers them)"
                    )));
                }
                let depth_ref = scalar_src(self, TexSrc::Comparator, Float)?;
                if depth_ref.is_some() && matches!(op, TexOp::Bias | TexOp::Gradient) {
                    return Err(unsupported(format_args!(
                        "a comparison with a bias or gradients"
                    )));
                }
                // The base level: `Zero` is the common form's level of comparisons and gathers
                // only (HLSL has no plain sample at level zero), a level of 0.0 otherwise.
                let base_level = |b: &mut Self| match depth_ref {
                    Some(_) => SampleLevel::Zero,
                    None => SampleLevel::Exact(b.scalar_constant(Float, 0.0f32.to_bits())),
                };
                let level = match op {
                    TexOp::Gather => SampleLevel::Zero,
                    // Outside the fragment stage an implicit level is the base level.
                    TexOp::Sample if self.stage != ShaderStage::Fragment => base_level(self),
                    TexOp::Sample => SampleLevel::Auto,
                    TexOp::Bias => SampleLevel::Bias(
                        scalar_src(self, TexSrc::Bias, Float)?
                            .ok_or_else(|| unsupported(format_args!("a bias")))?,
                    ),
                    TexOp::Level => match self.tex_src(i, TexSrc::Lod).map(|d| self.value(d)) {
                        Some(Ok(Value::Const(v, _, _))) if v[0] == 0 => base_level(self),
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
                        // The shaders crate's rewrite moves a gather's offset into its
                        // coordinate.
                        _ if op == TexOp::Gather => {
                            self.rewrites.runtime_gather_offsets = true;
                            Some(self.whole(def, Sint)?.0)
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
                if self.tex_src(i, TexSrc::Offset).is_some() {
                    return Err(unsupported(format_args!("a texel load with an offset")));
                }
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
                self.image_size(image, dim, arrayed, level)?
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
        let shadow = i.shadow != 0
            && matches!(
                op,
                TexOp::Sample | TexOp::Bias | TexOp::Level | TexOp::Gradient
            );
        let e = if shadow && i.components > 1 {
            // A comparison gives one value; NIR's legacy shadow lookups have it in every component.
            self.add(Expression::Splat {
                size: vector_size(i.components)?,
                value: e,
            })
        } else if !shadow && i.components == 1 && !matches!(op, TexOp::Size | TexOp::Levels) {
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

/// Why [`rewrite`] failed where the shaders crate gave no module.
const NO_MODULE: &str = "the shaders crate's rewrite gave no module";

/// The shaders crate's rewrites of plan 15.4 on route B's module: arrays of textures, storage
/// textures and samplers split into a binding an element, gathers at offsets known only at run
/// time moved into their coordinates, and each storage texture whose slot `variant` names given
/// the bound view's format; vertex inputs fetched in the program (`VariantKey::pulled`), which
/// find their inputs by location; and border colours applied in the program
/// (`VariantKey::border_in_shader`), which read the sampler's state from helper constants. A slot declared without a format that `variant` does not name is
/// refused by name, as is a variant asking for more than these (its rewrites read the program's
/// stage values, which the driver's reflection holds). `bindings` is what `Nir::to_common_form`
/// gave, `route_key` the program's `Nir::key`. The module carries `cache::rewritten_key` of it;
/// the reflection holds the bindings and helper constants the rewrites added.
pub fn rewrite(
    routed: Routed,
    bindings: Vec<ReflectedBinding>,
    variant: &VariantKey,
    route_key: &[u8; cache::KEY_BYTES],
) -> Result<(ShaderModule, Reflection), ConvertError> {
    let named = |slot: &u32| variant.storage_formats.iter().any(|(s, _)| s == slot);
    if !routed.rewrites.formatless.iter().all(named) {
        return Err(unsupported(format_args!(
            "a storage image without a format the variant does not name"
        )));
    }
    let routed_rewrites = VariantKey {
        storage_formats: variant.storage_formats.clone(),
        texels_through_buffer: variant.texels_through_buffer,
        pulled: variant.pulled.clone(),
        border_in_shader: variant.border_in_shader,
        ..VariantKey::default()
    };
    if *variant != routed_rewrites {
        return Err(unsupported(format_args!(
            "a variant rewrite reading the program's stage values"
        )));
    }
    let stage = match routed.module.entry_points.first().map(|ep| ep.stage) {
        Some(ShaderStage::Vertex) => SourceStage::Vertex,
        Some(ShaderStage::Fragment) => SourceStage::Pixel,
        _ => SourceStage::Compute,
    };
    let reflection = Reflection {
        stage,
        inputs: Vec::new(),
        outputs: Vec::new(),
        bindings,
        helpers: Vec::new(),
        helper_size: 0,
        sampled_pairs: Vec::new(),
        extras: StageExtras::None,
        slow_paths: Vec::new(),
    };
    let mut converted = wcontainers_shaders::rewrite(routed.module, reflection, variant, route_key)
        .map_err(|e| ConvertError::Rewrite(e.to_string()))?;
    let module = converted
        .modules
        .pop()
        .ok_or_else(|| ConvertError::Rewrite(NO_MODULE.into()))?;
    Ok((module, converted.reflection))
}
