//! Lowers dxbc-spirv's legalised IR (SSA, structured control flow) straight into a naga module.
//!
//! Inputs and outputs live in private variables that an entry point wrapper fills from its arguments and
//! reads into its result, so the shader's own functions read and write them anywhere. Every user
//! location is a four-component value, so stages converted apart always match. An SSA value used outside
//! the block that makes it, and every phi, goes through a function variable; within a block, values are
//! plain expressions.

use crate::ConvertError;
use crate::ir::{
    self, Basic, Ir, Op, Ty, atomic_op, builtin, code, construct, derivative, interp, kind, round,
    scalar,
};
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::num::NonZeroU32;
use naga::{
    AddressSpace, ArraySize, BinaryOperator, Block, Expression, Function, FunctionArgument,
    FunctionResult, GlobalVariable, Handle, ImageClass, ImageDimension, ImageQuery, Literal,
    LocalVariable, MathFunction, Module, ResourceBinding, SampleLevel, Scalar, ScalarKind, Span,
    Statement, StructMember, SwitchCase, SwitchValue, SwizzleComponent, Type, TypeInner,
    UnaryOperator, VectorSize,
};
use naga::{FastHashMap as HashMap, FastHashSet as HashSet};
use wcontainers_gpu::{
    DrawBaseSource, HelperSlot, HelperValue, Interpolation, ReflectedBinding, Reflection,
    SLOT_KINDS, ScalarKind as ValueKind, ShaderStage, SlotKind, SlowPath, SourceStage, StageExtras,
    StageValue, SystemValue, TexelBufferRoute, VariantFlags, VariantKey, helper_binding,
};

use crate::variant::texel::{TexelBuffer, TexelPlace};

pub(crate) mod evaluate;
pub(crate) mod geometry;
pub(crate) mod hull;

type Result<T> = core::result::Result<T, ConvertError>;

/// The clip and cull distances a vertex has in Direct3D 11 and 12, together
/// (`D3D11_CLIP_OR_CULL_DISTANCE_COUNT`).
const CLIP_OR_CULL_DISTANCES: u32 = 8;
/// A declaration's atomic minimums or maximums: signed ones, unsigned ones.
const SIGNED_EXTREMES: u8 = 1;
const UNSIGNED_EXTREMES: u8 = SIGNED_EXTREMES << 1;
/// The component counts of three- and four-component vectors.
const VEC3_SIZE: u8 = naga::VectorSize::Tri as u8;
const VEC4_SIZE: u8 = naga::VectorSize::Quad as u8;
/// The refusal of clip distances of a type the common form's array cannot hold.
pub(super) const CLIP_DISTANCE_TYPE: &str = "ClipDistanceType";

/// The names of resource variables before their register, as HLSL writes registers (`cb0`, `t3`,
/// `u1`; counters `c`), and before a register space other than the first.
mod register_prefix {
    pub const UNIFORM: &str = "cb";
    pub const SAMPLER: &str = "s";
    pub const TEXTURE: &str = "t";
    pub const STORAGE: &str = "u";
    pub const COUNTER: &str = "c";
    pub const SPACE: &str = "_space";
}

fn unsupported(what: &'static str) -> ConvertError {
    ConvertError::Unsupported(what)
}

fn malformed(what: String) -> ConvertError {
    ConvertError::Malformed(what)
}

/// The interpolation mode bits the variant asks a vertex output at `location` to be written with.
pub(crate) fn output_modes(asked: &[(u32, Interpolation)], location: u32) -> u64 {
    let Some(&(_, i)) = asked.iter().find(|(l, _)| *l == location) else {
        return 0;
    };
    match i {
        Interpolation::Perspective => 0,
        Interpolation::Linear => interp::NO_PERSPECTIVE,
        Interpolation::Flat => interp::FLAT,
        Interpolation::PerspectiveCentroid => interp::CENTROID,
        Interpolation::LinearCentroid => interp::NO_PERSPECTIVE | interp::CENTROID,
        Interpolation::PerspectiveSample => interp::SAMPLE,
        Interpolation::LinearSample => interp::NO_PERSPECTIVE | interp::SAMPLE,
    }
}

/// Turns off WGSL's derivative uniformity check for the module: Direct3D takes derivatives and
/// implicit-level samples in any control flow, giving helper lanes their values. Likewise WGSL's
/// subgroup uniformity check where the module has subgroup operations: Direct3D's wave operations
/// run over the lanes active where they are reached. The module's own filters stay in force above
/// these.
pub(crate) fn derivatives_anywhere(module: &mut Module) {
    use naga::diagnostic_filter as d;
    let mut filter = module.diagnostic_filters.append(
        d::DiagnosticFilterNode {
            inner: d::DiagnosticFilter {
                new_severity: d::Severity::Off,
                triggering_rule: d::FilterableTriggeringRule::Standard(
                    d::StandardFilterableTriggeringRule::DerivativeUniformity,
                ),
            },
            parent: module.diagnostic_filter_leaf,
        },
        Span::UNDEFINED,
    );
    if module.uses_subgroups() {
        filter = module.diagnostic_filters.append(
            d::DiagnosticFilterNode {
                inner: d::DiagnosticFilter {
                    new_severity: d::Severity::Off,
                    // A rule of WGSL's that naga does not check, so it has no name of naga's own.
                    triggering_rule: d::FilterableTriggeringRule::Unknown(
                        "subgroup_uniformity".into(),
                    ),
                },
                parent: Some(filter),
            },
            Span::UNDEFINED,
        );
    }
    module.diagnostic_filter_leaf = Some(filter);
}

/// naga's interpolation and sampling for interpolation mode bits (`ir::interp`).
pub(crate) fn naga_interpolation(
    modes: u64,
) -> (Option<naga::Interpolation>, Option<naga::Sampling>) {
    if modes & interp::FLAT != 0 {
        return (Some(naga::Interpolation::Flat), None);
    }
    let i = if modes & interp::NO_PERSPECTIVE != 0 {
        naga::Interpolation::Linear
    } else {
        naga::Interpolation::Perspective
    };
    let s = if modes & interp::SAMPLE != 0 {
        naga::Sampling::Sample
    } else if modes & interp::CENTROID != 0 {
        naga::Sampling::Centroid
    } else {
        naga::Sampling::Center
    };
    (Some(i), Some(s))
}

/// Whether `op` asks if level 0 is below a texture's level count (Direct3D's `resinfo` at level 0). A
/// bound view has at least one level, so it is true; GLSL ES 3.00 cannot ask for the level count.
fn level_zero_check(ir: &Ir, op: &Op) -> bool {
    let is_mips = |id: u32| ir.op(id).is_ok_and(|o| o.code == code::IMAGE_QUERY_MIPS);
    let is_zero = |id: u32| {
        ir.op(id)
            .is_ok_and(|o| o.code == code::CONSTANT && o.lits.first() == Some(&0))
    };
    match (op.code, &op.refs[..]) {
        (code::U_LT, &[x, y]) => is_zero(x) && is_mips(y),
        (code::U_GT | code::I_NE, &[x, y]) => is_mips(x) && is_zero(y),
        _ => false,
    }
}

/// What a lowered SSA value is.
#[derive(Clone)]
enum Val {
    Expr(Handle<Expression>),
    /// The members of a struct value (image size queries, carries).
    Tuple(Vec<Handle<Expression>>),
}

/// A resource's global and what the shader does with it.
#[derive(Clone, Copy)]
struct Resource {
    var: Handle<GlobalVariable>,
    /// The texture's dimension and arrayness, for images.
    image: Option<(u64, bool)>,
    /// The texture holds depth (it is compared against).
    depth: bool,
    /// A storage view: loads take no level.
    storage: bool,
}

/// One element of a resource declaration: the declaration's id and the element's place in its array.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ResourceElement {
    dcl: u32,
    element: u32,
}

/// One input or output declaration of the IR.
struct Io {
    var: Handle<GlobalVariable>,
    ty: Ty,
    location: Option<(u32, u32)>,
    builtin: Option<u64>,
    interpolation: u64,
    /// The geometry stream an output belongs to.
    stream: u32,
}

/// Why a load's result is malformed: it is no scalar or vector.
const LOAD_NOT_VECTOR: &str = "a load's type that is no scalar or vector";
/// Why a storage texture operation is malformed: its texture is not a storage view.
const NOT_STORAGE_VIEW: &str = "a texture that is not a storage view";

/// An output of a geometry stream past the first in another stage.
const STREAM_OUTSIDE_GEOMETRY: &str = "a stream outside a geometry shader";

struct Block2 {
    label: u32,
    /// Ids of the block's instructions after its label, terminator included.
    first: u32,
    last: u32,
    phis: Vec<u32>,
}

/// Lowers a whole shader.
pub(crate) fn lower(ir: &Ir, variant: &VariantKey) -> Result<(Module, Reflection, ShaderStage)> {
    let mut l = Lowering::new(ir, variant)?;
    l.declare()?;
    l.functions()?;
    let stage = match l.stage {
        SourceStage::Vertex => ShaderStage::Vertex,
        SourceStage::Pixel => ShaderStage::Fragment,
        SourceStage::Geometry if variant.flags.contains(VariantFlags::GEOMETRY_REPLAY) => {
            ShaderStage::Vertex
        }
        SourceStage::Compute | SourceStage::Geometry | SourceStage::Hull => ShaderStage::Compute,
        SourceStage::Domain => ShaderStage::Vertex,
    };
    match l.stage {
        SourceStage::Geometry => l.geometry_entry_point()?,
        SourceStage::Hull => l.hull_entry_point()?,
        _ => l.entry_point()?,
    }
    crate::dxil::helper_invocations(&mut l.module);
    Ok((l.module, l.reflection, stage))
}

struct Lowering<'a> {
    ir: &'a Ir,
    variant: &'a VariantKey,
    module: Module,
    stage: SourceStage,
    set: u32,
    reflection: Reflection,
    resources: HashMap<ResourceElement, Resource>,
    /// The element count of every resource declaration, by declaring id.
    lengths: HashMap<u32, u32>,
    /// The element of the resource declaration being declared.
    element: u32,
    io: Vec<(u32, Io)>,
    io_index: HashMap<u32, usize>,
    /// Private, workgroup and constant-array variables by declaring id.
    memory: HashMap<u32, Handle<GlobalVariable>>,
    functions: HashMap<u32, Handle<Function>>,
    main: u32,
    helpers: Option<Handle<GlobalVariable>>,
    early_tests: bool,
    conservative: Option<naga::ConservativeDepth>,
    workgroup: [u32; 3],
    /// Images and samplers used in depth comparisons, by declaring id.
    compared: HashSet<u32>,
    /// Shared memory and storage buffers some atomic operation uses, by declaring id: their scalars
    /// are declared atomic. The bits say which minimum and maximum it takes ([`SIGNED_EXTREMES`],
    /// [`UNSIGNED_EXTREMES`]).
    atomic: HashMap<u32, u8>,
    semantics: HashMap<u32, (String, u32)>,
    discards: bool,
    side_effects: bool,
    /// What a geometry shader declares and the parts of its compute form made so far.
    geometry: Option<geometry::Geometry>,
    /// What a hull or domain shader declares.
    tessellation: Option<hull::Tessellation>,
    /// Typed buffers read through storage buffers, by element.
    texels: HashMap<ResourceElement, TexelBuffer>,
    /// Under Direct3D 12's numbering (shader model 5.1), every resource's (kind rank, space,
    /// register), sorted: a resource's slot is its place among its kind's.
    spaces: Option<Vec<(u32, u32, u32)>>,
    /// Inputs a pixel program takes at other places in the pixel.
    evaluated: evaluate::Evaluated,
}

impl<'a> Lowering<'a> {
    fn new(ir: &'a Ir, variant: &'a VariantKey) -> Result<Self> {
        let entry = ir
            .ops
            .iter()
            .position(|op| op.code == code::ENTRY_POINT)
            .ok_or_else(|| malformed("no entry point".into()))?;
        let entry = &ir.ops[entry];
        let stage = match entry.lits.last().copied().unwrap_or(0) {
            ir::stage::VERTEX => SourceStage::Vertex,
            ir::stage::PIXEL => SourceStage::Pixel,
            ir::stage::COMPUTE => SourceStage::Compute,
            ir::stage::GEOMETRY => SourceStage::Geometry,
            ir::stage::HULL => SourceStage::Hull,
            ir::stage::DOMAIN => SourceStage::Domain,
            _ => return Err(malformed("unknown stage".into())),
        };
        let main = *entry
            .refs
            .first()
            .ok_or_else(|| malformed("entry point without function".into()))?;
        let set = match stage {
            SourceStage::Pixel => 1,
            _ => 0,
        };
        let mut module = Module::default();
        derivatives_anywhere(&mut module);
        let spaces = ir.spaces.then(|| {
            let mut order: Vec<_> = ir
                .ops
                .iter()
                .filter_map(|op| {
                    let kind = declared_kind(op)?;
                    let [space, register] = [0, 1].map(|i| op.lits.get(i).map_or(0, |&l| l as u32));
                    Some((kind, space, register, op))
                })
                .map(|(kind, space, register, op)| {
                    let length = declared_length(variant, kind, op)?;
                    let end = register
                        .checked_add(length)
                        .ok_or_else(|| malformed(ARRAY_PAST_LAST_REGISTER.into()))?;
                    Ok((register..end).map(move |r| (rank(kind), space, r)))
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect();
            order.sort_unstable();
            order.dedup();
            Ok::<_, ConvertError>(order)
        });
        let spaces = spaces.transpose()?;
        Ok(Lowering {
            ir,
            variant,
            module,
            stage,
            set,
            reflection: Reflection {
                stage,
                inputs: Vec::new(),
                outputs: Vec::new(),
                bindings: Vec::new(),
                helpers: Vec::new(),
                helper_size: 0,
                sampled_pairs: Vec::new(),
                extras: StageExtras::None,
                slow_paths: Vec::new(),
            },
            resources: HashMap::default(),
            lengths: HashMap::default(),
            element: 0,
            io: Vec::new(),
            io_index: HashMap::default(),
            memory: HashMap::default(),
            functions: HashMap::default(),
            main,
            helpers: None,
            early_tests: false,
            conservative: None,
            workgroup: [1; 3],
            compared: HashSet::default(),
            atomic: HashMap::default(),
            semantics: HashMap::default(),
            discards: false,
            side_effects: false,
            geometry: (stage == SourceStage::Geometry).then(geometry::Geometry::default),
            tessellation: matches!(stage, SourceStage::Hull | SourceStage::Domain).then(|| {
                hull::Tessellation {
                    constants: entry.refs.get(1).copied().unwrap_or(0),
                    ..Default::default()
                }
            }),
            texels: HashMap::default(),
            spaces,
            evaluated: evaluate::Evaluated::default(),
        })
    }

    /// The slot of a resource of `kind` at `space` and `register`: the register under Direct3D 11's
    /// numbering, which knows one space, and its place among its kind's in (space, register) order
    /// under Direct3D 12's (6.4), as the DXIL route numbers them.
    fn slot(&self, kind: SlotKind, space: u32, register: u32) -> Result<u32> {
        let Some(order) = &self.spaces else {
            return match space {
                0 => Ok(register),
                _ => Err(unsupported(crate::refusal::REGISTER_SPACE)),
            };
        };
        let key = (rank(kind), space, register);
        let first = order.partition_point(|o| o.0 < key.0);
        Ok((order.partition_point(|o| *o < key) - first) as u32)
    }

    /// The slot of the element being declared of the resource declaration `op` of `kind`.
    fn declared_slot(&self, kind: SlotKind, op: &Op) -> Result<u32> {
        self.element_slot(kind, op, self.element)
    }

    /// The slot of element `element` of the resource declaration `op` of `kind`.
    fn element_slot(&self, kind: SlotKind, op: &Op, element: u32) -> Result<u32> {
        let [space, register] = [0, 1].map(|i| op.lits.get(i).map_or(0, |&l| l as u32));
        self.slot(kind, space, register + element)
    }

    /// The place of the element being declared of declaration `id`.
    fn place(&self, id: u32) -> ResourceElement {
        ResourceElement {
            dcl: id,
            element: self.element,
        }
    }

    // Types.

    fn scalar(s: u8) -> Result<Scalar> {
        Ok(match s {
            scalar::BOOL => Scalar::BOOL,
            scalar::I32 => Scalar::I32,
            scalar::U32 | scalar::UNKNOWN => Scalar::U32,
            scalar::F32 => Scalar::F32,
            scalar::I8
            | scalar::I16
            | scalar::U8
            | scalar::U16
            | scalar::F16
            | scalar::MIN_I16
            | scalar::MIN_U16
            | scalar::MIN_F16
            | scalar::MIN_F10 => return Err(unsupported(crate::refusal::TYPE_16_BIT)),
            scalar::I64 | scalar::U64 => {
                return Err(unsupported(crate::refusal::TYPE_64_BIT_INTEGER));
            }
            scalar::F64 => return Err(unsupported(crate::refusal::FLOAT_64)),
            _ => return Err(malformed(format!("scalar type {s}"))),
        })
    }

    fn ty(&mut self, inner: TypeInner) -> Handle<Type> {
        self.module
            .types
            .insert(Type { name: None, inner }, Span::UNDEFINED)
    }

    fn basic_inner(b: Basic) -> Result<TypeInner> {
        let scalar = Self::scalar(b.scalar)?;
        Ok(match b.size {
            1 => TypeInner::Scalar(scalar),
            n => TypeInner::Vector {
                size: vector_size(n)?,
                scalar,
            },
        })
    }

    fn basic(&mut self, b: Basic) -> Result<Handle<Type>> {
        let inner = Self::basic_inner(b)?;
        Ok(self.ty(inner))
    }

    fn scalar_ty(&mut self, s: Scalar) -> Handle<Type> {
        self.ty(TypeInner::Scalar(s))
    }

    fn vec_ty(&mut self, n: u8, s: Scalar) -> Result<Handle<Type>> {
        self.basic(Basic {
            scalar: scalar_code(s),
            size: n,
        })
    }

    /// The naga type of an IR type with one member, arrays included.
    fn ir_ty(&mut self, t: &Ty) -> Result<Handle<Type>> {
        self.ir_ty_of(t, false)
    }

    /// The IR type of declaration `dcl`, its scalar the one its atomic operations take: signed where
    /// they take signed minimums or maximums, unsigned where unsigned ones or both (the signed ones
    /// then made by a compare-exchange loop).
    fn declared_ty(&self, dcl: u32) -> Result<Ty> {
        let mut t = self.ir.op(dcl)?.ty.clone();
        let scalar = match self.atomic.get(&dcl) {
            Some(&SIGNED_EXTREMES) => scalar::I32,
            Some(&s) if s & UNSIGNED_EXTREMES != 0 => scalar::U32,
            _ => return Ok(t),
        };
        if let [member] = &mut t.members[..] {
            member.scalar = scalar;
        }
        Ok(t)
    }

    /// [`Self::ir_ty`], with the member atomic where `atomic` (a 32-bit integer).
    fn ir_ty_of(&mut self, t: &Ty, atomic: bool) -> Result<Handle<Type>> {
        let member = match t.members[..] {
            [member] => member,
            [] => return Err(malformed("a declaration without a type".into())),
            _ if atomic => return Err(unsupported(crate::refusal::STRUCT_ATOMIC)),
            // A struct (dxbc-spirv's structured group-shared memory of members of several types),
            // each member at the next offset its alignment allows.
            _ => {
                let (handle, size, align) = self.struct_ty(&t.members)?;
                return self.arrays(t, handle, size, align);
            }
        };
        let handle = if atomic {
            let s = Self::scalar(member.scalar)?;
            if member.size != 1 || !matches!(s, Scalar::U32 | Scalar::I32) {
                return Err(unsupported(crate::refusal::ATOMIC_TYPE));
            }
            self.ty(TypeInner::Atomic(s))
        } else {
            self.basic(member)?
        };
        self.arrays(t, handle, basic_size(member), basic_align(member))
    }

    /// A struct of `members`, with its size and alignment.
    fn struct_ty(&mut self, members: &[Basic]) -> Result<(Handle<Type>, u32, u32)> {
        let mut offset = 0u32;
        let mut align = 4u32;
        let mut fields = Vec::new();
        for (i, &m) in members.iter().enumerate() {
            let at = offset.next_multiple_of(basic_align(m));
            let ty = self.basic(m)?;
            fields.push(StructMember {
                name: Some(format!("m{i}")),
                ty,
                binding: None,
                offset: at,
            });
            offset = at + basic_size(m);
            align = align.max(basic_align(m));
        }
        let span = offset.next_multiple_of(align);
        let ty = self.ty(TypeInner::Struct {
            members: fields,
            span,
        });
        Ok((ty, span, align))
    }

    /// `t`'s array dimensions around an element of type `handle`, `size` bytes aligned to `align`.
    fn arrays(
        &mut self,
        t: &Ty,
        mut handle: Handle<Type>,
        mut size: u32,
        mut align: u32,
    ) -> Result<Handle<Type>> {
        for &count in &t.dims {
            let stride = size.next_multiple_of(align);
            handle = self.ty(TypeInner::Array {
                base: handle,
                size: match NonZeroU32::new(count) {
                    Some(n) => ArraySize::Constant(n),
                    None => ArraySize::Dynamic,
                },
                stride,
            });
            size = stride * count.max(1);
            align = align.max(4);
        }
        Ok(handle)
    }

    fn global(
        &mut self,
        name: String,
        space: AddressSpace,
        binding: Option<ResourceBinding>,
        ty: Handle<Type>,
        init: Option<Handle<Expression>>,
    ) -> Handle<GlobalVariable> {
        self.module.global_variables.append(
            GlobalVariable {
                name: Some(name),
                space,
                binding,
                ty,
                init,
                memory_decorations: Default::default(),
            },
            Span::UNDEFINED,
        )
    }

    // Declarations.

    fn declare(&mut self) -> Result<()> {
        // Which images and samplers are compared against decides their types.
        for op in &self.ir.ops {
            let compare = match op.code {
                code::IMAGE_SAMPLE => op.refs.get(10).copied().unwrap_or(0) != 0,
                code::IMAGE_GATHER => op.refs.get(5).copied().unwrap_or(0) != 0,
                _ => false,
            };
            if compare {
                for &d in &op.refs[..2] {
                    let dcl = self.ir.op(d)?.refs[0];
                    self.compared.insert(dcl);
                }
            }
            let dcl = match op.code {
                code::LDS_ATOMIC => op.refs[0],
                code::BUFFER_ATOMIC | code::IMAGE_ATOMIC => self.ir.op(op.refs[0])?.refs[0],
                _ => continue,
            };
            let sign = match op.lits.first() {
                Some(&atomic_op::S_MIN | &atomic_op::S_MAX) => SIGNED_EXTREMES,
                Some(&atomic_op::U_MIN | &atomic_op::U_MAX) => UNSIGNED_EXTREMES,
                _ => 0,
            };
            *self.atomic.entry(dcl).or_default() |= sign;
        }
        for id in 1..=self.ir.ops.len() as u32 {
            let op = self.ir.op(id)?;
            if op.code > code::LAST_DECLARATIVE {
                break;
            }
            self.declare_op(id, op)?;
        }
        if self.stage == SourceStage::Pixel {
            self.declare_evaluated()?;
        }
        if let SourceStage::Vertex = self.stage
            && self.variant.draw_base != DrawBaseSource::None
            && self.io.iter().any(|(_, io)| {
                matches!(io.builtin, Some(builtin::VERTEX_ID | builtin::INSTANCE_ID))
            })
        {
            self.helper_block()?;
        }
        if self.geometry.is_some() {
            self.geometry_functions()?;
        }
        Ok(())
    }

    fn declare_op(&mut self, id: u32, op: &Op) -> Result<()> {
        match op.code {
            code::SEMANTIC => {
                let index = op.lits.first().copied().unwrap_or(0) as u32;
                let name = ir::literal_string(op.lits.get(1..).unwrap_or(&[]));
                self.semantics.insert(op.refs[0], (name, index));
            }
            code::SET_CS_WORKGROUP_SIZE => {
                for (w, &l) in self.workgroup.iter_mut().zip(&op.lits) {
                    *w = l as u32;
                }
            }
            code::SET_PS_EARLY_FRAGMENT_TEST => self.early_tests = true,
            code::SET_PS_DEPTH_GREATER_EQUAL => {
                self.conservative = Some(naga::ConservativeDepth::GreaterEqual)
            }
            code::SET_PS_DEPTH_LESS_EQUAL => {
                self.conservative = Some(naga::ConservativeDepth::LessEqual)
            }
            code::DCL_INPUT
            | code::DCL_INPUT_BUILT_IN
            | code::DCL_OUTPUT
            | code::DCL_OUTPUT_BUILT_IN => self.declare_io(id, op)?,
            code::SET_GS_INSTANCES..=code::SET_GS_OUTPUT_PRIMITIVE => {
                self.geometry_declaration(op)?
            }
            code::SET_TESS_PRIMITIVE..=code::SET_TESS_CONTROL_POINTS => {
                self.tessellation_declaration(op)?
            }
            code::DCL_SAMPLER | code::DCL_CBV | code::DCL_SRV | code::DCL_UAV => {
                self.declare_resource(id, op)?
            }
            code::DCL_UAV_COUNTER => {
                // One atomic word of its own, at the counter binding of the view's slot.
                let uav = self.ir.op(op.refs[1])?;
                if self.lengths.get(&op.refs[1]) != Some(&1) {
                    return Err(unsupported(crate::refusal::RESOURCE_ARRAY));
                }
                let ty = self.ty(TypeInner::Atomic(Scalar::U32));
                let space = AddressSpace::Storage {
                    access: naga::StorageAccess::LOAD | naga::StorageAccess::STORE,
                };
                self.resource_in(id, uav, SlotKind::Counter, ty, space, None, false)?;
            }
            code::DCL_LDS => {
                let ty = self.declared_ty(id)?;
                let ty = self.ir_ty_of(&ty, self.atomic.contains_key(&id))?;
                let var = self.global(format!("lds{id}"), AddressSpace::WorkGroup, None, ty, None);
                self.memory.insert(id, var);
            }
            code::DCL_SCRATCH => {
                // dxbc-spirv leaves a local array it resolved elsewhere declared without a type.
                if op.ty.is_void() && !self.ir.ops.iter().any(|o| o.refs.contains(&id)) {
                    return Ok(());
                }
                let ty = self.ir_ty(&op.ty)?;
                let var = self.global(
                    format!("scratch{id}"),
                    AddressSpace::Private,
                    None,
                    ty,
                    None,
                );
                self.memory.insert(id, var);
            }
            code::DCL_TMP => return Err(malformed("temporary left after SSA".into())),
            code::DCL_INPUT_TARGET => return Err(unsupported(crate::refusal::INPUT_TARGET)),
            36 | 37 => return Err(unsupported(crate::refusal::PUSH_DATA)),
            47 => return Err(unsupported(crate::refusal::STREAM_OUTPUT)),
            code::DCL_PARAM
            | code::ENTRY_POINT
            | code::DEBUG_NAME
            | code::DEBUG_MEMBER_NAME
            | code::CONSTANT
            | code::UNDEF
            | code::SET_FP_MODE => {}
            _ => return Err(malformed(format!("declaration {}", op.code))),
        }
        Ok(())
    }

    /// Declares every element of resource declaration `id`, each a binding of its own at its register.
    fn declare_resource(&mut self, id: u32, op: &Op) -> Result<()> {
        let kind = declared_kind(op)
            .ok_or_else(|| malformed(format!("declaration {id} is not a resource")))?;
        let length = declared_length(self.variant, kind, op)?;
        self.lengths.insert(id, length);
        for element in 0..length {
            self.element = element;
            self.declare_element(id, op)?;
        }
        self.element = 0;
        Ok(())
    }

    fn declare_element(&mut self, id: u32, op: &Op) -> Result<()> {
        match op.code {
            code::DCL_SAMPLER => {
                let compare = self.compared.contains(&id);
                let ty = self.ty(TypeInner::Sampler {
                    comparison: compare,
                });
                self.resource(id, op, SlotKind::Sampler, ty, None, false)?;
            }
            code::DCL_CBV => {
                let ty = self.ir_ty(&op.ty)?;
                self.resource(id, op, SlotKind::Uniform, ty, None, false)?;
            }
            code::DCL_SRV | code::DCL_UAV => {
                let storage = op.code == code::DCL_UAV;
                if storage
                    && op
                        .lits
                        .get(ir::resource_literal::UAV_FLAGS)
                        .is_some_and(|f| f & ir::uav::COHERENT != 0)
                {
                    return Err(unsupported(crate::refusal::GLOBALLY_COHERENT));
                }
                let k = op.lits[ir::resource_literal::KIND];
                let slot = if storage {
                    SlotKind::Storage
                } else {
                    SlotKind::Texture
                };
                match k {
                    kind::BUFFER_TYPED => self.texel_buffer(id, op, storage)?,
                    kind::BUFFER_RAW | kind::BUFFER_STRUCTURED => {
                        let ty = self.declared_ty(id)?;
                        let ty = self.ir_ty_of(&ty, self.atomic.contains_key(&id))?;
                        let read_only = !storage
                            || op
                                .lits
                                .get(ir::resource_literal::UAV_FLAGS)
                                .is_some_and(|f| f & ir::uav::READ_ONLY != 0);
                        let access = if read_only {
                            naga::StorageAccess::LOAD
                        } else {
                            naga::StorageAccess::LOAD | naga::StorageAccess::STORE
                        };
                        let space = AddressSpace::Storage { access };
                        self.resource_in(id, op, slot, ty, space, None, false)?;
                    }
                    _ if storage
                        && self.declared_slot(SlotKind::Storage, op).is_ok_and(|r| {
                            r < u64::BITS && self.variant.texels_through_buffer >> r & 1 != 0
                        }) =>
                    {
                        self.texel_image(id, op, k)?
                    }
                    _ if storage => {
                        let (dim, arrayed, multi) = image_dim(k)?;
                        if multi {
                            return Err(unsupported(crate::refusal::MULTISAMPLED_STORAGE));
                        }
                        let ty = self.storage_image(op, dim, arrayed)?;
                        self.resource(id, op, slot, ty, Some((k, arrayed)), false)?;
                    }
                    _ => {
                        let (dim, arrayed, multi) = image_dim(k)?;
                        let depth = self.compared.contains(&id);
                        let member = op.ty.basic().ok_or_else(|| {
                            malformed(format!("image declaration {id} without its type"))
                        })?;
                        let class = if depth {
                            ImageClass::Depth { multi }
                        } else {
                            ImageClass::Sampled {
                                kind: Self::scalar(member.scalar)?.kind,
                                multi,
                            }
                        };
                        let ty = self.ty(TypeInner::Image {
                            dim,
                            arrayed,
                            class,
                        });
                        self.resource(id, op, slot, ty, Some((k, arrayed)), depth)?;
                    }
                }
            }
            _ => return Err(malformed(format!("declaration {}", op.code))),
        }
        Ok(())
    }

    /// A typed buffer, read through a storage buffer of words or a 2D texture by the variant's route:
    /// its format from the variant (the bound view's), its words atomic where an atomic operation uses
    /// them or where elements narrower than a word are written.
    fn texel_buffer(&mut self, id: u32, op: &Op, storage: bool) -> Result<()> {
        let texture = match self.variant.texel_buffers {
            TexelBufferRoute::Native => return Err(unsupported(crate::refusal::TYPED_BUFFER)),
            TexelBufferRoute::Storage => false,
            TexelBufferRoute::Texture if !storage => true,
            TexelBufferRoute::Texture => {
                return Err(unsupported(crate::refusal::TYPED_BUFFER_STORAGE_AS_TEXTURE));
            }
        };
        let kind = if storage {
            SlotKind::Storage
        } else {
            SlotKind::Texture
        };
        let register = self.declared_slot(kind, op)?;
        let formats = if storage {
            &self.variant.storage_formats
        } else {
            &self.variant.texel_formats
        };
        let format = formats
            .iter()
            .find(|(slot, _)| *slot == register)
            .ok_or(unsupported(crate::refusal::TYPED_BUFFER_FORMAT))?
            .1;
        let member = op
            .ty
            .basic()
            .ok_or_else(|| malformed("typed buffer type".into()))?;
        let declared = Self::scalar(member.scalar)?;
        let layout = TexelBuffer::layout(format)?;
        let ir = self.ir;
        let written = storage
            && ir.ops.iter().any(|o| {
                o.code == code::BUFFER_STORE && ir.op(o.refs[0]).is_ok_and(|d| d.refs[0] == id)
            });
        let atomic = self.texel_atomic(id, declared, written, layout.2);
        let ty = if texture {
            self.ty(TypeInner::Image {
                dim: ImageDimension::D2,
                arrayed: false,
                class: ImageClass::Sampled {
                    kind: TexelBuffer::view_kind(&layout),
                    multi: false,
                },
            })
        } else {
            TexelBuffer::words(&mut self.module, atomic)
        };
        let read_only = !storage
            || op
                .lits
                .get(ir::resource_literal::UAV_FLAGS)
                .is_some_and(|f| f & ir::uav::READ_ONLY != 0);
        let access = if read_only {
            naga::StorageAccess::LOAD
        } else {
            naga::StorageAccess::LOAD | naga::StorageAccess::STORE
        };
        let slot = if storage {
            SlotKind::Storage
        } else {
            SlotKind::Texture
        };
        let space = if texture {
            AddressSpace::Handle
        } else {
            AddressSpace::Storage { access }
        };
        self.resource_in(id, op, slot, ty, space, None, false)?;
        let var = self.resources[&self.place(id)].var;
        let binding = self.module.global_variables[var].binding.unwrap().binding;
        let stage = match self.stage {
            SourceStage::Vertex => ShaderStage::Vertex,
            SourceStage::Pixel => ShaderStage::Fragment,
            _ => ShaderStage::Compute,
        };
        let place = if texture {
            TexelPlace::Texture
        } else {
            TexelPlace::Words
        };
        let buffer = TexelBuffer::declared(
            var,
            layout,
            declared.kind,
            atomic,
            place,
            stage,
            self.set,
            binding,
        );
        self.texels.insert(self.place(id), buffer);
        Ok(())
    }

    /// The scalar of a typed view's words where they are atomic: where an atomic operation uses them
    /// (float bits exchanged as u32), or where elements narrower than a word are written.
    fn texel_atomic(&self, id: u32, declared: Scalar, written: bool, size: u32) -> Option<Scalar> {
        match self.atomic.get(&id) {
            Some(_) if declared.kind == ScalarKind::Float => Some(Scalar::U32),
            Some(_) => Some(declared),
            None if written && TexelBuffer::narrow(size) => Some(Scalar::U32),
            None => None,
        }
    }

    /// A storage texture read and written through a buffer of its texels
    /// (`VariantKey::texels_through_buffer`): its format from the variant (the bound view's), laid out
    /// as `HelperValue::TexelLayout` says.
    fn texel_image(&mut self, id: u32, op: &Op, k: u64) -> Result<()> {
        let (_, arrayed, multi) = image_dim(k)?;
        if multi {
            return Err(unsupported(crate::refusal::MULTISAMPLED_STORAGE));
        }
        if self.stage == SourceStage::Geometry {
            return Err(unsupported(
                crate::refusal::TEXELS_THROUGH_BUFFER_IN_GEOMETRY,
            ));
        }
        let register = self.declared_slot(SlotKind::Storage, op)?;
        let format = self
            .variant
            .storage_formats
            .iter()
            .find(|(slot, _)| *slot == register)
            .ok_or(unsupported(crate::refusal::STORAGE_FORMAT))?
            .1;
        let member = op
            .ty
            .basic()
            .ok_or_else(|| malformed("storage image type".into()))?;
        let declared = Self::scalar(member.scalar)?;
        let layout = TexelBuffer::layout(format).map_err(|e| match e {
            ConvertError::Unsupported(crate::refusal::TYPED_BUFFER_FORMAT_ABSENT) => {
                unsupported(crate::refusal::STORAGE_FORMAT_ABSENT)
            }
            e => e,
        })?;
        let ir = self.ir;
        let written = ir.ops.iter().any(|o| {
            o.code == code::IMAGE_STORE && ir.op(o.refs[0]).is_ok_and(|d| d.refs[0] == id)
        });
        let atomic = self.texel_atomic(id, declared, written, layout.2);
        let words = TexelBuffer::words(&mut self.module, atomic);
        let access = if op
            .lits
            .get(ir::resource_literal::UAV_FLAGS)
            .is_some_and(|f| f & ir::uav::READ_ONLY != 0)
        {
            naga::StorageAccess::LOAD
        } else {
            naga::StorageAccess::LOAD | naga::StorageAccess::STORE
        };
        let space = AddressSpace::Storage { access };
        self.resource_in(
            id,
            op,
            SlotKind::Storage,
            words,
            space,
            Some((k, arrayed)),
            false,
        )?;
        let var = self.resources[&self.place(id)].var;
        let binding = self.module.global_variables[var].binding.unwrap().binding;
        let stage = match self.stage {
            SourceStage::Vertex => ShaderStage::Vertex,
            SourceStage::Pixel => ShaderStage::Fragment,
            _ => ShaderStage::Compute,
        };
        let place = TexelPlace::Image(register);
        let buffer = TexelBuffer::declared(
            var,
            layout,
            declared.kind,
            atomic,
            place,
            stage,
            self.set,
            binding,
        );
        self.texels.insert(self.place(id), buffer);
        Ok(())
    }

    /// The type of a storage texture view: its format from the variant (the bound view's), its access
    /// from the declaration. Read-write access exists in WebGPU for 32-bit single-channel formats only.
    fn storage_image(
        &mut self,
        op: &Op,
        dim: ImageDimension,
        arrayed: bool,
    ) -> Result<Handle<Type>> {
        let register = self.declared_slot(SlotKind::Storage, op)?;
        let format = self
            .variant
            .storage_formats
            .iter()
            .find(|(slot, _)| *slot == register)
            .ok_or(unsupported(crate::refusal::STORAGE_FORMAT))?
            .1;
        let format =
            storage_form(format).ok_or(unsupported(crate::refusal::STORAGE_FORMAT_ABSENT))?;
        let flags = op
            .lits
            .get(ir::resource_literal::UAV_FLAGS)
            .copied()
            .unwrap_or(0);
        let access = if flags & ir::uav::READ_ONLY != 0 {
            naga::StorageAccess::LOAD
        } else if flags & ir::uav::WRITE_ONLY != 0 {
            naga::StorageAccess::STORE
        } else {
            use naga::StorageFormat as S;
            if !matches!(format, S::R32Float | S::R32Uint | S::R32Sint) {
                return Err(unsupported(crate::refusal::READ_WRITE_STORAGE_FORMAT));
            }
            naga::StorageAccess::LOAD | naga::StorageAccess::STORE
        };
        Ok(self.ty(TypeInner::Image {
            dim,
            arrayed,
            class: ImageClass::Storage { format, access },
        }))
    }

    fn resource(
        &mut self,
        id: u32,
        op: &Op,
        slot: SlotKind,
        ty: Handle<Type>,
        image: Option<(u64, bool)>,
        depth: bool,
    ) -> Result<()> {
        let space = match slot {
            SlotKind::Uniform => AddressSpace::Uniform,
            _ => AddressSpace::Handle,
        };
        self.resource_in(id, op, slot, ty, space, image, depth)
    }

    #[allow(clippy::too_many_arguments)]
    fn resource_in(
        &mut self,
        id: u32,
        op: &Op,
        slot: SlotKind,
        ty: Handle<Type>,
        space: AddressSpace,
        image: Option<(u64, bool)>,
        depth: bool,
    ) -> Result<()> {
        let [space_index, register] = [0, 1].map(|i| op.lits.get(i).map(|&l| l as u32));
        let (Some(space_index), Some(register)) = (space_index, register) else {
            return Err(malformed(
                "a resource declaration without its register".into(),
            ));
        };
        let register = register + self.element;
        let number = self.slot(slot, space_index, register)?;
        let range = SLOT_KINDS[slot as usize];
        let binding = (number < range.slots)
            .then(|| range.first_binding + number)
            .ok_or(unsupported(crate::refusal::BINDINGS_BEYOND_LIMIT))?;
        let prefix = match slot {
            SlotKind::Uniform => register_prefix::UNIFORM,
            SlotKind::Sampler => register_prefix::SAMPLER,
            SlotKind::Texture => register_prefix::TEXTURE,
            SlotKind::Storage => register_prefix::STORAGE,
            SlotKind::Counter => register_prefix::COUNTER,
        };
        let name = match space_index {
            0 => format!("{prefix}{register}"),
            _ => format!("{prefix}{register}{}{space_index}", register_prefix::SPACE),
        };
        let var = self.global(
            name,
            space,
            Some(ResourceBinding {
                group: self.set,
                binding,
            }),
            ty,
            None,
        );
        let storage = slot == SlotKind::Storage;
        self.resources.insert(
            self.place(id),
            Resource {
                var,
                image,
                depth,
                storage,
            },
        );
        self.reflection.bindings.push(ReflectedBinding {
            kind: slot,
            space: space_index,
            register,
            count: 1,
            set: self.set,
            binding,
            added: None,
            stride: 0,
        });
        Ok(())
    }

    fn declare_io(&mut self, id: u32, op: &Op) -> Result<()> {
        let input = matches!(op.code, code::DCL_INPUT | code::DCL_INPUT_BUILT_IN);
        let (location, builtin, interpolation) = match op.code {
            code::DCL_INPUT => (
                Some((op.lits[0] as u32, op.lits[1] as u32)),
                None,
                op.lits.get(2).copied().unwrap_or(0),
            ),
            code::DCL_OUTPUT => (Some((op.lits[0] as u32, op.lits[1] as u32)), None, 0),
            code::DCL_INPUT_BUILT_IN => {
                (None, Some(op.lits[0]), op.lits.get(1).copied().unwrap_or(0))
            }
            _ => (None, Some(op.lits[0]), 0),
        };
        if let Some(b) = builtin {
            self.check_builtin(b, input)?;
        }
        let stream = match op.code {
            code::DCL_OUTPUT => op.lits.get(2),
            code::DCL_OUTPUT_BUILT_IN => op.lits.get(1),
            _ => None,
        };
        let stream = stream.map_or(0, |&s| s as u32);
        if stream != 0 && self.geometry.is_none() {
            return Err(malformed(STREAM_OUTSIDE_GEOMETRY.into()));
        }
        let ty = self.ir_ty(&op.ty)?;
        let name = format!("{}{id}", if input { "in" } else { "out" });
        let var = self.global(name, AddressSpace::Private, None, ty, None);
        self.io_index.insert(id, self.io.len());
        self.io.push((
            id,
            Io {
                var,
                ty: op.ty.clone(),
                location,
                builtin,
                interpolation,
                stream,
            },
        ));
        Ok(())
    }

    /// A pixel program's clip or cull distance inputs (built-in `b`, input `i` among them): one array
    /// argument of every declaration's distances in order, as the vertex program writes them, each
    /// declaration given its part. Only the first declaration makes it.
    fn distance_input(
        &mut self,
        f: &mut Function,
        body: &mut Block,
        e: &mut Emit,
        i: usize,
        b: u64,
    ) -> Result<()> {
        let input = |c: u16| matches!(c, code::DCL_INPUT | code::DCL_INPUT_BUILT_IN);
        let clips: Vec<usize> = (0..self.io.len())
            .filter(|&j| {
                let (other, ref io) = self.io[j];
                io.builtin == Some(b) && self.ir.op(other).is_ok_and(|o| input(o.code))
            })
            .collect();
        if clips[0] != i {
            return Ok(());
        }
        let mut parts = Vec::new();
        let mut total = 0;
        for &j in &clips {
            let io = &self.io[j].1;
            let element = io.ty.members[..]
                == [Basic {
                    scalar: scalar::F32,
                    size: 1,
                }];
            if !element || io.ty.dims.len() > 1 {
                return Err(unsupported(CLIP_DISTANCE_TYPE));
            }
            let count: u32 = io.ty.dims.iter().product();
            parts.push((j, total, count));
            total += count;
        }
        if total > CLIP_OR_CULL_DISTANCES {
            return Err(unsupported(CLIP_DISTANCE_TYPE));
        }
        let f32_ty = self.scalar_ty(Scalar::F32);
        let array = self.ty(TypeInner::Array {
            base: f32_ty,
            size: ArraySize::Constant(NonZeroU32::new(total).unwrap()),
            stride: core::mem::size_of::<f32>() as u32,
        });
        let distances = match b {
            builtin::CLIP_DISTANCE => naga::BuiltIn::ClipDistances,
            _ => naga::BuiltIn::CullDistance,
        };
        f.arguments.push(FunctionArgument {
            name: Some(format!("b{b}")),
            ty: array,
            binding: Some(naga::Binding::BuiltIn(distances)),
        });
        let argument = (f.arguments.len() - 1) as u32;
        let a = e.add(f, body, Expression::FunctionArgument(argument));
        for (j, start, count) in parts {
            let (id, ref io) = self.io[j];
            let (var, ty, modes) = (io.var, io.ty.clone(), io.interpolation);
            let value = if ty.dims.is_empty() {
                e.add(
                    f,
                    body,
                    Expression::AccessIndex {
                        base: a,
                        index: start,
                    },
                )
            } else {
                let components = (start..start + count)
                    .map(|index| e.add(f, body, Expression::AccessIndex { base: a, index }))
                    .collect();
                let ty = self.module.global_variables[var].ty;
                e.add(f, body, Expression::Compose { ty, components })
            };
            let p = e.add(f, body, Expression::GlobalVariable(var));
            e.stmt(f, body, Statement::Store { pointer: p, value });
            self.reflect_value(id, true, None, 0, ty.members[0], modes, system_value(b));
        }
        Ok(())
    }

    fn check_builtin(&self, b: u64, input: bool) -> Result<()> {
        let ok = match (self.stage, input, b) {
            (_, false, builtin::POSITION) => matches!(
                self.stage,
                SourceStage::Vertex
                    | SourceStage::Geometry
                    | SourceStage::Domain
                    | SourceStage::Hull
            ),
            (
                SourceStage::Hull,
                true,
                builtin::POSITION
                | builtin::PRIMITIVE_ID
                | builtin::TESS_CONTROL_POINT_ID
                | builtin::TESS_CONTROL_POINT_COUNT_IN,
            )
            | (SourceStage::Hull, false, builtin::TESS_FACTOR_INNER | builtin::TESS_FACTOR_OUTER)
            | (
                SourceStage::Domain,
                true,
                builtin::POSITION
                | builtin::PRIMITIVE_ID
                | builtin::TESS_COORD
                | builtin::TESS_CONTROL_POINT_COUNT_IN
                | builtin::TESS_FACTOR_INNER
                | builtin::TESS_FACTOR_OUTER,
            ) => true,
            (
                SourceStage::Geometry,
                true,
                builtin::POSITION
                | builtin::PRIMITIVE_ID
                | builtin::GS_INSTANCE_ID
                | builtin::GS_VERTEX_COUNT_IN
                // Read from the vertex records, which the stage before fills
                // ([`crate::stage::Stages::new`]).
                | builtin::CLIP_DISTANCE
                | builtin::CULL_DISTANCE,
            ) => true,
            (
                SourceStage::Vertex | SourceStage::Domain | SourceStage::Geometry,
                false,
                builtin::CLIP_DISTANCE,
            ) => true,
            (
                SourceStage::Geometry,
                false,
                builtin::LAYER_INDEX | builtin::VIEWPORT_INDEX | builtin::PRIMITIVE_ID,
            ) => true,
            // Drawn through the stage helper ([`crate::stage::Stages::culled`], its geometry stage's
            // cull program, and its domain stage's).
            (
                SourceStage::Vertex | SourceStage::Domain | SourceStage::Geometry,
                false,
                builtin::CULL_DISTANCE,
            ) => true,
            // Read from the linked values the vertex program passes them in.
            (SourceStage::Pixel, true, builtin::CLIP_DISTANCE) => {
                self.variant
                    .flags
                    .contains(wcontainers_gpu::VariantFlags::CLIP_DISTANCE)
                    && self.variant.linked.clip_distances > 0
            }
            // Read from the values the vertex program passes them in.
            (SourceStage::Pixel, true, builtin::CULL_DISTANCE) => self
                .variant
                .flags
                .contains(wcontainers_gpu::VariantFlags::CULL_DISTANCE_LINKED),
            (SourceStage::Vertex, true, builtin::VERTEX_ID | builtin::INSTANCE_ID) => true,
            (_, true, builtin::SAMPLE_COUNT) => self.variant.samples != 0,
            (SourceStage::Pixel, true, builtin::POSITION | builtin::IS_FRONT_FACE)
            | (SourceStage::Pixel, true, builtin::PRIMITIVE_ID)
            | (SourceStage::Pixel, true, builtin::SAMPLE_ID | builtin::SAMPLE_MASK)
            | (SourceStage::Pixel, true, builtin::LAYER_INDEX | builtin::VIEWPORT_INDEX)
            | (SourceStage::Pixel, false, builtin::DEPTH | builtin::SAMPLE_MASK) => true,
            (
                SourceStage::Compute,
                true,
                builtin::WORKGROUP_ID
                | builtin::GLOBAL_THREAD_ID
                | builtin::LOCAL_THREAD_ID
                | builtin::LOCAL_THREAD_INDEX,
            ) => true,
            _ => false,
        };
        if ok {
            return Ok(());
        }
        Err(unsupported(match b {
            builtin::CLIP_DISTANCE => crate::refusal::CLIP_DISTANCE,
            builtin::CULL_DISTANCE => crate::refusal::CULL_DISTANCE,
            builtin::PRIMITIVE_ID => crate::refusal::PRIMITIVE_ID,
            builtin::LAYER_INDEX => crate::refusal::RENDER_TARGET_ARRAY_INDEX,
            builtin::VIEWPORT_INDEX => crate::refusal::VIEWPORT_ARRAY_INDEX,
            builtin::SAMPLE_COUNT => crate::refusal::SAMPLE_COUNT,
            builtin::SAMPLE_POSITION => crate::refusal::SAMPLE_POSITION,
            builtin::STENCIL_REF => crate::refusal::STENCIL_REF,
            builtin::IS_FULLY_COVERED => crate::refusal::INNER_COVERAGE,
            _ => crate::refusal::BUILT_IN,
        }))
    }

    /// The vertex stage's helper constants: the draw's bases at offset 0 (for bases from an indirect
    /// draw's arguments too, which the variant pass then reads from them).
    fn helper_block(&mut self) -> Result<()> {
        let (ty, size) = crate::build::draw_bases_block(&mut self.module);
        let var = self.global(
            crate::build::HELPERS.into(),
            AddressSpace::Uniform,
            Some(ResourceBinding {
                group: wcontainers_gpu::HELPER_SET,
                binding: helper_binding(ShaderStage::Vertex),
            }),
            ty,
            None,
        );
        self.helpers = Some(var);
        self.reflection.helpers.push(HelperSlot {
            value: HelperValue::DrawBase,
            offset: 0,
        });
        self.reflection.helper_size = size;
        Ok(())
    }

    // Functions.

    fn functions(&mut self) -> Result<()> {
        let mut starts = Vec::new();
        for (i, op) in self.ir.ops.iter().enumerate() {
            if op.code == code::FUNCTION {
                starts.push(i as u32 + 1);
            }
        }
        // Callees come before callers in naga's arena: lower in reverse order of first call.
        let mut order = Vec::new();
        let mut seen = HashSet::default();
        self.call_order(self.main, &mut order, &mut seen)?;
        if let Some(t) = &self.tessellation
            && t.constants != 0
        {
            self.call_order(t.constants, &mut order, &mut seen)?;
        }
        for f in order {
            let handle = FunctionLowering::lower(self, f)?;
            self.functions.insert(f, handle);
        }
        Ok(())
    }

    fn call_order(&self, f: u32, order: &mut Vec<u32>, seen: &mut HashSet<u32>) -> Result<()> {
        if !seen.insert(f) {
            return Ok(());
        }
        let mut id = f + 1;
        loop {
            let op = self.ir.op(id)?;
            match op.code {
                code::FUNCTION_END => break,
                code::FUNCTION_CALL => self.call_order(op.refs[0], order, seen)?,
                _ => {}
            }
            id += 1;
        }
        order.push(f);
        Ok(())
    }

    // The entry point wrapper.

    fn entry_point(&mut self) -> Result<()> {
        let domain_inputs = match self.stage {
            SourceStage::Domain => Some(self.domain_inputs()?),
            _ => None,
        };
        let mut f = Function {
            name: Some(crate::ENTRY_POINT.into()),
            ..Default::default()
        };
        let mut body = Block::new();
        let mut e = Emit::new(&f);
        let input = |c: u16| matches!(c, code::DCL_INPUT | code::DCL_INPUT_BUILT_IN);
        // A domain program's inputs all come from its vertex number.
        if let Some(function) = domain_inputs {
            let u32_ty = self.scalar_ty(Scalar::U32);
            f.arguments.push(FunctionArgument {
                name: Some(hull::DOMAIN_VERTEX_ARGUMENT.into()),
                ty: u32_ty,
                binding: Some(naga::Binding::BuiltIn(naga::BuiltIn::VertexIndex)),
            });
            let a = e.add(&mut f, &mut body, Expression::FunctionArgument(0));
            e.stmt(
                &f,
                &mut body,
                Statement::Call {
                    function,
                    arguments: vec![a],
                    result: None,
                },
            );
        }

        // Inputs.
        let mut locations: Vec<(u32, Scalar, u64, usize)> = Vec::new();
        let io_count = self.io.len();
        for i in 0..io_count {
            let (id, ref io) = self.io[i];
            if !input(self.ir.op(id)?.code) || domain_inputs.is_some() {
                continue;
            }
            let (var, location, builtin, modes) =
                (io.var, io.location, io.builtin, io.interpolation);
            let member = io.ty.basic();
            if let Some((loc, comp)) = location {
                let member = member.ok_or(unsupported(crate::refusal::INPUT_ARRAY))?;
                let s = Self::scalar(member.scalar)?;
                let arg = match locations.iter().find(|l| l.0 == loc) {
                    Some(l) => l.3,
                    None => {
                        let s = if s == Scalar::BOOL { Scalar::U32 } else { s };
                        let vty = self.vec_ty(4, s)?;
                        let flat = modes & interp::FLAT != 0 || s.kind != ScalarKind::Float;
                        let binding = self.location_binding(loc, modes, flat, true);
                        f.arguments.push(FunctionArgument {
                            name: Some(format!("v{loc}")),
                            ty: vty,
                            binding: Some(binding),
                        });
                        let index = f.arguments.len() - 1;
                        locations.push((loc, s, modes, index));
                        index
                    }
                };
                let arg_scalar = locations.iter().find(|l| l.0 == loc).unwrap().1;
                let a = e.add(&mut f, &mut body, Expression::FunctionArgument(arg as u32));
                let v = e.swizzle(&mut f, &mut body, a, comp, member.size)?;
                let v = e.bitcast(&mut f, &mut body, v, arg_scalar, s);
                let p = e.add(&mut f, &mut body, Expression::GlobalVariable(var));
                e.stmt(
                    &f,
                    &mut body,
                    Statement::Store {
                        pointer: p,
                        value: v,
                    },
                );
                self.reflect_value(id, true, Some(loc), comp, member, modes, None);
            } else {
                let b = builtin.unwrap();
                if matches!(b, builtin::CLIP_DISTANCE | builtin::CULL_DISTANCE) {
                    self.distance_input(&mut f, &mut body, &mut e, i, b)?;
                    continue;
                }
                let member = member.ok_or(unsupported(crate::refusal::BUILT_IN_ARRAY))?;
                let v = self.builtin_input(&mut f, &mut body, &mut e, b, member)?;
                let p = e.add(&mut f, &mut body, Expression::GlobalVariable(var));
                e.stmt(
                    &f,
                    &mut body,
                    Statement::Store {
                        pointer: p,
                        value: v,
                    },
                );
                let system = system_value(b);
                self.reflect_value(id, true, None, 0, member, modes, system);
            }
        }

        self.evaluate_prologue(&mut f, &mut body, &mut e)?;
        let main = self.functions[&self.main];
        e.stmt(
            &f,
            &mut body,
            Statement::Call {
                function: main,
                arguments: Vec::new(),
                result: None,
            },
        );

        // Outputs.
        let mut members = Vec::new();
        let mut values = Vec::new();
        let mut out_locations: Vec<u32> = Vec::new();
        for i in 0..io_count {
            let (id, ref io) = self.io[i];
            if input(self.ir.op(id)?.code) {
                continue;
            }
            if let Some((loc, _)) = io.location {
                if !out_locations.contains(&loc) {
                    out_locations.push(loc);
                }
                continue;
            }
            let b = io.builtin.unwrap();
            let distances = match b {
                builtin::CLIP_DISTANCE => Some(naga::BuiltIn::ClipDistances),
                builtin::CULL_DISTANCE => Some(naga::BuiltIn::CullDistance),
                _ => None,
            };
            if let Some(distances) = distances {
                // The common form's clip (or cull) distances are one array of f32, however the IR
                // groups them: every declaration's, in order, made one array where the first is met.
                let clips: Vec<usize> = (0..io_count)
                    .filter(|&j| {
                        let (other, ref io) = self.io[j];
                        io.builtin == Some(b) && self.ir.op(other).is_ok_and(|o| !input(o.code))
                    })
                    .collect();
                if clips[0] != i {
                    continue;
                }
                let f32_ty = self.scalar_ty(Scalar::F32);
                let mut parts = Vec::new();
                for &j in &clips {
                    let (other, ref io) = self.io[j];
                    let element = io.ty.members[..]
                        == [Basic {
                            scalar: scalar::F32,
                            size: 1,
                        }];
                    if !element || io.ty.dims.len() > 1 {
                        return Err(unsupported(CLIP_DISTANCE_TYPE));
                    }
                    let count: u32 = io.ty.dims.iter().product();
                    let (var, ty) = (io.var, io.ty.clone());
                    let p = e.add(&mut f, &mut body, Expression::GlobalVariable(var));
                    let v = e.add(&mut f, &mut body, Expression::Load { pointer: p });
                    if ty.dims.is_empty() {
                        parts.push(v);
                    } else {
                        for index in 0..count {
                            parts.push(e.add(
                                &mut f,
                                &mut body,
                                Expression::AccessIndex { base: v, index },
                            ));
                        }
                    }
                    self.reflect_value(other, false, None, 0, ty.members[0], 0, system_value(b));
                }
                let count = parts.len() as u32;
                if count > CLIP_OR_CULL_DISTANCES {
                    return Err(unsupported(CLIP_DISTANCE_TYPE));
                }
                let array = self.ty(TypeInner::Array {
                    base: f32_ty,
                    size: ArraySize::Constant(NonZeroU32::new(count).unwrap()),
                    stride: core::mem::size_of::<f32>() as u32,
                });
                let v = e.add(
                    &mut f,
                    &mut body,
                    Expression::Compose {
                        ty: array,
                        components: parts,
                    },
                );
                members.push((format!("b{b}"), array, naga::Binding::BuiltIn(distances)));
                values.push(v);
                continue;
            }
            let member = io
                .ty
                .basic()
                .ok_or(unsupported(crate::refusal::BUILT_IN_ARRAY))?;
            let var = io.var;
            let p = e.add(&mut f, &mut body, Expression::GlobalVariable(var));
            let v = e.add(&mut f, &mut body, Expression::Load { pointer: p });
            let (bi, s, size) = match b {
                builtin::POSITION => (naga::BuiltIn::Position { invariant: false }, Scalar::F32, 4),
                builtin::DEPTH => (naga::BuiltIn::FragDepth, Scalar::F32, 1),
                _ => (naga::BuiltIn::SampleMask, Scalar::U32, 1),
            };
            let v = e.bitcast(&mut f, &mut body, v, Self::scalar(member.scalar)?, s);
            let ty = self.vec_ty(size, s)?;
            members.push((format!("b{}", b), ty, naga::Binding::BuiltIn(bi)));
            values.push(v);
            self.reflect_value(id, false, None, 0, member, 0, system_value(b));
        }
        out_locations.sort_unstable();
        for loc in out_locations {
            let decls: Vec<usize> = (0..io_count)
                .filter(|&i| {
                    self.ir.op(self.io[i].0).is_ok_and(|o| !input(o.code))
                        && self.io[i].1.location.is_some_and(|l| l.0 == loc)
                })
                .collect();
            let first = self.io[decls[0]]
                .1
                .ty
                .basic()
                .ok_or(unsupported(crate::refusal::OUTPUT_ARRAY))?;
            let s = Self::scalar(first.scalar)?;
            let s = if s == Scalar::BOOL { Scalar::U32 } else { s };
            let mut comps = Vec::new();
            for c in 0..4u32 {
                let found = decls.iter().find(|&&i| {
                    let io = &self.io[i].1;
                    let (_, comp) = io.location.unwrap();
                    let size = io.ty.basic().map_or(1, |b| b.size as u32);
                    (comp..comp + size).contains(&c)
                });
                let v = match found {
                    Some(&i) => {
                        let io = &self.io[i].1;
                        let (_, comp) = io.location.unwrap();
                        let b = io
                            .ty
                            .basic()
                            .ok_or(unsupported(crate::refusal::OUTPUT_ARRAY))?;
                        let p = e.add(&mut f, &mut body, Expression::GlobalVariable(io.var));
                        let v = e.add(&mut f, &mut body, Expression::Load { pointer: p });
                        let v = if b.size > 1 {
                            e.add(
                                &mut f,
                                &mut body,
                                Expression::AccessIndex {
                                    base: v,
                                    index: c - comp,
                                },
                            )
                        } else {
                            v
                        };
                        e.bitcast(&mut f, &mut body, v, Self::scalar(b.scalar)?, s)
                    }
                    None => e.add(&mut f, &mut body, Expression::Literal(zero(s))),
                };
                comps.push(v);
            }
            let ty = self.vec_ty(4, s)?;
            let v = e.add(
                &mut f,
                &mut body,
                Expression::Compose {
                    ty,
                    components: comps,
                },
            );
            let modes = match self.stage {
                SourceStage::Vertex | SourceStage::Domain => {
                    output_modes(&self.variant.output_interpolation, loc)
                }
                _ => 0,
            };
            let flat = s.kind != ScalarKind::Float || modes & interp::FLAT != 0;
            let binding = self.location_binding(loc, modes, flat, false);
            members.push((format!("v{loc}"), ty, binding));
            values.push(v);
            for &i in &decls {
                let (id, ref io) = self.io[i];
                let (loc, comp) = io.location.unwrap();
                let b = io.ty.basic().unwrap();
                self.reflect_value(id, false, Some(loc), comp, b, modes, None);
            }
        }
        if !members.is_empty() {
            // Each member at the next offset its type's alignment allows, as WGSL lays a
            // structure out.
            let mut layouter = naga::proc::Layouter::default();
            layouter
                .update(self.module.to_ctx())
                .map_err(|e| malformed(format!("output layout: {e}")))?;
            let mut offset = 0;
            let mut largest = naga::proc::Alignment::ONE;
            let mut struct_members = Vec::new();
            for (name, ty, binding) in members {
                let layout = layouter[ty];
                offset = layout.alignment.round_up(offset);
                largest = largest.max(layout.alignment);
                struct_members.push(StructMember {
                    name: Some(name),
                    ty,
                    binding: Some(binding),
                    offset,
                });
                offset += layout.size;
            }
            let ty = self.module.types.insert(
                Type {
                    name: Some("Outputs".into()),
                    inner: TypeInner::Struct {
                        members: struct_members,
                        span: largest.round_up(offset),
                    },
                },
                Span::UNDEFINED,
            );
            let v = e.add(
                &mut f,
                &mut body,
                Expression::Compose {
                    ty,
                    components: values,
                },
            );
            e.stmt(&f, &mut body, Statement::Return { value: Some(v) });
            f.result = Some(FunctionResult { ty, binding: None });
        }
        e.flush(&f, &mut body);
        f.body = body;

        let stage = match self.stage {
            SourceStage::Vertex | SourceStage::Domain => naga::ShaderStage::Vertex,
            SourceStage::Pixel => naga::ShaderStage::Fragment,
            _ => naga::ShaderStage::Compute,
        };
        // Early tests are the reflection's to report (WGSL cannot force them), and conservative depth is
        // only a hint WGSL has no form for.
        let early_depth_test = None;
        let _ = self.conservative;
        self.reflection.extras = match self.stage {
            SourceStage::Pixel => StageExtras::Pixel {
                early_tests: self.early_tests,
                side_effects: self.side_effects,
                discards: self.discards,
                per_sample: self.io.iter().any(|(_, io)| {
                    io.builtin == Some(builtin::SAMPLE_ID) || io.interpolation & interp::SAMPLE != 0
                }),
            },
            SourceStage::Compute => StageExtras::Compute {
                workgroup_size: self.workgroup,
            },
            SourceStage::Domain => self.reflection.extras.clone(),
            _ => StageExtras::None,
        };
        self.module.entry_points.push(naga::EntryPoint {
            name: crate::ENTRY_POINT.into(),
            stage,
            early_depth_test,
            workgroup_size: if stage == naga::ShaderStage::Compute {
                self.workgroup
            } else {
                [0; 3]
            },
            workgroup_size_overrides: None,
            function: f,
            mesh_info: None,
            task_payload: None,
            incoming_ray_payload: None,
        });
        Ok(())
    }

    fn location_binding(&self, loc: u32, modes: u64, flat: bool, input: bool) -> naga::Binding {
        let between_stages = match self.stage {
            SourceStage::Vertex | SourceStage::Domain => !input,
            SourceStage::Pixel => input,
            _ => false,
        };
        let (interpolation, sampling) = if !between_stages {
            (None, None)
        } else if flat {
            naga_interpolation(interp::FLAT)
        } else {
            naga_interpolation(modes)
        };
        naga::Binding::Location {
            location: loc,
            interpolation,
            sampling,
            blend_src: None,
            per_primitive: false,
        }
    }

    fn builtin_input(
        &mut self,
        f: &mut Function,
        body: &mut Block,
        e: &mut Emit,
        b: u64,
        member: Basic,
    ) -> Result<Handle<Expression>> {
        if b == builtin::SAMPLE_COUNT {
            // The targets' sample count, which the variant fixes.
            let count = e.add(
                f,
                body,
                Expression::Literal(Literal::U32(self.variant.samples.into())),
            );
            return Ok(e.bitcast(f, body, count, Scalar::U32, Self::scalar(member.scalar)?));
        }
        if b == builtin::LAYER_INDEX && self.variant.flags.contains(VariantFlags::LAYER_FROM_STAGE)
        {
            // The layer the geometry stage's primitive names, which its pass-through passes.
            let u32_ty = self.scalar_ty(Scalar::U32);
            f.arguments.push(FunctionArgument {
                name: Some(format!("b{b}")),
                ty: u32_ty,
                binding: Some(naga::Binding::Location {
                    location: crate::stage::named_layer_location(self.variant),
                    interpolation: Some(naga::Interpolation::Flat),
                    sampling: None,
                    blend_src: None,
                    per_primitive: false,
                }),
            });
            let index = f.arguments.len() as u32 - 1;
            let v = e.add(f, body, Expression::FunctionArgument(index));
            return Ok(e.bitcast(f, body, v, Scalar::U32, Self::scalar(member.scalar)?));
        }
        if let builtin::LAYER_INDEX | builtin::VIEWPORT_INDEX = b {
            // The draw's run's layer or viewport (a pixel shader's inputs; no stage before writes
            // them where the front draws, or it splits the draw per layer or viewport).
            let value = match b {
                builtin::LAYER_INDEX => HelperValue::Layer,
                _ => HelperValue::Viewport,
            };
            let u32_ty = self.scalar_ty(Scalar::U32);
            let (var, index) = crate::variant::helper_once(
                &mut self.module,
                &mut self.reflection,
                ShaderStage::Fragment,
                value,
                u32_ty,
            );
            let p = e.add(f, body, Expression::GlobalVariable(var));
            let p = e.add(f, body, Expression::AccessIndex { base: p, index });
            let v = e.add(f, body, Expression::Load { pointer: p });
            return Ok(e.bitcast(f, body, v, Scalar::U32, Self::scalar(member.scalar)?));
        }
        let (bi, size, s) = match b {
            builtin::POSITION => (
                naga::BuiltIn::Position { invariant: false },
                VEC4_SIZE,
                Scalar::F32,
            ),
            builtin::VERTEX_ID => (naga::BuiltIn::VertexIndex, 1, Scalar::U32),
            builtin::INSTANCE_ID => (naga::BuiltIn::InstanceIndex, 1, Scalar::U32),
            builtin::IS_FRONT_FACE => (naga::BuiltIn::FrontFacing, 1, Scalar::BOOL),
            builtin::PRIMITIVE_ID => (naga::BuiltIn::PrimitiveIndex, 1, Scalar::U32),
            builtin::SAMPLE_ID => (naga::BuiltIn::SampleIndex, 1, Scalar::U32),
            builtin::SAMPLE_MASK => (naga::BuiltIn::SampleMask, 1, Scalar::U32),
            builtin::WORKGROUP_ID => (naga::BuiltIn::WorkGroupId, VEC3_SIZE, Scalar::U32),
            builtin::GLOBAL_THREAD_ID => {
                (naga::BuiltIn::GlobalInvocationId, VEC3_SIZE, Scalar::U32)
            }
            builtin::LOCAL_THREAD_ID => (naga::BuiltIn::LocalInvocationId, VEC3_SIZE, Scalar::U32),
            _ => (naga::BuiltIn::LocalInvocationIndex, 1, Scalar::U32),
        };
        let ty = self.vec_ty(size, s)?;
        f.arguments.push(FunctionArgument {
            name: Some(format!("b{b}")),
            ty,
            binding: Some(naga::Binding::BuiltIn(bi)),
        });
        let mut v = e.add(
            f,
            body,
            Expression::FunctionArgument(f.arguments.len() as u32 - 1),
        );
        if let (Some(helpers), builtin::VERTEX_ID | builtin::INSTANCE_ID) = (self.helpers, b) {
            let p = e.add(f, body, Expression::GlobalVariable(helpers));
            let index = if b == builtin::VERTEX_ID { 0 } else { 1 };
            let p = e.add(f, body, Expression::AccessIndex { base: p, index });
            let base = e.add(f, body, Expression::Load { pointer: p });
            let base = if b == builtin::VERTEX_ID {
                e.bitcast(f, body, base, Scalar::I32, Scalar::U32)
            } else {
                base
            };
            v = e.add(
                f,
                body,
                Expression::Binary {
                    op: BinaryOperator::Subtract,
                    left: v,
                    right: base,
                },
            );
        }
        let want = Self::scalar(member.scalar)?;
        if s == Scalar::BOOL && want != Scalar::BOOL {
            let one = e.add(f, body, Expression::Literal(ones(want)));
            let zero = e.add(f, body, Expression::Literal(zero(want)));
            return Ok(e.add(
                f,
                body,
                Expression::Select {
                    condition: v,
                    accept: one,
                    reject: zero,
                },
            ));
        }
        let v = if member.size < size {
            e.swizzle(f, body, v, 0, member.size)?
        } else {
            v
        };
        Ok(e.bitcast(f, body, v, s, want))
    }

    #[allow(clippy::too_many_arguments)]
    fn reflect_value(
        &mut self,
        id: u32,
        input: bool,
        location: Option<u32>,
        component: u32,
        member: Basic,
        modes: u64,
        system: Option<SystemValue>,
    ) {
        let (semantic, semantic_index) = self.semantics.get(&id).cloned().unwrap_or_default();
        let stream = self
            .io_index
            .get(&id)
            .map_or(0, |&i| self.io[i].1.stream as u8);
        let place = Placed {
            location,
            component,
            member,
            modes,
            system,
            stream,
        };
        let value = stage_value(
            semantic,
            semantic_index,
            &place,
            self.stage == SourceStage::Pixel && !input,
        );
        if input {
            self.reflection.inputs.push(value);
        } else {
            self.reflection.outputs.push(value);
        }
    }
}

fn system_value(b: u64) -> Option<SystemValue> {
    Some(match b {
        builtin::POSITION => SystemValue::Position,
        builtin::VERTEX_ID => SystemValue::VertexId,
        builtin::INSTANCE_ID => SystemValue::InstanceId,
        builtin::IS_FRONT_FACE => SystemValue::FrontFacing,
        builtin::PRIMITIVE_ID => SystemValue::PrimitiveId,
        builtin::SAMPLE_ID => SystemValue::SampleIndex,
        builtin::SAMPLE_MASK => SystemValue::Coverage,
        builtin::DEPTH => SystemValue::Depth,
        builtin::CLIP_DISTANCE => SystemValue::ClipDistance,
        builtin::CULL_DISTANCE => SystemValue::CullDistance,
        builtin::LAYER_INDEX => SystemValue::RenderTargetArrayIndex,
        builtin::VIEWPORT_INDEX => SystemValue::ViewportArrayIndex,
        _ => return None,
    })
}

fn vector_size(n: u8) -> Result<VectorSize> {
    Ok(match n {
        2 => VectorSize::Bi,
        VEC3_SIZE => VectorSize::Tri,
        VEC4_SIZE => VectorSize::Quad,
        _ => return Err(malformed(format!("vector of {n}"))),
    })
}

/// Where a stage value is placed and how it is read, as the reflection records it.
pub(crate) struct Placed {
    pub location: Option<u32>,
    pub component: u32,
    pub member: Basic,
    /// Interpolation modes (`ir::interp`).
    pub modes: u64,
    pub system: Option<SystemValue>,
    pub stream: u8,
}

/// A stage value's reflection; a pixel program's output without a system value is a target.
pub(crate) fn stage_value(
    semantic: String,
    semantic_index: u32,
    p: &Placed,
    pixel_output: bool,
) -> StageValue {
    let s = Lowering::scalar(p.member.scalar).unwrap_or(Scalar::U32);
    let scalar = match s.kind {
        ScalarKind::Float => ValueKind::Float,
        ScalarKind::Sint => ValueKind::Sint,
        _ => ValueKind::Uint,
    };
    let modes = p.modes;
    let interpolation = if modes & interp::FLAT != 0 || scalar != ValueKind::Float {
        Interpolation::Flat
    } else {
        let linear = modes & interp::NO_PERSPECTIVE != 0;
        match (
            linear,
            modes & interp::CENTROID != 0,
            modes & interp::SAMPLE != 0,
        ) {
            (false, false, false) => Interpolation::Perspective,
            (true, false, false) => Interpolation::Linear,
            (false, true, _) => Interpolation::PerspectiveCentroid,
            (true, true, _) => Interpolation::LinearCentroid,
            (false, false, true) => Interpolation::PerspectiveSample,
            (true, false, true) => Interpolation::LinearSample,
        }
    };
    let system = p
        .system
        .or_else(|| pixel_output.then(|| SystemValue::Target(p.location.unwrap_or(0) as u8)));
    StageValue {
        semantic,
        semantic_index,
        register: p.location.unwrap_or(0),
        mask: (((1u32 << p.member.size) - 1) << p.component) as u8,
        scalar,
        interpolation,
        system,
        location: p.location.unwrap_or(0),
        component: p.component as u8,
        stream: p.stream,
    }
}

pub(crate) fn scalar_code(s: Scalar) -> u8 {
    match s.kind {
        ScalarKind::Bool => scalar::BOOL,
        ScalarKind::Sint => scalar::I32,
        ScalarKind::Float => scalar::F32,
        _ => scalar::U32,
    }
}

fn basic_size(b: Basic) -> u32 {
    crate::build::WORD * u32::from(b.size)
}

/// A vector's alignment in WGSL's layout: its size, a three-component one's rounded up to four.
fn basic_align(b: Basic) -> u32 {
    crate::build::WORD * u32::from(b.size).next_power_of_two()
}

fn zero(s: Scalar) -> Literal {
    match s.kind {
        ScalarKind::Float => Literal::F32(0.0),
        ScalarKind::Sint => Literal::I32(0),
        ScalarKind::Bool => Literal::Bool(false),
        _ => Literal::U32(0),
    }
}

fn ones(s: Scalar) -> Literal {
    match s.kind {
        ScalarKind::Sint => Literal::I32(-1),
        ScalarKind::Bool => Literal::Bool(true),
        _ => Literal::U32(u32::MAX),
    }
}

/// The storage texture format of an interface format, where WGSL has one.
pub(crate) fn storage_format(f: wcontainers_gpu::Format) -> Option<naga::StorageFormat> {
    use naga::StorageFormat as S;
    use wcontainers_gpu::Format as F;
    Some(match f {
        F::R8Unorm => S::R8Unorm,
        F::R8Snorm => S::R8Snorm,
        F::R8Uint => S::R8Uint,
        F::R8Sint => S::R8Sint,
        F::R16Unorm => S::R16Unorm,
        F::R16Snorm => S::R16Snorm,
        F::R16Uint => S::R16Uint,
        F::R16Sint => S::R16Sint,
        F::R16Float => S::R16Float,
        F::Rg8Unorm => S::Rg8Unorm,
        F::Rg8Snorm => S::Rg8Snorm,
        F::Rg8Uint => S::Rg8Uint,
        F::Rg8Sint => S::Rg8Sint,
        F::R32Uint => S::R32Uint,
        F::R32Sint => S::R32Sint,
        F::R32Float => S::R32Float,
        F::Rg16Unorm => S::Rg16Unorm,
        F::Rg16Snorm => S::Rg16Snorm,
        F::Rg16Uint => S::Rg16Uint,
        F::Rg16Sint => S::Rg16Sint,
        F::Rg16Float => S::Rg16Float,
        F::Rgba8Unorm => S::Rgba8Unorm,
        F::Rgba8Snorm => S::Rgba8Snorm,
        F::Rgba8Uint => S::Rgba8Uint,
        F::Rgba8Sint => S::Rgba8Sint,
        F::Bgra8Unorm => S::Bgra8Unorm,
        F::Rgb10a2Unorm => S::Rgb10a2Unorm,
        F::Rgb10a2Uint => S::Rgb10a2Uint,
        F::Rg11b10Float => S::Rg11b10Ufloat,
        F::Rg32Uint => S::Rg32Uint,
        F::Rg32Sint => S::Rg32Sint,
        F::Rg32Float => S::Rg32Float,
        F::Rgba16Unorm => S::Rgba16Unorm,
        F::Rgba16Snorm => S::Rgba16Snorm,
        F::Rgba16Uint => S::Rgba16Uint,
        F::Rgba16Sint => S::Rgba16Sint,
        F::Rgba16Float => S::Rgba16Float,
        F::Rgba32Uint => S::Rgba32Uint,
        F::Rgba32Sint => S::Rgba32Sint,
        F::Rgba32Float => S::Rgba32Float,
        _ => return None,
    })
}

/// The storage texture format a program declares for a storage view of an interface format: WGSL's
/// own, or `rgba8unorm` for `A8Unorm`, which the back ends hold so with the value in alpha (the
/// variant's `alpha_only` keeps the colour channels zero).
pub(crate) fn storage_form(f: wcontainers_gpu::Format) -> Option<naga::StorageFormat> {
    storage_format(f)
        .or((f == wcontainers_gpu::Format::A8Unorm).then_some(naga::StorageFormat::Rgba8Unorm))
}

/// The scalar kind a storage format's texels are read and written as.
pub(crate) fn storage_kind(f: naga::StorageFormat) -> ScalarKind {
    use naga::StorageFormat as S;
    match f {
        S::R8Uint
        | S::R16Uint
        | S::Rg8Uint
        | S::R32Uint
        | S::Rg16Uint
        | S::Rgba8Uint
        | S::Rgb10a2Uint
        | S::Rg32Uint
        | S::Rgba16Uint
        | S::Rgba32Uint
        | S::R64Uint => ScalarKind::Uint,
        S::R8Sint
        | S::R16Sint
        | S::Rg8Sint
        | S::R32Sint
        | S::Rg16Sint
        | S::Rgba8Sint
        | S::Rg32Sint
        | S::Rgba16Sint
        | S::Rgba32Sint => ScalarKind::Sint,
        _ => ScalarKind::Float,
    }
}

fn image_dim(k: u64) -> Result<(ImageDimension, bool, bool)> {
    Ok(match k {
        kind::IMAGE_1D | kind::IMAGE_2D => (ImageDimension::D2, false, false),
        kind::IMAGE_1D_ARRAY | kind::IMAGE_2D_ARRAY => (ImageDimension::D2, true, false),
        kind::IMAGE_2DMS => (ImageDimension::D2, false, true),
        kind::IMAGE_2DMS_ARRAY => (ImageDimension::D2, true, true),
        kind::IMAGE_CUBE => (ImageDimension::Cube, false, false),
        kind::IMAGE_CUBE_ARRAY => (ImageDimension::Cube, true, false),
        kind::IMAGE_3D => (ImageDimension::D3, false, false),
        _ => return Err(malformed(format!("resource kind {k}"))),
    })
}

/// Places expressions into `Emit` statements as naga requires: expressions that need no emitting
/// (literals, variables, arguments, call results) close the running range.
struct Emit {
    start: usize,
}

impl Emit {
    fn new(f: &Function) -> Emit {
        Emit {
            start: f.expressions.len(),
        }
    }

    fn flush(&mut self, f: &Function, block: &mut Block) {
        let len = f.expressions.len();
        if len > self.start {
            block.push(
                Statement::Emit(f.expressions.range_from(self.start)),
                Span::UNDEFINED,
            );
        }
        self.start = len;
    }

    fn add(&mut self, f: &mut Function, block: &mut Block, expr: Expression) -> Handle<Expression> {
        if expr.needs_pre_emit()
            || matches!(
                expr,
                Expression::CallResult(_) | Expression::AtomicResult { .. }
            )
        {
            self.flush(f, block);
            let h = f.expressions.append(expr, Span::UNDEFINED);
            self.start = f.expressions.len();
            h
        } else {
            f.expressions.append(expr, Span::UNDEFINED)
        }
    }

    fn stmt(&mut self, f: &Function, block: &mut Block, s: Statement) {
        self.flush(f, block);
        block.push(s, Span::UNDEFINED);
    }

    /// Components `first..first + n` of a vector.
    fn swizzle(
        &mut self,
        f: &mut Function,
        block: &mut Block,
        v: Handle<Expression>,
        first: u32,
        n: u8,
    ) -> Result<Handle<Expression>> {
        if n == 1 {
            return Ok(self.add(
                f,
                block,
                Expression::AccessIndex {
                    base: v,
                    index: first,
                },
            ));
        }
        let mut pattern = [SwizzleComponent::X; 4];
        for (i, p) in pattern.iter_mut().enumerate().take(n as usize) {
            *p = SwizzleComponent::XYZW[(first as usize + i).min(3)];
        }
        Ok(self.add(
            f,
            block,
            Expression::Swizzle {
                size: vector_size(n)?,
                vector: v,
                pattern,
            },
        ))
    }

    /// Reinterprets the bits of `v` (of scalar `from`) as scalar `to`.
    fn bitcast(
        &mut self,
        f: &mut Function,
        block: &mut Block,
        v: Handle<Expression>,
        from: Scalar,
        to: Scalar,
    ) -> Handle<Expression> {
        if from.kind == to.kind {
            return v;
        }
        if to.kind == ScalarKind::Bool {
            let z = self.add(f, block, Expression::Literal(zero(from)));
            return self.add(
                f,
                block,
                Expression::Binary {
                    op: BinaryOperator::NotEqual,
                    left: v,
                    right: z,
                },
            );
        }
        if from.kind == ScalarKind::Bool {
            let one = self.add(f, block, Expression::Literal(ones(to)));
            let z = self.add(f, block, Expression::Literal(zero(to)));
            return self.add(
                f,
                block,
                Expression::Select {
                    condition: v,
                    accept: one,
                    reject: z,
                },
            );
        }
        self.add(
            f,
            block,
            Expression::As {
                expr: v,
                kind: to.kind,
                convert: None,
            },
        )
    }
}

/// An index on the way into a variable.
#[derive(Clone, Copy)]
enum Index {
    Const(u32),
    Dyn(Handle<Expression>),
}

/// Where an access leads.
enum Place {
    /// A pointer to an element, and the component to take if one is named.
    Element(Handle<Expression>, Basic, Option<Index>),
    /// A pointer to an array of scalars, and the index of the first of several consecutive ones.
    Run(Handle<Expression>, Index, Basic),
}

/// Lowers one IR function.
struct FunctionLowering<'l, 'a> {
    l: &'l mut Lowering<'a>,
    f: Function,
    e: Emit,
    vals: HashMap<u32, Val>,
    consts: HashMap<u32, Handle<Expression>>,
    spills: HashMap<u32, Handle<LocalVariable>>,
    phis: HashMap<u32, Handle<LocalVariable>>,
    blocks: HashMap<u32, Block2>,
    block_of: HashMap<u32, u32>,
    params: Vec<u32>,
    cur: u32,
    /// The switch exit flags (`Targets::switch_exit`) some case has set.
    switch_exits: Vec<Handle<LocalVariable>>,
    /// The element each descriptor picking one at run time names in the switch case being lowered.
    picked: HashMap<u32, u32>,
    /// The regions enclosing the one being lowered.
    nesting: usize,
}

/// Where branches go inside a construct.
#[derive(Clone, Copy, Default)]
struct Targets {
    brk: Option<u32>,
    cont: Option<u32>,
    header: Option<u32>,
    /// Inside a switch within a loop: set before a case leaves the switch for the loop's break, and
    /// tested after the switch, since a break in a case leaves only the switch.
    switch_exit: Option<Handle<LocalVariable>>,
}

impl<'l, 'a> FunctionLowering<'l, 'a> {
    fn lower(l: &'l mut Lowering<'a>, id: u32) -> Result<Handle<Function>> {
        let ir = l.ir;
        let fop = ir.op(id)?;
        let mut f = Function {
            name: Some(if id == l.main {
                "shader".into()
            } else {
                format!("f{id}")
            }),
            ..Default::default()
        };
        for (i, &p) in fop.refs.iter().enumerate() {
            let ty = l.ir_ty(&ir.op(p)?.ty)?;
            f.arguments.push(FunctionArgument {
                name: Some(format!("p{i}")),
                ty,
                binding: None,
            });
        }
        if !fop.ty.is_void() {
            let ty = l.ir_ty(&fop.ty)?;
            f.result = Some(FunctionResult { ty, binding: None });
        }
        let e = Emit::new(&f);
        let mut fl = FunctionLowering {
            l,
            f,
            e,
            vals: HashMap::default(),
            consts: HashMap::default(),
            spills: HashMap::default(),
            phis: HashMap::default(),
            blocks: HashMap::default(),
            block_of: HashMap::default(),
            params: fop.refs.clone(),
            cur: 0,
            switch_exits: Vec::new(),
            picked: HashMap::default(),
            nesting: 0,
        };
        let entry = fl.scan(id)?;
        let mut body = Block::new();
        fl.region(entry, None, Targets::default(), &mut body)?;
        fl.e.flush(&fl.f, &mut body);
        fl.f.body = body;
        let f = fl.f;
        Ok(fl.l.module.functions.append(f, Span::UNDEFINED))
    }

    /// Finds the blocks and which values need variables; returns the entry block's label.
    fn scan(&mut self, fid: u32) -> Result<u32> {
        let ir = self.l.ir;
        let mut id = fid + 1;
        let mut entry = 0;
        let mut cur: Option<Block2> = None;
        loop {
            let op = ir.op(id)?;
            match op.code {
                code::FUNCTION_END => break,
                code::LABEL => {
                    if entry == 0 {
                        entry = id;
                    }
                    cur = Some(Block2 {
                        label: id,
                        first: id + 1,
                        last: id,
                        phis: Vec::new(),
                    });
                }
                _ => {
                    let b = cur
                        .as_mut()
                        .ok_or_else(|| malformed("code outside a block".into()))?;
                    self.block_of.insert(id, b.label);
                    if op.code == code::PHI {
                        b.phis.push(id);
                    }
                    if matches!(
                        op.code,
                        code::BRANCH
                            | code::BRANCH_CONDITIONAL
                            | code::SWITCH
                            | code::RETURN
                            | code::UNREACHABLE
                    ) {
                        b.last = id;
                        let b = cur.take().unwrap();
                        self.blocks.insert(b.label, b);
                    }
                }
            }
            id += 1;
        }
        // Values used outside their block, or by a phi from another block, get a variable.
        let mut spill = Vec::new();
        for (&id, &label) in &self.block_of {
            let op = ir.op(id)?;
            if op.code == code::PHI {
                for pair in op.refs.chunks(2) {
                    if let [from, v] = *pair
                        && self.block_of.get(&v).is_some_and(|&b| b != from)
                    {
                        spill.push(v);
                    }
                }
                continue;
            }
            for &r in &op.refs {
                if self.block_of.get(&r).is_some_and(|&b| b != label) {
                    spill.push(r);
                }
            }
        }
        // Variables are made in id order, so the module is the same on every run and every host.
        spill.sort_unstable();
        spill.dedup();
        for v in spill {
            let op = ir.op(v)?;
            if self.spills.contains_key(&v)
                || op.code == code::DESCRIPTOR_LOAD
                || op.ty.members.len() != 1
            {
                continue;
            }
            let ty = self.l.ir_ty(&ir.op(v)?.ty)?;
            let local = self.f.local_variables.append(
                LocalVariable {
                    name: Some(format!("x{v}")),
                    ty,
                    init: None,
                },
                Span::UNDEFINED,
            );
            self.spills.insert(v, local);
        }
        let mut phis: Vec<u32> = self.blocks.values().flat_map(|b| b.phis.clone()).collect();
        phis.sort_unstable();
        for p in phis {
            let ty = self.l.ir_ty(&ir.op(p)?.ty)?;
            let local = self.f.local_variables.append(
                LocalVariable {
                    name: Some(format!("phi{p}")),
                    ty,
                    init: None,
                },
                Span::UNDEFINED,
            );
            self.phis.insert(p, local);
        }
        Ok(entry)
    }

    // Expression helpers.

    fn add(&mut self, b: &mut Block, expr: Expression) -> Handle<Expression> {
        self.e.add(&mut self.f, b, expr)
    }

    fn stmt(&mut self, b: &mut Block, s: Statement) {
        self.e.stmt(&self.f, b, s);
    }

    fn lit(&mut self, b: &mut Block, l: Literal) -> Handle<Expression> {
        self.add(b, Expression::Literal(l))
    }

    fn bin(
        &mut self,
        b: &mut Block,
        op: BinaryOperator,
        left: Handle<Expression>,
        right: Handle<Expression>,
    ) -> Handle<Expression> {
        self.add(b, Expression::Binary { op, left, right })
    }

    fn math(
        &mut self,
        b: &mut Block,
        fun: MathFunction,
        args: &[Handle<Expression>],
    ) -> Handle<Expression> {
        self.add(
            b,
            Expression::Math {
                fun,
                arg: args[0],
                arg1: args.get(1).copied(),
                arg2: args.get(2).copied(),
                arg3: args.get(3).copied(),
            },
        )
    }

    fn basic_of(&self, id: u32) -> Result<Basic> {
        self.l
            .ir
            .op(id)?
            .ty
            .basic()
            .ok_or_else(|| malformed(format!("value {id} is not a scalar or vector")))
    }

    fn scalar_of(&self, id: u32) -> Result<Scalar> {
        Lowering::scalar(self.basic_of(id)?.scalar)
    }

    /// The expression for value `id` here: a struct value (kept as its members) composed whole, as
    /// a phi or a call takes it.
    fn get(&mut self, b: &mut Block, id: u32) -> Result<Handle<Expression>> {
        match self.val(b, id)? {
            Val::Expr(h) => Ok(h),
            Val::Tuple(components) => {
                let ty = self.l.ir_ty(&self.l.ir.op(id)?.ty)?;
                Ok(self.add(b, Expression::Compose { ty, components }))
            }
        }
    }

    /// Value `id` converted to scalar kind `to` by its bits.
    fn get_as(&mut self, b: &mut Block, id: u32, to: Scalar) -> Result<Handle<Expression>> {
        let v = self.get(b, id)?;
        let from = self.scalar_of(id)?;
        Ok(self.e.bitcast(&mut self.f, b, v, from, to))
    }

    fn val(&mut self, b: &mut Block, id: u32) -> Result<Val> {
        let ir = self.l.ir;
        let op = ir.op(id)?;
        match op.code {
            code::CONSTANT => return Ok(Val::Expr(self.constant(b, id)?)),
            code::UNDEF => {
                if let Some(&h) = self.consts.get(&id) {
                    return Ok(Val::Expr(h));
                }
                let ty = self.l.ir_ty(&op.ty)?;
                let h = self.add(b, Expression::ZeroValue(ty));
                self.consts.insert(id, h);
                return Ok(Val::Expr(h));
            }
            _ => {}
        }
        if let Some(&local) = self.spills.get(&id)
            && self.block_of.get(&id) != Some(&self.cur)
        {
            let p = self.add(b, Expression::LocalVariable(local));
            return Ok(Val::Expr(self.add(b, Expression::Load { pointer: p })));
        }
        self.vals
            .get(&id)
            .cloned()
            .ok_or_else(|| malformed(format!("value {id} used before it is made")))
    }

    fn constant(&mut self, b: &mut Block, id: u32) -> Result<Handle<Expression>> {
        if let Some(&h) = self.consts.get(&id) {
            return Ok(h);
        }
        let op = self.l.ir.op(id)?;
        let basic = op.ty.basic();
        let h = match basic {
            Some(m) if m.size == 1 => {
                let l = literal(Lowering::scalar(m.scalar)?, op.lits[0]);
                if let Literal::F32(x) = l
                    && !x.is_finite()
                {
                    // naga takes no infinite or NaN literals, and WGSL compilers refuse them as
                    // constant expressions: the bits, read from a variable so the value is made at
                    // run time, reinterpreted where used.
                    let bits = self.lit(b, Literal::U32(op.lits[0] as u32));
                    let ty = self.l.scalar_ty(Scalar::U32);
                    let var = self.f.local_variables.append(
                        LocalVariable {
                            name: Some(format!("bits{id}")),
                            ty,
                            init: Some(bits),
                        },
                        Span::UNDEFINED,
                    );
                    let pointer = self.add(b, Expression::LocalVariable(var));
                    let bits = self.add(b, Expression::Load { pointer });
                    return Ok(self.add(
                        b,
                        Expression::As {
                            expr: bits,
                            kind: ScalarKind::Float,
                            convert: None,
                        },
                    ));
                }
                self.add(b, Expression::Literal(l))
            }
            Some(m) if non_finite(&op.ty, &op.lits) => {
                // As for scalars: the bits, from a variable, reinterpreted.
                let ty = Ty {
                    dims: Vec::new(),
                    members: vec![Basic {
                        scalar: scalar::U32,
                        size: m.size,
                    }],
                };
                let naga_ty = self.l.ir_ty(&ty)?;
                let init = self.l.const_expr(&ty, &op.lits)?;
                let c = self.l.module.constants.append(
                    naga::Constant {
                        name: None,
                        ty: naga_ty,
                        init,
                    },
                    Span::UNDEFINED,
                );
                let bits = self
                    .f
                    .expressions
                    .append(Expression::Constant(c), Span::UNDEFINED);
                let var = self.f.local_variables.append(
                    LocalVariable {
                        name: Some(format!("bits{id}")),
                        ty: naga_ty,
                        init: Some(bits),
                    },
                    Span::UNDEFINED,
                );
                let pointer = self.add(b, Expression::LocalVariable(var));
                let bits = self.add(b, Expression::Load { pointer });
                return Ok(self.add(
                    b,
                    Expression::As {
                        expr: bits,
                        kind: ScalarKind::Float,
                        convert: None,
                    },
                ));
            }
            _ => {
                let c = self.l.global_constant(id)?;
                self.add(b, Expression::Constant(c))
            }
        };
        self.consts.insert(id, h);
        Ok(h)
    }

    fn define(&mut self, b: &mut Block, id: u32, v: Val) {
        if let (Some(&local), Val::Expr(h)) = (self.spills.get(&id), &v) {
            let p = self.add(b, Expression::LocalVariable(local));
            self.stmt(
                b,
                Statement::Store {
                    pointer: p,
                    value: *h,
                },
            );
        }
        self.vals.insert(id, v);
    }

    // Control flow.

    /// Emits blocks from `label` on until reaching `stop`. Lowering recurses once per nested region,
    /// and every pass over the module after it once per nested block, so nesting deeper than the
    /// SPIR-V reader takes is refused.
    fn region(&mut self, label: u32, stop: Option<u32>, t: Targets, out: &mut Block) -> Result<()> {
        if self.nesting == naga::front::spv::NESTING_LIMIT {
            return Err(unsupported(crate::refusal::NESTING_BEYOND_LIMIT));
        }
        self.nesting += 1;
        let lowered = self.region_within(label, stop, t, out);
        self.nesting -= 1;
        lowered
    }

    fn region_within(
        &mut self,
        mut label: u32,
        stop: Option<u32>,
        t: Targets,
        out: &mut Block,
    ) -> Result<()> {
        loop {
            if Some(label) == stop {
                return Ok(());
            }
            let lop = self.l.ir.op(label)?;
            if lop.lits.last() == Some(&construct::STRUCTURED_LOOP) && t.header != Some(label) {
                let merge = lop.refs[0];
                self.lower_loop(label, t, out)?;
                label = merge;
                continue;
            }
            match self.block(label, t, stop, out)? {
                Some(next) => label = next,
                None => return Ok(()),
            }
        }
    }

    fn lower_loop(&mut self, header: u32, outer: Targets, out: &mut Block) -> Result<()> {
        let lop = self.l.ir.op(header)?;
        let (merge, cont) = (lop.refs[0], lop.refs[1]);
        let t = Targets {
            brk: Some(merge),
            cont: Some(cont),
            header: Some(header),
            switch_exit: None,
        };
        let _ = outer;
        self.e.flush(&self.f, out);
        let mut body = Block::new();
        let mut continuing = Block::new();
        let break_if;
        if cont == header {
            break_if = self.latch(header, header, merge, &mut body)?;
        } else {
            self.region(header, Some(cont), t, &mut body)?;
            self.e.flush(&self.f, &mut body);
            // The continue construct runs from `cont` to the back edge.
            let mut label = cont;
            loop {
                let b = &self.blocks[&label];
                let term = self.l.ir.op(b.last)?;
                let back = match term.code {
                    code::BRANCH => term.refs[0] == header,
                    code::BRANCH_CONDITIONAL => term.refs[1] == header || term.refs[2] == header,
                    _ => false,
                };
                if back {
                    break_if = self.latch(label, header, merge, &mut continuing)?;
                    break;
                }
                let inner = Targets {
                    header: Some(header),
                    ..Targets::default()
                };
                match self.block(label, inner, None, &mut continuing)? {
                    Some(next) => label = next,
                    None => return Err(unsupported(crate::refusal::CONTINUE_CONSTRUCT)),
                }
            }
        }
        self.e.flush(&self.f, &mut continuing);
        self.stmt(
            out,
            Statement::Loop {
                body,
                continuing,
                break_if,
            },
        );
        Ok(())
    }

    /// Emits the block holding a loop's back edge; returns the condition that leaves the loop.
    fn latch(
        &mut self,
        label: u32,
        header: u32,
        merge: u32,
        out: &mut Block,
    ) -> Result<Option<Handle<Expression>>> {
        self.contents(label, out)?;
        let term = self.l.ir.op(self.blocks[&label].last)?;
        match term.code {
            code::BRANCH => {
                self.store_phis(label, term.refs[0], out)?;
                Ok(None)
            }
            code::BRANCH_CONDITIONAL => {
                let (c, tr, fa) = (term.refs[0], term.refs[1], term.refs[2]);
                self.store_phis(label, tr, out)?;
                self.store_phis(label, fa, out)?;
                let cond = self.get(out, c)?;
                if fa == merge && tr == header {
                    Ok(Some(self.add(
                        out,
                        Expression::Unary {
                            op: UnaryOperator::LogicalNot,
                            expr: cond,
                        },
                    )))
                } else if tr == merge && fa == header {
                    Ok(Some(cond))
                } else {
                    Err(unsupported(crate::refusal::LOOP_LATCH))
                }
            }
            _ => Err(unsupported(crate::refusal::LOOP_LATCH)),
        }
    }

    /// Emits a block's instructions (phis loaded first).
    fn contents(&mut self, label: u32, out: &mut Block) -> Result<()> {
        self.cur = label;
        let (first, last, phis) = {
            let b = &self.blocks[&label];
            (b.first, b.last, b.phis.clone())
        };
        for p in phis {
            let local = self.phis[&p];
            let ptr = self.add(out, Expression::LocalVariable(local));
            let v = self.add(out, Expression::Load { pointer: ptr });
            self.define(out, p, Val::Expr(v));
        }
        for id in first..last {
            let op = self.l.ir.op(id)?;
            if op.code == code::PHI {
                continue;
            }
            self.instruction(id, out)?;
        }
        Ok(())
    }

    /// Emits one block and its branch; returns the block to continue with in the same region.
    fn block(
        &mut self,
        label: u32,
        t: Targets,
        stop: Option<u32>,
        out: &mut Block,
    ) -> Result<Option<u32>> {
        self.contents(label, out)?;
        let lop = self.l.ir.op(label)?;
        let selection =
            (lop.lits.last() == Some(&construct::STRUCTURED_SELECTION)).then(|| lop.refs[0]);
        let term = self.l.ir.op(self.blocks[&label].last)?;
        match term.code {
            code::BRANCH => {
                let target = term.refs[0];
                self.store_phis(label, target, out)?;
                if self.jump(target, t, stop, out)? {
                    return Ok(None);
                }
                Ok(Some(target))
            }
            code::BRANCH_CONDITIONAL => {
                let (c, tr, fa) = (term.refs[0], term.refs[1], term.refs[2]);
                let condition = self.get(out, c)?;
                self.e.flush(&self.f, out);
                let merge = selection;
                let mut accept = Block::new();
                let mut reject = Block::new();
                for (target, arm) in [(tr, &mut accept), (fa, &mut reject)] {
                    self.cur = label;
                    self.store_phis(label, target, arm)?;
                    let end = merge.or(stop);
                    if Some(target) != end && !self.jump(target, t, end, arm)? {
                        self.region(target, end, t, arm)?;
                    }
                    self.e.flush(&self.f, arm);
                }
                self.stmt(
                    out,
                    Statement::If {
                        condition,
                        accept,
                        reject,
                    },
                );
                Ok(merge.filter(|&m| Some(m) != stop))
            }
            code::SWITCH => {
                let merge = selection.ok_or(unsupported(crate::refusal::SWITCH_WITHOUT_MERGE))?;
                self.switch(label, term, merge, t, out)?;
                Ok((Some(merge) != stop).then_some(merge))
            }
            code::RETURN => {
                let value = match term.refs.first() {
                    Some(&v) if v != 0 => Some(self.get(out, v)?),
                    _ => None,
                };
                self.stmt(out, Statement::Return { value });
                Ok(None)
            }
            _ => {
                // Unreachable: nothing runs past it.
                Ok(None)
            }
        }
    }

    /// Emits a jump to `target` that leaves the current region, if it is one; returns whether it was.
    fn jump(
        &mut self,
        target: u32,
        t: Targets,
        stop: Option<u32>,
        out: &mut Block,
    ) -> Result<bool> {
        if Some(target) == stop {
            return Ok(true);
        }
        if Some(target) == t.brk {
            if let Some(flag) = t.switch_exit {
                let pointer = self.add(out, Expression::LocalVariable(flag));
                let value = self.lit(out, Literal::Bool(true));
                self.stmt(out, Statement::Store { pointer, value });
                self.switch_exits.push(flag);
            }
            self.stmt(out, Statement::Break);
            return Ok(true);
        }
        if Some(target) == t.cont || Some(target) == t.header {
            self.stmt(out, Statement::Continue);
            return Ok(true);
        }
        Ok(false)
    }

    fn switch(
        &mut self,
        label: u32,
        term: &Op,
        merge: u32,
        t: Targets,
        out: &mut Block,
    ) -> Result<()> {
        let selector_id = term.refs[0];
        let s = self.scalar_of(selector_id)?;
        let selector = self.get(out, selector_id)?;
        self.e.flush(&self.f, out);
        let default = term.refs[1];
        let mut targets: Vec<(Vec<SwitchValue>, u32)> = Vec::new();
        for pair in term.refs[2..].chunks(2) {
            let [value, target] = *pair else {
                return Err(malformed("switch case".into()));
            };
            let raw = self.l.ir.op(value)?.lits[0];
            let v = if s.kind == ScalarKind::Sint {
                SwitchValue::I32(raw as i32)
            } else {
                SwitchValue::U32(raw as u32)
            };
            match targets.iter_mut().find(|x| x.1 == target) {
                Some(x) => x.0.push(v),
                None => targets.push((vec![v], target)),
            }
        }
        match targets.iter_mut().find(|x| x.1 == default) {
            Some(x) => x.0.push(SwitchValue::Default),
            None => targets.push((vec![SwitchValue::Default], default)),
        }
        // The case with the default goes last, so naga's rule that the last case does not fall through
        // holds whatever order the cases came in.
        let d = targets
            .iter()
            .position(|x| x.0.contains(&SwitchValue::Default))
            .unwrap();
        let last = targets.remove(d);
        targets.push(last);
        let exit = t.brk.map(|_| {
            let ty = self.l.scalar_ty(Scalar::BOOL);
            self.f.local_variables.append(
                LocalVariable {
                    name: Some(format!("exit{merge}")),
                    ty,
                    init: None,
                },
                Span::UNDEFINED,
            )
        });
        let inner = Targets {
            brk: t.brk,
            cont: t.cont,
            header: t.header,
            switch_exit: exit,
        };
        let mut cases = Vec::new();
        for (values, target) in targets {
            let mut body = Block::new();
            self.cur = label;
            self.store_phis(label, target, &mut body)?;
            if target != merge {
                let falls = self.falls_into_case(target, merge, term)?;
                if falls {
                    return Err(unsupported(crate::refusal::SWITCH_FALLTHROUGH));
                }
                if !self.jump(target, inner, Some(merge), &mut body)? {
                    self.region(target, Some(merge), inner, &mut body)?;
                }
            }
            self.e.flush(&self.f, &mut body);
            let n = values.len();
            for (i, value) in values.into_iter().enumerate() {
                let last = i + 1 == n;
                cases.push(SwitchCase {
                    value,
                    body: if last {
                        core::mem::take(&mut body)
                    } else {
                        Block::new()
                    },
                    fall_through: !last,
                });
            }
        }
        let exit = exit.filter(|flag| self.switch_exits.contains(flag));
        if let Some(flag) = exit {
            let pointer = self.add(out, Expression::LocalVariable(flag));
            let value = self.lit(out, Literal::Bool(false));
            self.stmt(out, Statement::Store { pointer, value });
        }
        self.stmt(out, Statement::Switch { selector, cases });
        if let (Some(flag), Some(brk)) = (exit, t.brk) {
            let pointer = self.add(out, Expression::LocalVariable(flag));
            let condition = self.add(out, Expression::Load { pointer });
            self.e.flush(&self.f, out);
            let mut accept = Block::new();
            self.jump(brk, t, None, &mut accept)?;
            self.e.flush(&self.f, &mut accept);
            self.stmt(
                out,
                Statement::If {
                    condition,
                    accept,
                    reject: Block::new(),
                },
            );
        }
        Ok(())
    }

    fn falls_into_case(&self, target: u32, merge: u32, term: &Op) -> Result<bool> {
        // A case whose block branches straight into another case's block.
        let b = &self.blocks[&target];
        let end = self.l.ir.op(b.last)?;
        Ok(end.code == code::BRANCH
            && end.refs[0] != merge
            && term.refs[1..].contains(&end.refs[0])
            && self.l.ir.op(end.refs[0])?.code == code::LABEL)
    }

    /// Stores the values the phis of `to` take when coming from `from`.
    fn store_phis(&mut self, from: u32, to: u32, out: &mut Block) -> Result<()> {
        let Some(b) = self.blocks.get(&to) else {
            return Ok(());
        };
        let phis = b.phis.clone();
        let saved = self.cur;
        self.cur = from;
        let mut stores = Vec::new();
        for p in phis {
            let op = self.l.ir.op(p)?;
            let v = op
                .refs
                .chunks(2)
                .find(|pair| pair[0] == from)
                .map(|pair| pair[1])
                .ok_or_else(|| malformed(format!("phi {p} has no value from {from}")))?;
            let h = self.get(out, v)?;
            stores.push((self.phis[&p], h));
        }
        for (local, value) in stores {
            let pointer = self.add(out, Expression::LocalVariable(local));
            self.stmt(out, Statement::Store { pointer, value });
        }
        self.cur = saved;
        Ok(())
    }

    // Instructions.

    fn instruction(&mut self, id: u32, b: &mut Block) -> Result<()> {
        use code as c;
        let ir = self.l.ir;
        let op = ir.op(id)?;
        if let Some(desc) = self.picked_later(op)? {
            return self.picked_at_run_time(id, desc, b);
        }
        let r = &op.refs;
        let v: Handle<Expression> = match op.code {
            c::DEBUG_NAME | c::DEBUG_MEMBER_NAME | c::SEMANTIC => return Ok(()),
            // Each use finds its element (`place_of`).
            c::DESCRIPTOR_LOAD => return Ok(()),
            c::PARAM_LOAD => {
                let index = self
                    .params
                    .iter()
                    .position(|&p| p == r[1])
                    .ok_or_else(|| malformed("parameter".into()))?;
                self.add(b, Expression::FunctionArgument(index as u32))
            }
            c::FUNCTION_CALL => {
                let function = *self
                    .l
                    .functions
                    .get(&r[0])
                    .ok_or_else(|| malformed("call before callee".into()))?;
                let params = ir.op(r[0])?.refs.clone();
                let mut arguments = Vec::new();
                for (&a, &p) in r[1..].iter().zip(&params) {
                    let s =
                        Lowering::scalar(ir.op(p)?.ty.basic().map_or(scalar::U32, |m| m.scalar))?;
                    arguments.push(self.get_as(b, a, s)?);
                }
                let result =
                    (!op.ty.is_void()).then(|| self.add(b, Expression::CallResult(function)));
                self.stmt(
                    b,
                    Statement::Call {
                        function,
                        arguments,
                        result,
                    },
                );
                match result {
                    Some(h) => h,
                    None => return Ok(()),
                }
            }
            // Rasterizer ordered views (WebGPU has no fragment interlock).
            c::ROV_SCOPED_LOCK_BEGIN | c::ROV_SCOPED_LOCK_END => {
                return Err(unsupported(crate::refusal::RASTERIZER_ORDERED_VIEW));
            }
            // A fence of the whole device reaches only the workgroup: with no view globally
            // coherent (refused at its declaration), no other workgroup's stores are seen anyway.
            c::BARRIER => {
                let (exec, mem) = (op.lits[0], op.lits[2]);
                let mut flags = naga::Barrier::empty();
                if mem & ir::memory::LDS != 0 {
                    flags |= naga::Barrier::WORK_GROUP;
                }
                if mem & ir::memory::UAV != 0 {
                    flags |= naga::Barrier::STORAGE | naga::Barrier::TEXTURE;
                }
                let s = if exec >= ir::scope::WORKGROUP {
                    Statement::ControlBarrier(flags)
                } else {
                    Statement::MemoryBarrier(flags)
                };
                self.stmt(b, s);
                return Ok(());
            }
            c::EMIT_VERTEX | c::EMIT_PRIMITIVE => {
                let stream = op.lits.first().map_or(0, |&s| s as usize);
                let g = self.l.geometry.as_ref();
                let function = g
                    .and_then(|g| match op.code {
                        c::EMIT_VERTEX => g.emit.get(stream).copied(),
                        _ => g.cut,
                    })
                    .ok_or_else(|| malformed("an emit outside a geometry shader".into()))?;
                self.stmt(
                    b,
                    Statement::Call {
                        function,
                        arguments: Vec::new(),
                        result: None,
                    },
                );
                return Ok(());
            }
            c::DEMOTE => {
                self.l.discards = true;
                self.stmt(b, Statement::Kill);
                return Ok(());
            }
            c::COMPOSITE_CONSTRUCT => {
                let mut parts = Vec::new();
                for &m in r {
                    parts.push(self.get(b, m)?);
                }
                if op.ty.members.len() > 1 {
                    self.define(b, id, Val::Tuple(parts));
                    return Ok(());
                }
                let ty = self.l.ir_ty(&op.ty)?;
                self.add(
                    b,
                    Expression::Compose {
                        ty,
                        components: parts,
                    },
                )
            }
            c::COMPOSITE_EXTRACT => {
                let addr = ir.op(r[1])?;
                if addr.code != c::CONSTANT {
                    return Err(malformed("composite index is not a constant".into()));
                }
                if let (Some(vector), [index]) = (ir.op(r[0])?.ty.basic(), &addr.lits[..])
                    && *index >= u64::from(vector.size)
                {
                    return Err(unsupported(crate::refusal::COMPONENT_PAST_VECTOR));
                }
                let mut indices = addr.lits.iter().map(|&l| l as u32);
                let mut v = match self.val(b, r[0])? {
                    Val::Tuple(m) => {
                        let first = indices.next().unwrap_or(0) as usize;
                        *m.get(first)
                            .ok_or_else(|| malformed("member index".into()))?
                    }
                    Val::Expr(h) => h,
                };
                for index in indices {
                    v = self.add(b, Expression::AccessIndex { base: v, index });
                }
                v
            }
            c::INPUT_LOAD | c::OUTPUT_LOAD | c::SCRATCH_LOAD | c::LDS_LOAD | c::CONSTANT_LOAD => {
                let (var, ty) = self.memory_var(op.code, r[0])?;
                let addr = r.get(1).copied().unwrap_or(0);
                self.load(b, var, &ty, addr, &op.ty)?
            }
            c::OUTPUT_STORE | c::SCRATCH_STORE | c::LDS_STORE => {
                let (var, ty) = self.memory_var(op.code, r[0])?;
                self.store(b, var, &ty, r[1], r[2])?;
                return Ok(());
            }
            c::BUFFER_LOAD => match self.texel(r[0])? {
                Some(dcl) => {
                    if op.ty.members.len() > 1 {
                        return Err(unsupported(crate::refusal::SPARSE_FEEDBACK));
                    }
                    let index = self.get_as(b, r[1], Scalar::U32)?;
                    let want = op
                        .ty
                        .basic()
                        .ok_or_else(|| malformed("typed load of an aggregate".into()))?;
                    let mut v = self.texel_load(b, dcl, index)?;
                    if want.size < VEC4_SIZE {
                        v = self.e.swizzle(&mut self.f, b, v, 0, want.size)?;
                    }
                    let from = self.l.texels[&dcl].scalar();
                    self.e
                        .bitcast(&mut self.f, b, v, from, Lowering::scalar(want.scalar)?)
                }
                None => {
                    let (res, ty) = self.buffer(r[0])?;
                    self.load(b, res.var, &ty, r[1], &op.ty)?
                }
            },
            c::BUFFER_STORE => {
                self.l.side_effects = true;
                if let Some(dcl) = self.texel(r[0])? {
                    let index = self.get_as(b, r[1], Scalar::U32)?;
                    let s = self.l.texels[&dcl].scalar();
                    let v = self.get_as(b, r[2], s)?;
                    let n = self.basic_of(r[2])?.size;
                    return self.texel_store(b, dcl, index, v, n);
                }
                let (res, ty) = self.buffer(r[0])?;
                self.store(b, res.var, &ty, r[1], r[2])?;
                return Ok(());
            }
            c::BUFFER_QUERY_SIZE => {
                if let Some(dcl) = self.texel(r[0])? {
                    let l = &mut *self.l;
                    let t = l.texels.get_mut(&dcl).unwrap();
                    let function = t.length(&mut l.module, &mut l.reflection);
                    let v = self.add(b, Expression::CallResult(function));
                    self.stmt(
                        b,
                        Statement::Call {
                            function,
                            arguments: Vec::new(),
                            result: Some(v),
                        },
                    );
                    self.define(b, id, Val::Expr(v));
                    return Ok(());
                }
                let (res, _) = self.buffer(r[0])?;
                let p = self.add(b, Expression::GlobalVariable(res.var));
                self.add(b, Expression::ArrayLength(p))
            }
            c::IMAGE_SAMPLE | c::IMAGE_GATHER => self.sample(b, op)?,
            c::IMAGE_LOAD if self.texel(r[0])?.is_some() => {
                if op.flags & ir::flag::SPARSE_FEEDBACK != 0 {
                    return Err(unsupported(crate::refusal::SPARSE_FEEDBACK));
                }
                let dcl = self.texel(r[0])?.unwrap();
                let index = self.texel_at(b, dcl, r[2], r[3])?;
                self.texel_load(b, dcl, index)?
            }
            c::IMAGE_LOAD => self.image_load(b, op)?,
            c::IMAGE_QUERY_SIZE if self.texel(r[0])?.is_some() => {
                let dcl = self.texel(r[0])?.unwrap();
                let (k, arrayed) = self.l.resources[&dcl].image.unwrap();
                let l = &mut *self.l;
                let function = l
                    .texels
                    .get_mut(&dcl)
                    .unwrap()
                    .size(&mut l.module, &mut l.reflection);
                let v = self.add(b, Expression::CallResult(function));
                self.stmt(
                    b,
                    Statement::Call {
                        function,
                        arguments: Vec::new(),
                        result: Some(v),
                    },
                );
                // Width, height and depth as the dimension has them; the layers apart.
                let n = match k {
                    kind::IMAGE_1D | kind::IMAGE_1D_ARRAY => 1,
                    kind::IMAGE_3D => 3,
                    _ => 2,
                };
                let size = self.e.swizzle(&mut self.f, b, v, 0, n)?;
                let layers = if arrayed {
                    self.add(b, Expression::AccessIndex { base: v, index: 2 })
                } else {
                    self.lit(b, Literal::U32(1))
                };
                self.define(b, id, Val::Tuple(vec![size, layers]));
                return Ok(());
            }
            c::IMAGE_QUERY_SIZE => {
                let res = self.image(r[0])?;
                let (k, arrayed) = res.image.unwrap();
                let image = self.add(b, Expression::GlobalVariable(res.var));
                let level = if matches!(k, kind::IMAGE_2DMS | kind::IMAGE_2DMS_ARRAY)
                    || r[1] == 0
                    || res.storage
                {
                    None
                } else {
                    Some(self.get_as(b, r[1], Scalar::U32)?)
                };
                let size = self.add(
                    b,
                    Expression::ImageQuery {
                        image,
                        query: ImageQuery::Size { level },
                    },
                );
                let size = if matches!(k, kind::IMAGE_1D | kind::IMAGE_1D_ARRAY) {
                    self.add(
                        b,
                        Expression::AccessIndex {
                            base: size,
                            index: 0,
                        },
                    )
                } else {
                    size
                };
                let layers = if arrayed {
                    self.add(
                        b,
                        Expression::ImageQuery {
                            image,
                            query: ImageQuery::NumLayers,
                        },
                    )
                } else {
                    self.lit(b, Literal::U32(1))
                };
                self.define(b, id, Val::Tuple(vec![size, layers]));
                return Ok(());
            }
            // Read only by checks that level 0 exists, which are true.
            c::IMAGE_QUERY_MIPS
                if ir
                    .ops
                    .iter()
                    .filter(|o| o.refs.contains(&id))
                    .all(|o| level_zero_check(ir, o)) =>
            {
                return Ok(());
            }
            c::IMAGE_QUERY_MIPS | c::IMAGE_QUERY_SAMPLES => {
                let res = self.image(r[0])?;
                let image = self.add(b, Expression::GlobalVariable(res.var));
                let query = if op.code == c::IMAGE_QUERY_MIPS {
                    ImageQuery::NumLevels
                } else {
                    ImageQuery::NumSamples
                };
                self.add(b, Expression::ImageQuery { image, query })
            }
            // Derivatives exist in pixel programs only.
            c::IMAGE_COMPUTE_LOD if self.l.stage != SourceStage::Pixel => {
                return Err(unsupported(crate::refusal::CALCULATE_LEVEL_OF_DETAIL));
            }
            c::IMAGE_COMPUTE_LOD => self.level_of_detail(b, r)?,
            c::IMAGE_STORE => {
                self.l.side_effects = true;
                if let Some(dcl) = self.texel(r[0])? {
                    let index = self.texel_at(b, dcl, r[1], r[2])?;
                    let s = self.l.texels[&dcl].scalar();
                    let v = self.get_as(b, r[3], s)?;
                    let n = self.basic_of(r[3])?.size;
                    return self.texel_store(b, dcl, index, v, n);
                }
                self.image_store(b, op)?;
                return Ok(());
            }
            c::IMAGE_ATOMIC if self.texel(r[0])?.is_none() => {
                return Err(unsupported(crate::refusal::STORAGE_IMAGE_ATOMICS));
            }
            c::LDS_ATOMIC | c::BUFFER_ATOMIC | c::IMAGE_ATOMIC | c::COUNTER_ATOMIC => {
                match self.atomic(b, op)? {
                    Some(v) => v,
                    None => return Ok(()),
                }
            }
            c::INTERPOLATE_AT_CENTROID | c::INTERPOLATE_AT_SAMPLE | c::INTERPOLATE_AT_OFFSET => {
                self.evaluate(b, op)?
            }
            c::DERIV_X | c::DERIV_Y => {
                let expr = self.get(b, r[0])?;
                let axis = if op.code == c::DERIV_X {
                    naga::DerivativeAxis::X
                } else {
                    naga::DerivativeAxis::Y
                };
                let ctrl = match op.lits.first() {
                    Some(&derivative::COARSE) => naga::DerivativeControl::Coarse,
                    Some(&derivative::FINE) => naga::DerivativeControl::Fine,
                    _ => naga::DerivativeControl::None,
                };
                self.add(b, Expression::Derivative { axis, ctrl, expr })
            }
            _ => self.arithmetic(b, op)?,
        };
        self.define(b, id, Val::Expr(v));
        Ok(())
    }

    /// An atomic operation on shared memory, a storage buffer or a view's counter; its result (the
    /// value before, or after for a counter's decrement as Direct3D defines it) unless it has none.
    fn atomic(&mut self, b: &mut Block, op: &Op) -> Result<Option<Handle<Expression>>> {
        use naga::AtomicFunction as A;
        let r = &op.refs;
        let what = *op
            .lits
            .first()
            .ok_or_else(|| malformed("atomic without its operation".into()))?;
        let typed = match op.code {
            code::BUFFER_ATOMIC | code::IMAGE_ATOMIC => self.texel(r[0])?,
            _ => None,
        };
        // A storage texture's operands follow its layer and coordinate; a counter has none.
        let operands = match op.code {
            code::IMAGE_ATOMIC => r[3],
            _ => r.get(2).copied().unwrap_or(0),
        };
        let (pointer, s) = if let Some(dcl) = typed {
            // A typed view's element: through its atomic function, by index.
            self.l.side_effects = true;
            let index = match op.code {
                code::IMAGE_ATOMIC => self.texel_at(b, dcl, r[1], r[2])?,
                _ => self.get_as(b, r[1], Scalar::U32)?,
            };
            (index, self.l.texels[&dcl].scalar())
        } else if op.code == code::COUNTER_ATOMIC {
            let (res, _) = self.buffer(r[0])?;
            self.l.side_effects = true;
            (
                self.add(b, Expression::GlobalVariable(res.var)),
                Scalar::U32,
            )
        } else {
            let (var, ty) = if op.code == code::LDS_ATOMIC {
                self.memory_var(code::LDS_LOAD, r[0])?
            } else {
                self.l.side_effects = true;
                let (res, ty) = self.buffer(r[0])?;
                (res.var, ty)
            };
            match self.walk(b, var, &ty, r[1], 1)? {
                Place::Element(p, element, None | Some(Index::Const(0))) if element.size == 1 => {
                    (p, Lowering::scalar(element.scalar)?)
                }
                _ => return Err(malformed("atomic on a vector".into())),
            }
        };
        let want = op
            .ty
            .basic()
            .map(|m| Lowering::scalar(m.scalar))
            .transpose()?;
        let operand = |me: &mut Self, b: &mut Block, k: u32| -> Result<Handle<Expression>> {
            let v = me.get_as(b, operands, s)?;
            Ok(if me.basic_of(operands)?.size == 1 {
                v
            } else {
                me.add(b, Expression::AccessIndex { base: v, index: k })
            })
        };
        let (fun, value) = match what {
            // Load and store: atomic loads and stores in naga's form.
            atomic_op::LOAD => {
                let v = match typed {
                    Some(dcl) => {
                        let v = self.texel_load(b, dcl, pointer)?;
                        self.add(b, Expression::AccessIndex { base: v, index: 0 })
                    }
                    None => self.add(b, Expression::Load { pointer }),
                };
                return Ok(Some(self.retyped(b, v, s, want)));
            }
            atomic_op::STORE => {
                let value = operand(self, b, 0)?;
                match typed {
                    Some(dcl) => self.texel_store(b, dcl, pointer, value, 1)?,
                    None => self.stmt(b, Statement::Store { pointer, value }),
                }
                return Ok(None);
            }
            atomic_op::EXCHANGE => (A::Exchange { compare: None }, operand(self, b, 0)?),
            atomic_op::COMPARE_EXCHANGE => {
                let compare = operand(self, b, 0)?;
                (
                    A::Exchange {
                        compare: Some(compare),
                    },
                    operand(self, b, 1)?,
                )
            }
            atomic_op::ADD => (A::Add, operand(self, b, 0)?),
            atomic_op::SUB => (A::Subtract, operand(self, b, 0)?),
            atomic_op::S_MIN..=atomic_op::U_MAX => {
                let signed = what <= atomic_op::S_MAX;
                if signed != (s == Scalar::I32) {
                    if typed.is_some() {
                        return Err(unsupported(crate::refusal::ATOMIC_SIGNEDNESS));
                    }
                    let value = operand(self, b, 0)?;
                    let v = self.exchange_loop(
                        b,
                        pointer,
                        s,
                        value,
                        matches!(what, atomic_op::S_MIN | atomic_op::U_MIN),
                    )?;
                    return Ok(want.is_some().then(|| self.retyped(b, v, s, want)));
                }
                let fun = match what {
                    atomic_op::S_MIN | atomic_op::U_MIN => A::Min,
                    _ => A::Max,
                };
                (fun, operand(self, b, 0)?)
            }
            atomic_op::AND => (A::And, operand(self, b, 0)?),
            atomic_op::OR => (A::InclusiveOr, operand(self, b, 0)?),
            atomic_op::XOR => (A::ExclusiveOr, operand(self, b, 0)?),
            atomic_op::INC | atomic_op::DEC => {
                let one = match s {
                    Scalar::I32 => Literal::I32(1),
                    _ => Literal::U32(1),
                };
                let fun = if what == atomic_op::INC {
                    A::Add
                } else {
                    A::Subtract
                };
                (fun, self.lit(b, one))
            }
            _ => return Err(malformed(format!("atomic operation {what}"))),
        };
        let compared = matches!(fun, A::Exchange { compare: Some(_) });
        if let Some(dcl) = typed {
            let l = &mut *self.l;
            let t = l.texels.get_mut(&dcl).unwrap();
            let function = t.atomic(&mut l.module, &mut l.reflection, fun)?;
            let mut arguments = vec![pointer, value];
            if let A::Exchange { compare: Some(c) } = fun {
                arguments.push(c);
            }
            let v = self.add(b, Expression::CallResult(function));
            self.stmt(
                b,
                Statement::Call {
                    function,
                    arguments,
                    result: Some(v),
                },
            );
            return Ok(want.is_some().then(|| self.retyped(b, v, s, want)));
        }
        // A compare-exchange always takes its result: naga's HLSL writer has no form without it.
        let result = (want.is_some() || compared).then(|| {
            let ty = if compared {
                self.l.module.generate_predeclared_type(
                    naga::PredeclaredType::AtomicCompareExchangeWeakResult(s),
                )
            } else {
                self.l.scalar_ty(s)
            };
            self.add(
                b,
                Expression::AtomicResult {
                    ty,
                    comparison: compared,
                },
            )
        });
        self.stmt(
            b,
            Statement::Atomic {
                pointer,
                fun,
                value,
                result,
            },
        );
        let (Some(mut v), Some(_)) = (result, want) else {
            return Ok(None);
        };
        if compared {
            v = self.add(b, Expression::AccessIndex { base: v, index: 0 });
        }
        if op.code == code::COUNTER_ATOMIC && what == atomic_op::DEC {
            let one = self.lit(b, Literal::U32(1));
            v = self.bin(b, BinaryOperator::Subtract, v, one);
        }
        Ok(Some(self.retyped(b, v, s, want)))
    }

    /// An atomic minimum (or maximum) of `value` into `pointer` in the other signedness than the
    /// memory's scalar `s`, by a compare-exchange loop; gives the old value. Each try compares with
    /// the value the last exchange gave, never with a plain load, which a cache may keep stale.
    fn exchange_loop(
        &mut self,
        b: &mut Block,
        pointer: Handle<Expression>,
        s: Scalar,
        value: Handle<Expression>,
        min: bool,
    ) -> Result<Handle<Expression>> {
        if !self
            .l
            .reflection
            .slow_paths
            .contains(&SlowPath::AtomicExchangeLoop)
        {
            self.l
                .reflection
                .slow_paths
                .push(SlowPath::AtomicExchangeLoop);
        }
        let other = match s {
            Scalar::I32 => Scalar::U32,
            _ => Scalar::I32,
        };
        let ty = self.l.scalar_ty(s);
        let old = self.f.local_variables.append(
            LocalVariable {
                name: None,
                ty,
                init: None,
            },
            Span::UNDEFINED,
        );
        let operand = self.e.bitcast(&mut self.f, b, value, s, other);
        let first = self.add(b, Expression::Load { pointer });
        let var = self.add(b, Expression::LocalVariable(old));
        self.stmt(
            b,
            Statement::Store {
                pointer: var,
                value: first,
            },
        );
        let mut body = Block::new();
        let var = self.add(&mut body, Expression::LocalVariable(old));
        let current = self.add(&mut body, Expression::Load { pointer: var });
        let a = self.e.bitcast(&mut self.f, &mut body, current, s, other);
        let fun = if min {
            MathFunction::Min
        } else {
            MathFunction::Max
        };
        let m = self.add(
            &mut body,
            Expression::Math {
                fun,
                arg: a,
                arg1: Some(operand),
                arg2: None,
                arg3: None,
            },
        );
        let new = self.e.bitcast(&mut self.f, &mut body, m, other, s);
        let result_ty = self
            .l
            .module
            .generate_predeclared_type(naga::PredeclaredType::AtomicCompareExchangeWeakResult(s));
        let result = self.add(
            &mut body,
            Expression::AtomicResult {
                ty: result_ty,
                comparison: true,
            },
        );
        self.stmt(
            &mut body,
            Statement::Atomic {
                pointer,
                fun: naga::AtomicFunction::Exchange {
                    compare: Some(current),
                },
                value: new,
                result: Some(result),
            },
        );
        let seen = self.add(
            &mut body,
            Expression::AccessIndex {
                base: result,
                index: 0,
            },
        );
        self.stmt(
            &mut body,
            Statement::Store {
                pointer: var,
                value: seen,
            },
        );
        let exchanged = self.add(
            &mut body,
            Expression::AccessIndex {
                base: result,
                index: 1,
            },
        );
        // A helper lane's exchange does nothing and gives no defined value: it leaves.
        let flag = naga::front::spv::helper_invocation_flag(&mut self.l.module, Span::UNDEFINED);
        let flag = self.add(&mut body, Expression::GlobalVariable(flag));
        let helper = self.add(&mut body, Expression::Load { pointer: flag });
        let leave = self.bin(&mut body, BinaryOperator::LogicalOr, exchanged, helper);
        self.e.flush(&self.f, &mut body);
        let mut accept = Block::new();
        accept.push(Statement::Break, Span::UNDEFINED);
        body.push(
            Statement::If {
                condition: leave,
                accept,
                reject: Block::new(),
            },
            Span::UNDEFINED,
        );
        self.stmt(
            b,
            Statement::Loop {
                body,
                continuing: Block::new(),
                break_if: None,
            },
        );
        let var = self.add(b, Expression::LocalVariable(old));
        Ok(self.add(b, Expression::Load { pointer: var }))
    }

    /// `v` of scalar `from` as the result type's scalar.
    fn retyped(
        &mut self,
        b: &mut Block,
        v: Handle<Expression>,
        from: Scalar,
        want: Option<Scalar>,
    ) -> Handle<Expression> {
        match want {
            Some(to) => self.e.bitcast(&mut self.f, b, v, from, to),
            None => v,
        }
    }

    fn memory_var(&mut self, opcode: u16, dcl: u32) -> Result<(Handle<GlobalVariable>, Ty)> {
        if opcode == code::CONSTANT_LOAD {
            return self.l.constant_array(dcl);
        }
        if let Some(&i) = self.l.io_index.get(&dcl) {
            let io = &self.l.io[i].1;
            return Ok((io.var, io.ty.clone()));
        }
        let var = *self
            .l
            .memory
            .get(&dcl)
            .ok_or_else(|| malformed(format!("memory {dcl}")))?;
        Ok((var, self.l.declared_ty(dcl)?))
    }

    /// The element a descriptor loads: the one its constant index names, or the one the switch
    /// lowering this instruction picked ([`Self::picked_at_run_time`]).
    fn place_of(&self, desc: u32) -> Result<ResourceElement> {
        let op = self.l.ir.op(desc)?;
        let dcl = op.refs[0];
        let element = match self.picked.get(&desc) {
            Some(&element) => element,
            None => self.constant_index(op)?,
        };
        if element >= self.l.lengths.get(&dcl).copied().unwrap_or(1) {
            return Err(malformed(format!("descriptor {desc} past its array")));
        }
        Ok(ResourceElement { dcl, element })
    }

    /// A descriptor load's index, where it is a constant (zero where it has none).
    fn constant_index(&self, op: &Op) -> Result<u32> {
        let Some(&i) = op.refs.get(1) else {
            return Ok(0);
        };
        let index = self.l.ir.op(i)?;
        match (index.code, index.lits.first()) {
            (code::CONSTANT, Some(&l)) => Ok(l as u32),
            _ => Err(malformed(format!("descriptor index {i} not constant"))),
        }
    }

    /// A descriptor `op` uses that picks an element of an array at run time and that no enclosing
    /// switch has picked yet.
    fn picked_later(&self, op: &Op) -> Result<Option<u32>> {
        let ir = self.l.ir;
        for &r in &op.refs {
            let Ok(d) = ir.op(r) else { continue };
            if d.code == code::DESCRIPTOR_LOAD
                && !self.picked.contains_key(&r)
                && let Some(&i) = d.refs.get(1)
                && ir.op(i)?.code != code::CONSTANT
            {
                return Ok(Some(r));
            }
        }
        Ok(None)
    }

    /// Lowers instruction `id`, whose descriptor `desc` picks an element of an array at run time: a
    /// switch on the index with a case per element, each lowering the instruction with its element,
    /// the result carried out through locals. An index past the array takes the last element
    /// (Direct3D leaves it undefined).
    fn picked_at_run_time(&mut self, id: u32, desc: u32, b: &mut Block) -> Result<()> {
        let ir = self.l.ir;
        let d = ir.op(desc)?;
        let length = self.l.lengths.get(&d.refs[0]).copied().unwrap_or(1);
        let selector = self.get_as(b, d.refs[1], Scalar::U32)?;
        self.e.flush(&self.f, b);
        let mut cases = Vec::new();
        let mut carried: Option<Vec<Handle<LocalVariable>>> = None;
        for element in 0..length {
            let mut body = Block::new();
            self.picked.insert(desc, element);
            let lowered = self.instruction(id, &mut body);
            self.picked.remove(&desc);
            lowered?;
            if let Some(v) = self.vals.remove(&id) {
                let parts = match v {
                    Val::Expr(h) => vec![h],
                    Val::Tuple(parts) => parts,
                };
                let locals = match carried.take() {
                    Some(locals) => locals,
                    None => self.carriers(id, parts.len())?,
                };
                for (&value, &local) in parts.iter().zip(&locals) {
                    let pointer = self.add(&mut body, Expression::LocalVariable(local));
                    self.stmt(&mut body, Statement::Store { pointer, value });
                }
                carried = Some(locals);
            }
            self.e.flush(&self.f, &mut body);
            let value = match element + 1 == length {
                true => SwitchValue::Default,
                false => SwitchValue::U32(element),
            };
            cases.push(SwitchCase {
                value,
                body,
                fall_through: false,
            });
        }
        self.stmt(b, Statement::Switch { selector, cases });
        if let Some(locals) = carried {
            let mut parts = Vec::new();
            for local in locals {
                let pointer = self.add(b, Expression::LocalVariable(local));
                parts.push(self.add(b, Expression::Load { pointer }));
            }
            let v = match parts[..] {
                [h] => Val::Expr(h),
                _ => Val::Tuple(parts),
            };
            self.define(b, id, v);
        }
        Ok(())
    }

    /// Locals carrying the `n` parts of value `id` out of a switch's cases.
    fn carriers(&mut self, id: u32, n: usize) -> Result<Vec<Handle<LocalVariable>>> {
        let ty = &self.l.ir.op(id)?.ty;
        let types: Vec<Handle<Type>> = match n {
            1 => vec![self.l.ir_ty(ty)?],
            _ => {
                let mut types = Vec::new();
                for &m in &ty.members {
                    let inner = Lowering::basic_inner(m)?;
                    types.push(self.l.ty(inner));
                }
                types
            }
        };
        Ok(types
            .into_iter()
            .map(|ty| {
                self.f.local_variables.append(
                    LocalVariable {
                        name: None,
                        ty,
                        init: None,
                    },
                    Span::UNDEFINED,
                )
            })
            .collect())
    }

    /// The element a descriptor loads, where it is a typed buffer.
    fn texel(&self, desc: u32) -> Result<Option<ResourceElement>> {
        let place = self.place_of(desc)?;
        Ok(self.l.texels.contains_key(&place).then_some(place))
    }

    /// The index of a texel of storage texture `dcl` read through a buffer, from an image operation's
    /// layer and coordinate operands; past every index where outside the view.
    fn texel_at(
        &mut self,
        b: &mut Block,
        dcl: ResourceElement,
        layer: u32,
        coord: u32,
    ) -> Result<Handle<Expression>> {
        let c = self.get_as(b, coord, Scalar::U32)?;
        let n = self.basic_of(coord)?.size;
        let mut xyz = Vec::new();
        for i in 0..n.min(3) as u32 {
            xyz.push(if n == 1 {
                c
            } else {
                self.add(b, Expression::AccessIndex { base: c, index: i })
            });
        }
        if layer != 0 {
            xyz.push(self.get_as(b, layer, Scalar::U32)?);
        }
        while xyz.len() < 3 {
            xyz.push(self.lit(b, Literal::U32(0)));
        }
        if xyz.len() > 3 {
            return Err(malformed("storage texture coordinate".into()));
        }
        // A 1D array's layers are its rows' places in the layout.
        let (k, _) = self.l.resources[&dcl].image.unwrap();
        if k == kind::IMAGE_1D_ARRAY {
            xyz.swap(1, 2);
        }
        let l = &mut *self.l;
        let function = l
            .texels
            .get_mut(&dcl)
            .unwrap()
            .at(&mut l.module, &mut l.reflection);
        let v = self.add(b, Expression::CallResult(function));
        self.stmt(
            b,
            Statement::Call {
                function,
                arguments: xyz,
                result: Some(v),
            },
        );
        Ok(v)
    }

    /// Element `index` of typed buffer `dcl`, four components of its format's scalar.
    fn texel_load(
        &mut self,
        b: &mut Block,
        dcl: ResourceElement,
        index: Handle<Expression>,
    ) -> Result<Handle<Expression>> {
        let l = &mut *self.l;
        let t = l.texels.get_mut(&dcl).unwrap();
        let function = t.load(&mut l.module, &mut l.reflection);
        let v = self.add(b, Expression::CallResult(function));
        self.stmt(
            b,
            Statement::Call {
                function,
                arguments: vec![index],
                result: Some(v),
            },
        );
        Ok(v)
    }

    /// Writes `v` (`n` components of the format's scalar) to element `index` of typed buffer `dcl`.
    fn texel_store(
        &mut self,
        b: &mut Block,
        dcl: ResourceElement,
        index: Handle<Expression>,
        v: Handle<Expression>,
        n: u8,
    ) -> Result<()> {
        let s = self.l.texels[&dcl].scalar();
        let value = if n >= VEC4_SIZE {
            v
        } else {
            let zero = self.lit(
                b,
                match s.kind {
                    ScalarKind::Float => Literal::F32(0.0),
                    ScalarKind::Sint => Literal::I32(0),
                    _ => Literal::U32(0),
                },
            );
            let mut components = Vec::new();
            for i in 0..4 {
                components.push(match (i < n as u32, n) {
                    (true, 1) => v,
                    (true, _) => self.add(b, Expression::AccessIndex { base: v, index: i }),
                    (false, _) => zero,
                });
            }
            let ty = self.l.vec_ty(4, s)?;
            self.add(b, Expression::Compose { ty, components })
        };
        let l = &mut *self.l;
        let t = l.texels.get_mut(&dcl).unwrap();
        let function = t.store(&mut l.module, &mut l.reflection);
        self.stmt(
            b,
            Statement::Call {
                function,
                arguments: vec![index, value],
                result: None,
            },
        );
        Ok(())
    }

    fn buffer(&self, desc: u32) -> Result<(Resource, Ty)> {
        let place = self.place_of(desc)?;
        let res = *self
            .l
            .resources
            .get(&place)
            .ok_or_else(|| malformed(format!("descriptor {desc}")))?;
        Ok((res, self.l.declared_ty(place.dcl)?))
    }

    fn image(&self, desc: u32) -> Result<Resource> {
        let (res, _) = self.buffer(desc)?;
        if res.image.is_none() {
            return Err(malformed("not an image".into()));
        }
        Ok(res)
    }

    /// The indices an address operand holds.
    fn address(&mut self, b: &mut Block, addr: u32) -> Result<Vec<Index>> {
        if addr == 0 {
            return Ok(Vec::new());
        }
        let ir = self.l.ir;
        let op = ir.op(addr)?;
        let n = op.ty.basic().map_or(1, |m| m.size);
        let mut out = Vec::new();
        match op.code {
            code::CONSTANT => out.extend(op.lits.iter().map(|&l| Index::Const(l as u32))),
            code::COMPOSITE_CONSTRUCT => {
                for &m in &op.refs {
                    let mo = ir.op(m)?;
                    if mo.code == code::CONSTANT {
                        out.push(Index::Const(mo.lits[0] as u32));
                    } else {
                        out.push(Index::Dyn(self.get_as(b, m, Scalar::U32)?));
                    }
                }
            }
            _ if n == 1 => out.push(Index::Dyn(self.get_as(b, addr, Scalar::U32)?)),
            _ => {
                let v = self.get_as(b, addr, Scalar::U32)?;
                for i in 0..n as u32 {
                    out.push(Index::Dyn(
                        self.add(b, Expression::AccessIndex { base: v, index: i }),
                    ));
                }
            }
        }
        Ok(out)
    }

    /// Where an address leads in `var` (of IR type `ty`): a pointer, the element type there, and how
    /// to reach the value: one component of a vector, or `n` consecutive scalars of an array.
    fn walk(
        &mut self,
        b: &mut Block,
        var: Handle<GlobalVariable>,
        ty: &Ty,
        addr: u32,
        n: u8,
    ) -> Result<Place> {
        let indices = self.address(b, addr)?;
        let element = *ty
            .members
            .first()
            .ok_or_else(|| malformed("void memory".into()))?;
        let dims = ty.dims.len();
        if ty.members.len() > 1 {
            return self.walk_struct(b, var, ty, &indices, n);
        }
        if indices.len() > dims + 1 {
            return Err(malformed("address too long".into()));
        }
        // Consecutive scalars: the innermost array index (zero when absent) is where they start.
        let run = element.size == 1 && n > 1 && dims > 0;
        let applied = if run {
            indices.len().min(dims - 1)
        } else {
            indices.len().min(dims)
        };
        let mut p = self.add(b, Expression::GlobalVariable(var));
        for &i in &indices[..applied] {
            p = self.index(b, p, i);
        }
        if run {
            let start = indices.get(dims - 1).copied().unwrap_or(Index::Const(0));
            return Ok(Place::Run(p, start, element));
        }
        Ok(Place::Element(p, element, indices.get(dims).copied()))
    }

    /// [`Self::walk`] in an array of structs: the index after the array's is the member's, then the
    /// component's. Several consecutive scalars are consecutive members of one type.
    fn walk_struct(
        &mut self,
        b: &mut Block,
        var: Handle<GlobalVariable>,
        ty: &Ty,
        indices: &[Index],
        n: u8,
    ) -> Result<Place> {
        let dims = ty.dims.len();
        if indices.len() <= dims || indices.len() > dims + 2 {
            return Err(malformed("a struct member's address".into()));
        }
        let Index::Const(m) = indices[dims] else {
            return Err(malformed("a struct member chosen at run time".into()));
        };
        let element = *ty
            .members
            .get(m as usize)
            .ok_or_else(|| malformed("a struct member past the last".into()))?;
        let mut p = self.add(b, Expression::GlobalVariable(var));
        for &i in &indices[..dims] {
            p = self.index(b, p, i);
        }
        if element.size == 1 && n > 1 {
            let run = ty.members.get(m as usize..m as usize + n as usize);
            if !run.is_some_and(|r| r.iter().all(|&e| e == element)) {
                return Err(unsupported(crate::refusal::STRUCT_MEMBERS_AS_VECTOR));
            }
            return Ok(Place::Run(p, Index::Const(m), element));
        }
        let p = self.add(b, Expression::AccessIndex { base: p, index: m });
        Ok(Place::Element(p, element, indices.get(dims + 1).copied()))
    }

    fn index(&mut self, b: &mut Block, base: Handle<Expression>, i: Index) -> Handle<Expression> {
        match i {
            Index::Const(index) => self.add(b, Expression::AccessIndex { base, index }),
            Index::Dyn(index) => self.add(b, Expression::Access { base, index }),
        }
    }

    /// Index `start + k`.
    fn offset_index(&mut self, b: &mut Block, start: Index, k: u32) -> Index {
        match start {
            Index::Const(c) => Index::Const(c + k),
            Index::Dyn(_) if k == 0 => start,
            Index::Dyn(h) => {
                let kk = self.lit(b, Literal::U32(k));
                Index::Dyn(self.bin(b, BinaryOperator::Add, h, kk))
            }
        }
    }

    fn load(
        &mut self,
        b: &mut Block,
        var: Handle<GlobalVariable>,
        ty: &Ty,
        addr: u32,
        result: &Ty,
    ) -> Result<Handle<Expression>> {
        let want = result
            .basic()
            .ok_or_else(|| malformed("load of an aggregate".into()))?;
        let to = Lowering::scalar(want.scalar)?;
        let (v, element) = match self.walk(b, var, ty, addr, want.size)? {
            Place::Run(p, start, element) => {
                let mut comps = Vec::new();
                for k in 0..want.size as u32 {
                    let i = self.offset_index(b, start, k);
                    let q = self.index(b, p, i);
                    comps.push(self.add(b, Expression::Load { pointer: q }));
                }
                let ty = self
                    .l
                    .vec_ty(want.size, Lowering::scalar(element.scalar)?)?;
                (
                    self.add(
                        b,
                        Expression::Compose {
                            ty,
                            components: comps,
                        },
                    ),
                    element,
                )
            }
            Place::Element(p, element, comp) => {
                let v = match comp {
                    None if element.size == want.size => {
                        self.add(b, Expression::Load { pointer: p })
                    }
                    None if element.size > want.size => {
                        let whole = self.add(b, Expression::Load { pointer: p });
                        self.e.swizzle(&mut self.f, b, whole, 0, want.size)?
                    }
                    Some(Index::Const(c)) if c + want.size as u32 <= element.size as u32 => {
                        let whole = self.add(b, Expression::Load { pointer: p });
                        if element.size == 1 {
                            whole
                        } else {
                            self.e.swizzle(&mut self.f, b, whole, c, want.size)?
                        }
                    }
                    Some(Index::Dyn(i)) if want.size == 1 => {
                        let q = self.add(b, Expression::Access { base: p, index: i });
                        self.add(b, Expression::Load { pointer: q })
                    }
                    _ => return Err(unsupported(crate::refusal::VECTOR_LOAD_ACROSS_ELEMENTS)),
                };
                (v, element)
            }
        };
        let from = Lowering::scalar(element.scalar)?;
        Ok(self.e.bitcast(&mut self.f, b, v, from, to))
    }

    fn store(
        &mut self,
        b: &mut Block,
        var: Handle<GlobalVariable>,
        ty: &Ty,
        addr: u32,
        value: u32,
    ) -> Result<()> {
        let have = self.basic_of(value)?;
        let parts = |s: &mut Self, b: &mut Block, v: Handle<Expression>, i: u32| {
            if have.size == 1 {
                v
            } else {
                s.add(b, Expression::AccessIndex { base: v, index: i })
            }
        };
        match self.walk(b, var, ty, addr, have.size)? {
            Place::Run(p, start, element) => {
                let v = self.get_as(b, value, Lowering::scalar(element.scalar)?)?;
                for k in 0..have.size as u32 {
                    let i = self.offset_index(b, start, k);
                    let q = self.index(b, p, i);
                    let x = parts(self, b, v, k);
                    self.stmt(
                        b,
                        Statement::Store {
                            pointer: q,
                            value: x,
                        },
                    );
                }
            }
            Place::Element(p, element, comp) => {
                let v = self.get_as(b, value, Lowering::scalar(element.scalar)?)?;
                match comp {
                    None if element.size == have.size => {
                        self.stmt(
                            b,
                            Statement::Store {
                                pointer: p,
                                value: v,
                            },
                        );
                    }
                    Some(Index::Const(c)) if c + have.size as u32 <= element.size as u32 => {
                        if element.size == 1 {
                            self.stmt(
                                b,
                                Statement::Store {
                                    pointer: p,
                                    value: v,
                                },
                            );
                            return Ok(());
                        }
                        for i in 0..have.size as u32 {
                            let q = self.add(
                                b,
                                Expression::AccessIndex {
                                    base: p,
                                    index: c + i,
                                },
                            );
                            let x = parts(self, b, v, i);
                            self.stmt(
                                b,
                                Statement::Store {
                                    pointer: q,
                                    value: x,
                                },
                            );
                        }
                    }
                    Some(Index::Dyn(i)) if have.size == 1 => {
                        let q = self.add(b, Expression::Access { base: p, index: i });
                        self.stmt(
                            b,
                            Statement::Store {
                                pointer: q,
                                value: v,
                            },
                        );
                    }
                    _ => return Err(unsupported(crate::refusal::VECTOR_STORE_ACROSS_ELEMENTS)),
                }
            }
        }
        Ok(())
    }

    /// Texture coordinates as the common form takes them: 1D textures are 2D textures one texel high.
    fn coords(
        &mut self,
        b: &mut Block,
        k: u64,
        id: u32,
        float: bool,
    ) -> Result<Handle<Expression>> {
        let s = self.scalar_of(id)?;
        let want = if float {
            Scalar::F32
        } else if s.kind == ScalarKind::Sint {
            Scalar::I32
        } else {
            Scalar::U32
        };
        let v = self.get_as(b, id, want)?;
        if matches!(k, kind::IMAGE_1D | kind::IMAGE_1D_ARRAY) {
            let y = self.lit(b, if float { Literal::F32(0.5) } else { zero(want) });
            let ty = self.l.vec_ty(2, want)?;
            return Ok(self.add(
                b,
                Expression::Compose {
                    ty,
                    components: vec![v, y],
                },
            ));
        }
        Ok(v)
    }

    /// A sample's gradient `id` in image kind `k`: as its coordinate, but a 1D texture's row has no
    /// extent in y.
    fn gradient(&mut self, b: &mut Block, k: u64, id: u32) -> Result<Handle<Expression>> {
        let v = self.get_as(b, id, Scalar::F32)?;
        if !matches!(k, kind::IMAGE_1D | kind::IMAGE_1D_ARRAY) {
            return Ok(v);
        }
        let y = self.lit(b, Literal::F32(0.0));
        let ty = self.l.vec_ty(2, Scalar::F32)?;
        Ok(self.add(
            b,
            Expression::Compose {
                ty,
                components: vec![v, y],
            },
        ))
    }

    /// `lod`: the level of detail an implicit sample of image `r[0]` through sampler `r[1]` at
    /// coordinate `r[2]` takes, clamped and unclamped (`variant::lod`), from the sampler's state, bias
    /// and the resource's clamp in the helper block.
    fn level_of_detail(&mut self, b: &mut Block, r: &[u32]) -> Result<Handle<Expression>> {
        let res = self.image(r[0])?;
        let (k, _) = res.image.unwrap();
        if matches!(k, kind::IMAGE_CUBE | kind::IMAGE_CUBE_ARRAY) {
            return Err(unsupported(crate::refusal::CALCULATE_LEVEL_OF_DETAIL_CUBE));
        }
        let texture = self.slot_of(r[0])?;
        let sampler = self.slot_of(r[1])?;
        let coordinate = self.coords(b, k, r[2], true)?;
        let image = self.add(b, Expression::GlobalVariable(res.var));
        let n = if k == kind::IMAGE_3D {
            VectorSize::Tri
        } else {
            VectorSize::Bi
        } as u8;
        let q = crate::variant::lod::Query {
            image,
            dimensions: n,
            texture,
            sampler,
            coordinate,
            coordinate_size: n,
            gradients: None,
        };
        // The query emits its own expressions.
        self.e.flush(&self.f, b);
        let l = &mut *self.l;
        let result = crate::variant::lod::call(
            &mut l.module,
            &mut l.reflection,
            l.variant,
            crate::variant::lod::function,
            &mut self.f,
            b,
            &q,
            None,
        )?;
        self.e.start = self.f.expressions.len();
        Ok(result)
    }

    /// The gradients a sample (operands `r`, its clamp present) of image `image` of kind `k` at
    /// `coordinate` takes for Direct3D's per-sample clamp (`variant::lod::clamped`), from its own
    /// gradients or bias where it has them.
    fn clamped_gradients(
        &mut self,
        b: &mut Block,
        image: Handle<Expression>,
        k: u64,
        coordinate: Handle<Expression>,
        r: &[u32],
        Clamped {
            bias,
            clamp,
            dx,
            dy,
        }: Clamped,
    ) -> Result<[Handle<Expression>; 2]> {
        let n = if k == kind::IMAGE_3D {
            VectorSize::Tri
        } else {
            VectorSize::Bi
        } as u8;
        let gradients = if dx != 0 {
            Some([self.gradient(b, k, dx)?, self.gradient(b, k, dy)?])
        } else {
            None
        };
        let shift = if bias != 0 && dx == 0 {
            Some(self.get_as(b, bias, Scalar::F32)?)
        } else {
            None
        };
        let clamp = self.get_as(b, clamp, Scalar::F32)?;
        let q = crate::variant::lod::Query {
            image,
            dimensions: n,
            texture: self.slot_of(r[0])?,
            sampler: self.slot_of(r[1])?,
            coordinate,
            coordinate_size: n,
            gradients,
        };
        // The library's call emits its own expressions.
        self.e.flush(&self.f, b);
        let l = &mut *self.l;
        let result = crate::variant::lod::clamped(
            &mut l.module,
            &mut l.reflection,
            crate::variant::lod::gradients_function,
            &mut self.f,
            b,
            &q,
            shift,
            clamp,
            None,
        )?;
        let columns = crate::variant::lod::columns(&mut self.f, b, result, n);
        self.e.start = self.f.expressions.len();
        Ok(columns)
    }

    fn layer(&mut self, b: &mut Block, id: u32) -> Result<Option<Handle<Expression>>> {
        if id == 0 {
            return Ok(None);
        }
        let s = self.scalar_of(id)?;
        let v = self.get(b, id)?;
        if s.kind == ScalarKind::Float {
            let rounded = self.math(b, MathFunction::Round, &[v]);
            let z = self.lit(b, Literal::F32(0.0));
            let rounded = self.math(b, MathFunction::Max, &[rounded, z]);
            return Ok(Some(self.add(
                b,
                Expression::As {
                    expr: rounded,
                    kind: ScalarKind::Uint,
                    convert: Some(Scalar::U32.width),
                },
            )));
        }
        Ok(Some(v))
    }

    fn sample(&mut self, b: &mut Block, op: &Op) -> Result<Handle<Expression>> {
        let r = &op.refs;
        let gather = op.code == code::IMAGE_GATHER;
        if op.flags & ir::flag::SPARSE_FEEDBACK != 0 {
            return Err(unsupported(crate::refusal::SPARSE_FEEDBACK));
        }
        let res = self.image(r[0])?;
        let (sampler_res, _) = self.buffer(r[1])?;
        let (k, _) = res.image.unwrap();
        let tex_slot = self.slot_of(r[0])?;
        let smp_slot = self.slot_of(r[1])?;
        if !self
            .l
            .reflection
            .sampled_pairs
            .contains(&(tex_slot, smp_slot))
        {
            self.l.reflection.sampled_pairs.push((tex_slot, smp_slot));
        }
        let image = self.add(b, Expression::GlobalVariable(res.var));
        let sampler = self.add(b, Expression::GlobalVariable(sampler_res.var));
        let array_index = self.layer(b, r[2])?;
        let mut coordinate = self.coords(b, k, r[3], true)?;
        let offset = if r[4] == 0 {
            None
        } else if gather && self.l.ir.op(r[4])?.code != code::CONSTANT {
            coordinate = self.offset_coordinate(b, image, coordinate, r[4])?;
            None
        } else {
            Some(self.offset_constant(b, k, r[4])?)
        };
        let depth_ref_id = if gather { r[5] } else { r[10] };
        let depth_ref = if depth_ref_id != 0 {
            Some(self.get_as(b, depth_ref_id, Scalar::F32)?)
        } else {
            None
        };
        let level = if gather {
            SampleLevel::Zero
        } else {
            let (lod, bias, clamp, dx, dy) = (r[5], r[6], r[7], r[8], r[9]);
            if clamp != 0 {
                // The clamp is taken through gradients, which neither a depth texture nor a cube
                // takes in the library, and which need derivatives where the sample has no gradients.
                let cube = matches!(k, kind::IMAGE_CUBE | kind::IMAGE_CUBE_ARRAY);
                if depth_ref.is_some()
                    || res.depth
                    || cube
                    || lod != 0
                    || self.l.stage != SourceStage::Pixel
                {
                    return Err(unsupported(crate::refusal::SAMPLE_CLAMP));
                }
                let operands = Clamped {
                    bias,
                    clamp,
                    dx,
                    dy,
                };
                let [x, y] = self.clamped_gradients(b, image, k, coordinate, r, operands)?;
                SampleLevel::Gradient { x, y }
            } else if lod != 0 {
                let is_zero = {
                    let o = self.l.ir.op(lod)?;
                    o.code == code::CONSTANT && f32::from_bits(o.lits[0] as u32) == 0.0
                };
                // Level zero is naga's form for comparisons only (HLSL has no plain `SampleLevelZero`).
                if depth_ref.is_some() {
                    if !is_zero {
                        return Err(unsupported(crate::refusal::SAMPLE_COMPARE_LEVEL));
                    }
                    SampleLevel::Zero
                } else if res.depth {
                    // WGSL samples a depth texture at whole levels only.
                    let o = self.l.ir.op(lod)?;
                    let v = f32::from_bits(o.lits.first().copied().unwrap_or_default() as u32);
                    let whole = v as i32;
                    if o.code != code::CONSTANT || whole as f32 != v || whole < 0 {
                        return Err(unsupported(crate::refusal::DEPTH_SAMPLE_LEVEL));
                    }
                    SampleLevel::Exact(self.lit(b, Literal::I32(whole)))
                } else {
                    SampleLevel::Exact(self.get_as(b, lod, Scalar::F32)?)
                }
            } else if bias != 0 {
                if depth_ref.is_some() {
                    return Err(unsupported(crate::refusal::SAMPLE_COMPARE_BIAS));
                }
                if res.depth {
                    return Err(unsupported(crate::refusal::DEPTH_SAMPLE_BIAS));
                }
                SampleLevel::Bias(self.get_as(b, bias, Scalar::F32)?)
            } else if dx != 0 {
                if depth_ref.is_some() {
                    return Err(unsupported(crate::refusal::SAMPLE_COMPARE_GRADIENT));
                }
                if res.depth {
                    return Err(unsupported(crate::refusal::DEPTH_SAMPLE_GRADIENT));
                }
                let x = self.gradient(b, k, dx)?;
                let y = self.gradient(b, k, dy)?;
                SampleLevel::Gradient { x, y }
            } else {
                SampleLevel::Auto
            }
        };
        let gather_component = if gather {
            let c = op.lits.first().copied().unwrap_or(0) as usize;
            // A depth texture's other components are resolved by `variant::swizzle`, through the
            // view's mapping.
            Some(SwizzleComponent::XYZW[c.min(SwizzleComponent::XYZW.len() - 1)])
        } else {
            None
        };
        let v = self.add(
            b,
            Expression::ImageSample {
                image,
                sampler,
                gather: gather_component,
                coordinate,
                array_index,
                offset,
                level,
                depth_ref,
                clamp_to_edge: false,
            },
        );
        if res.depth && depth_ref.is_none() && !gather {
            return self.depth_vec4(b, v, op);
        }
        Ok(v)
    }

    /// A depth texture read as a color one gives (depth, 0, 0, 1).
    fn depth_vec4(
        &mut self,
        b: &mut Block,
        v: Handle<Expression>,
        op: &Op,
    ) -> Result<Handle<Expression>> {
        let want = op.ty.basic().map_or(4, |m| m.size);
        if want == 1 {
            return Ok(v);
        }
        let z = self.lit(b, Literal::F32(0.0));
        let one = self.lit(b, Literal::F32(1.0));
        let ty = self.l.vec_ty(4, Scalar::F32)?;
        let v4 = self.add(
            b,
            Expression::Compose {
                ty,
                components: vec![v, z, z, one],
            },
        );
        if want == 4 {
            return Ok(v4);
        }
        self.e.swizzle(&mut self.f, b, v4, 0, want)
    }

    fn offset_constant(&mut self, b: &mut Block, k: u64, id: u32) -> Result<Handle<Expression>> {
        let op = self.l.ir.op(id)?;
        if op.code != code::CONSTANT {
            return Err(unsupported(crate::refusal::DYNAMIC_OFFSET));
        }
        let mut comps: Vec<i32> = op.lits.iter().map(|&l| l as i32).collect();
        if matches!(k, kind::IMAGE_1D | kind::IMAGE_1D_ARRAY) {
            comps.push(0);
        }
        let c = self.l.ivec_constant(&comps)?;
        Ok(self.add(b, Expression::Constant(c)))
    }

    /// A gather's offset given at run time (`gather4_po`), which the common form takes only as a
    /// constant: the coordinate moved by the offset in texels of level 0, which picks the same four
    /// texels. Direct3D takes each component's low six bits, signed.
    fn offset_coordinate(
        &mut self,
        b: &mut Block,
        image: Handle<Expression>,
        coordinate: Handle<Expression>,
        id: u32,
    ) -> Result<Handle<Expression>> {
        let o = self.get_as(b, id, Scalar::I32)?;
        let o = self.e.swizzle(&mut self.f, b, o, 0, 2)?;
        let shift = u32::BITS - crate::common::GATHER_OFFSET_BITS;
        let six = self.splat_lit(b, Literal::U32(shift), 2)?;
        let o = self.bin(b, BinaryOperator::ShiftLeft, o, six);
        let o = self.bin(b, BinaryOperator::ShiftRight, o, six);
        let o = self.add(
            b,
            Expression::As {
                expr: o,
                kind: ScalarKind::Float,
                convert: Some(Scalar::U32.width),
            },
        );
        let zero = self.lit(b, Literal::U32(0));
        let size = self.add(
            b,
            Expression::ImageQuery {
                image,
                query: ImageQuery::Size { level: Some(zero) },
            },
        );
        let size = self.add(
            b,
            Expression::As {
                expr: size,
                kind: ScalarKind::Float,
                convert: Some(Scalar::U32.width),
            },
        );
        let shift = self.bin(b, BinaryOperator::Divide, o, size);
        Ok(self.bin(b, BinaryOperator::Add, coordinate, shift))
    }

    fn slot_of(&self, desc: u32) -> Result<u32> {
        let place = self.place_of(desc)?;
        let dcl = self.l.ir.op(place.dcl)?;
        let kind =
            declared_kind(dcl).ok_or_else(|| malformed("a descriptor of no resource".into()))?;
        self.l.element_slot(kind, dcl, place.element)
    }

    fn image_load(&mut self, b: &mut Block, op: &Op) -> Result<Handle<Expression>> {
        let r = &op.refs;
        if op.flags & ir::flag::SPARSE_FEEDBACK != 0 {
            return Err(unsupported(crate::refusal::SPARSE_FEEDBACK));
        }
        let res = self.image(r[0])?;
        let (k, _) = res.image.unwrap();
        let image = self.add(b, Expression::GlobalVariable(res.var));
        let (mip, layer, coord, sample, offset) = (r[1], r[2], r[3], r[4], r[5]);
        let array_index = self.layer(b, layer)?;
        let mut coordinate = self.coords(b, k, coord, false)?;
        if offset != 0 {
            let o = self.l.ir.op(offset)?;
            if o.code != code::CONSTANT {
                return Err(unsupported(crate::refusal::DYNAMIC_OFFSET));
            }
            let s = self.scalar_of(coord)?;
            let s = if s.kind == ScalarKind::Sint {
                Scalar::I32
            } else {
                Scalar::U32
            };
            let mut comps = Vec::new();
            for &l in &o.lits {
                comps.push(self.lit(
                    b,
                    if s == Scalar::I32 {
                        Literal::I32(l as i32)
                    } else {
                        Literal::U32(l as u32)
                    },
                ));
            }
            if matches!(k, kind::IMAGE_1D | kind::IMAGE_1D_ARRAY) {
                comps.push(self.lit(b, zero(s)));
            }
            let off = if comps.len() == 1 {
                comps[0]
            } else {
                let ty = self.l.vec_ty(comps.len() as u8, s)?;
                self.add(
                    b,
                    Expression::Compose {
                        ty,
                        components: comps,
                    },
                )
            };
            coordinate = self.bin(b, BinaryOperator::Add, coordinate, off);
        }
        let (sample, level) = if res.storage {
            (None, None)
        } else if matches!(k, kind::IMAGE_2DMS | kind::IMAGE_2DMS_ARRAY) {
            (Some(self.get(b, sample)?), None)
        } else if mip != 0 {
            (None, Some(self.get(b, mip)?))
        } else {
            (None, Some(self.lit(b, Literal::U32(0))))
        };
        let v = self.add(
            b,
            Expression::ImageLoad {
                image,
                coordinate,
                array_index,
                sample,
                level,
            },
        );
        if res.depth {
            return self.depth_vec4(b, v, op);
        }
        if res.storage {
            // A view of another kind than the declaration's is read by its bits, as Direct3D does.
            let view = self.storage_scalar(res.var)?;
            let member = op
                .ty
                .basic()
                .ok_or_else(|| malformed(LOAD_NOT_VECTOR.into()))?;
            let declared = Lowering::scalar(member.scalar)?;
            return Ok(self.e.bitcast(&mut self.f, b, v, view, declared));
        }
        Ok(v)
    }

    /// The scalar of a storage texture's format.
    fn storage_scalar(&self, var: Handle<GlobalVariable>) -> Result<Scalar> {
        match self.l.module.types[self.l.module.global_variables[var].ty].inner {
            TypeInner::Image {
                class: ImageClass::Storage { format, .. },
                ..
            } => Ok(Scalar {
                kind: storage_kind(format),
                ..Scalar::U32
            }),
            _ => Err(malformed(NOT_STORAGE_VIEW.into())),
        }
    }

    /// A store to a storage texture: the value made the four components of the format's kind, by
    /// its bits.
    fn image_store(&mut self, b: &mut Block, op: &Op) -> Result<()> {
        let r = &op.refs;
        let res = self.image(r[0])?;
        let (k, _) = res.image.unwrap();
        self.l.side_effects = true;
        let image = self.add(b, Expression::GlobalVariable(res.var));
        let array_index = self.layer(b, r[1])?;
        let coordinate = self.coords(b, k, r[2], false)?;
        let s = self.storage_scalar(res.var)?;
        let v = self.get_as(b, r[3], s)?;
        let n = self.basic_of(r[3])?.size;
        let value = if n == 4 {
            v
        } else {
            let mut components = Vec::new();
            for i in 0..4u32 {
                components.push(if i >= u32::from(n) {
                    self.lit(b, zero(s))
                } else if n == 1 {
                    v
                } else {
                    self.add(b, Expression::AccessIndex { base: v, index: i })
                });
            }
            let ty = self.l.vec_ty(4, s)?;
            self.add(b, Expression::Compose { ty, components })
        };
        self.stmt(
            b,
            Statement::ImageStore {
                image,
                coordinate,
                array_index,
                value,
            },
        );
        Ok(())
    }

    /// Whether instruction `id` loads a hull program's tessellation factor output.
    fn loads_tessellation_factor(&self, id: u32) -> bool {
        let Ok(op) = self.l.ir.op(id) else {
            return false;
        };
        op.code == code::OUTPUT_LOAD
            && op
                .refs
                .first()
                .and_then(|dcl| self.l.io_index.get(dcl))
                .is_some_and(|&i| {
                    matches!(
                        self.l.io[i].1.builtin,
                        Some(builtin::TESS_FACTOR_INNER | builtin::TESS_FACTOR_OUTER)
                    )
                })
    }

    fn arithmetic(&mut self, b: &mut Block, op: &Op) -> Result<Handle<Expression>> {
        use BinaryOperator as B;
        use MathFunction as M;
        use code as c;
        let r = &op.refs;
        let res = op
            .ty
            .basic()
            .ok_or_else(|| malformed(format!("opcode {} result", op.code)))?;
        let rs = Lowering::scalar(res.scalar)?;
        let bin = |s: &mut Self, b: &mut Block, o: B, ty: Scalar| -> Result<Handle<Expression>> {
            let x = s.get_as(b, r[0], ty)?;
            let y = s.get_as(b, r[1], ty)?;
            Ok(s.bin(b, o, x, y))
        };
        let math = |s: &mut Self, b: &mut Block, f: M, ty: Scalar| -> Result<Handle<Expression>> {
            let mut args = Vec::new();
            for &a in r.iter() {
                args.push(s.get_as(b, a, ty)?);
            }
            Ok(s.math(b, f, &args))
        };
        let operand = |s: &Self| s.scalar_of(r[0]);
        if level_zero_check(self.l.ir, op) {
            return Ok(self.lit(b, Literal::Bool(true)));
        }
        Ok(match op.code {
            c::F_EQ => bin(self, b, B::Equal, Scalar::F32)?,
            c::F_NE => {
                // Direct3D's `ne` is unordered (true where an operand is NaN): the negated ordered
                // equality is so on every back end, where `!=` is ordered in SPIR-V.
                let equal = bin(self, b, B::Equal, Scalar::F32)?;
                self.add(
                    b,
                    Expression::Unary {
                        op: UnaryOperator::LogicalNot,
                        expr: equal,
                    },
                )
            }
            c::F_LT => bin(self, b, B::Less, Scalar::F32)?,
            c::F_LE => bin(self, b, B::LessEqual, Scalar::F32)?,
            c::F_GT => bin(self, b, B::Greater, Scalar::F32)?,
            c::F_GE => bin(self, b, B::GreaterEqual, Scalar::F32)?,
            c::F_IS_NAN => {
                // A bit test: WGSL has no isnan, and the result never depends on fast-math rules.
                let x = self.get_as(b, r[0], Scalar::U32)?;
                let m = self.lit(b, Literal::U32(!crate::build::F32_SIGN_MASK));
                let x = self.bin(b, B::And, x, m);
                let inf = self.lit(b, Literal::U32(crate::build::F32_EXPONENT_MASK));
                self.bin(b, B::Greater, x, inf)
            }
            c::I_EQ => bin(self, b, B::Equal, operand(self)?)?,
            c::I_NE => bin(self, b, B::NotEqual, operand(self)?)?,
            c::S_LT => bin(self, b, B::Less, Scalar::I32)?,
            c::S_LE => bin(self, b, B::LessEqual, Scalar::I32)?,
            c::S_GT => bin(self, b, B::Greater, Scalar::I32)?,
            c::S_GE => bin(self, b, B::GreaterEqual, Scalar::I32)?,
            c::U_LT => bin(self, b, B::Less, Scalar::U32)?,
            c::U_LE => bin(self, b, B::LessEqual, Scalar::U32)?,
            c::U_GT => bin(self, b, B::Greater, Scalar::U32)?,
            c::U_GE => bin(self, b, B::GreaterEqual, Scalar::U32)?,
            c::B_AND => bin(self, b, B::LogicalAnd, Scalar::BOOL)?,
            c::B_OR => bin(self, b, B::LogicalOr, Scalar::BOOL)?,
            c::B_EQ => bin(self, b, B::Equal, Scalar::BOOL)?,
            c::B_NE => bin(self, b, B::NotEqual, Scalar::BOOL)?,
            c::B_NOT => {
                let x = self.get_as(b, r[0], Scalar::BOOL)?;
                self.add(
                    b,
                    Expression::Unary {
                        op: UnaryOperator::LogicalNot,
                        expr: x,
                    },
                )
            }
            c::SELECT => {
                let condition = self.get_as(b, r[0], Scalar::BOOL)?;
                let accept = self.get_as(b, r[1], rs)?;
                let reject = self.get_as(b, r[2], rs)?;
                self.add(
                    b,
                    Expression::Select {
                        condition,
                        accept,
                        reject,
                    },
                )
            }
            c::F_ABS => math(self, b, M::Abs, rs)?,
            c::F_NEG | c::I_NEG => {
                // naga negates only floats and signed integers; two's complement makes it one.
                let ty = if rs.kind == ScalarKind::Uint {
                    Scalar {
                        kind: ScalarKind::Sint,
                        ..rs
                    }
                } else {
                    rs
                };
                let x = self.get_as(b, r[0], ty)?;
                let v = self.add(
                    b,
                    Expression::Unary {
                        op: UnaryOperator::Negate,
                        expr: x,
                    },
                );
                self.e.bitcast(&mut self.f, b, v, ty, rs)
            }
            c::F_ADD | c::I_ADD => bin(self, b, B::Add, rs)?,
            c::F_SUB | c::I_SUB => bin(self, b, B::Subtract, rs)?,
            c::F_MUL | c::I_MUL => bin(self, b, B::Multiply, rs)?,
            c::F_DIV if rs == Scalar::F32 => {
                // Direct3D's `div` as WARP gives it: the device's quotient refined once by its
                // fused remainder, kept where it or the refined one is zero, infinite or NaN.
                // Correctly rounded except where the remainder is below the normal range and
                // flushed (quotients just above the range's bottom), which keeps the device's.
                let a = self.get_as(b, r[0], rs)?;
                let d = self.get_as(b, r[1], rs)?;
                let q = self.bin(b, B::Divide, a, d);
                let negated = self.add(
                    b,
                    Expression::Unary {
                        op: UnaryOperator::Negate,
                        expr: d,
                    },
                );
                let remainder = self.math(b, M::Fma, &[negated, q, a]);
                let one = self.splat_lit(b, Literal::F32(1.0), res.size)?;
                let reciprocal = self.bin(b, B::Divide, one, d);
                let refined = self.math(b, M::Fma, &[remainder, reciprocal, q]);
                let size = self.math(b, M::Abs, &[refined]);
                let largest = self.splat_lit(b, Literal::F32(f32::MAX), res.size)?;
                let finite = self.bin(b, B::LessEqual, size, largest);
                let zero = self.splat_lit(b, Literal::F32(0.0), res.size)?;
                let nonzero = self.bin(b, B::NotEqual, q, zero);
                let kept = self.add(
                    b,
                    Expression::Select {
                        condition: finite,
                        accept: refined,
                        reject: q,
                    },
                );
                self.add(
                    b,
                    Expression::Select {
                        condition: nonzero,
                        accept: kept,
                        reject: q,
                    },
                )
            }
            c::F_DIV | c::U_DIV => bin(self, b, B::Divide, rs)?,
            c::U_MOD => bin(self, b, B::Modulo, rs)?,
            // Direct3D's `mad` may be fused; `fma` leaves the device the same choice it has for it.
            c::F_MAD => math(self, b, M::Fma, rs)?,
            c::F_RCP => {
                let x = self.get_as(b, r[0], rs)?;
                let one = self.splat_lit(b, Literal::F32(1.0), res.size)?;
                self.bin(b, B::Divide, one, x)
            }
            c::F_SQRT => math(self, b, M::Sqrt, rs)?,
            c::F_RSQ => math(self, b, M::InverseSqrt, rs)?,
            c::F_EXP_2 => math(self, b, M::Exp2, rs)?,
            c::F_LOG_2 => math(self, b, M::Log2, rs)?,
            c::F_FRACT => math(self, b, M::Fract, rs)?,
            c::F_ROUND => {
                let f = match op.lits.first() {
                    Some(&round::ZERO) => M::Trunc,
                    Some(&round::NEGATIVE_INF) => M::Floor,
                    Some(&round::POSITIVE_INF) => M::Ceil,
                    _ => M::Round,
                };
                math(self, b, f, rs)?
            }
            c::F_MIN | c::S_MIN | c::U_MIN => math(self, b, M::Min, rs)?,
            c::F_MAX | c::S_MAX | c::U_MAX => math(self, b, M::Max, rs)?,
            // dxbc-spirv clamps a hull program's tessellation factors to the declared largest. A
            // subnormal factor keeps its bits, as Direct3D hands it to the tessellator; the back
            // end's compiled clamp would flush it to zero.
            c::F_CLAMP if self.loads_tessellation_factor(r[0]) => {
                let clamped = math(self, b, M::Clamp, rs)?;
                let factor = self.get_as(b, r[0], rs)?;
                let bits = self.get_as(b, r[0], Scalar::U32)?;
                let mask = self.lit(b, Literal::U32(crate::build::F32_EXPONENT_MASK));
                let exponent = self.bin(b, B::And, bits, mask);
                let zero = self.lit(b, Literal::U32(0));
                let subnormal = self.bin(b, B::Equal, exponent, zero);
                self.add(
                    b,
                    Expression::Select {
                        condition: subnormal,
                        accept: factor,
                        reject: clamped,
                    },
                )
            }
            c::F_CLAMP | c::S_CLAMP | c::U_CLAMP => math(self, b, M::Clamp, rs)?,
            c::F_DOT => math(self, b, M::Dot, rs)?,
            c::F_SIN => math(self, b, M::Sin, rs)?,
            c::F_COS => math(self, b, M::Cos, rs)?,
            c::F_POW => math(self, b, M::Pow, rs)?,
            c::F_SGN => math(self, b, M::Sign, rs)?,
            c::I_ABS => math(self, b, M::Abs, rs)?,
            c::I_AND => bin(self, b, B::And, rs)?,
            c::I_OR => bin(self, b, B::InclusiveOr, rs)?,
            c::I_XOR => bin(self, b, B::ExclusiveOr, rs)?,
            c::I_NOT => {
                let x = self.get_as(b, r[0], rs)?;
                self.add(
                    b,
                    Expression::Unary {
                        op: UnaryOperator::BitwiseNot,
                        expr: x,
                    },
                )
            }
            c::I_SHL | c::S_SHR | c::U_SHR => {
                let ty = match op.code {
                    c::S_SHR => Scalar::I32,
                    c::U_SHR => Scalar::U32,
                    _ => rs,
                };
                let x = self.get_as(b, r[0], ty)?;
                let n = self.get_as(b, r[1], Scalar::U32)?;
                let o = if op.code == c::I_SHL {
                    B::ShiftLeft
                } else {
                    B::ShiftRight
                };
                let v = self.bin(b, o, x, n);
                self.e.bitcast(&mut self.f, b, v, ty, rs)
            }
            c::I_BIT_INSERT => {
                let base = self.get_as(b, r[0], rs)?;
                let insert = self.get_as(b, r[1], rs)?;
                let offset = self.get_as(b, r[2], Scalar::U32)?;
                let count = self.get_as(b, r[3], Scalar::U32)?;
                self.math(b, M::InsertBits, &[base, insert, offset, count])
            }
            c::U_BIT_EXTRACT | c::S_BIT_EXTRACT => {
                let ty = if op.code == c::S_BIT_EXTRACT {
                    Scalar::I32
                } else {
                    Scalar::U32
                };
                let v = self.get_as(b, r[0], ty)?;
                let offset = self.get_as(b, r[1], Scalar::U32)?;
                let count = self.get_as(b, r[2], Scalar::U32)?;
                let x = self.math(b, M::ExtractBits, &[v, offset, count]);
                self.e.bitcast(&mut self.f, b, x, ty, rs)
            }
            c::I_BIT_COUNT => {
                let x = self.get(b, r[0])?;
                let v = self.math(b, M::CountOneBits, &[x]);
                let s = operand(self)?;
                self.e.bitcast(&mut self.f, b, v, s, rs)
            }
            c::I_BIT_REVERSE => math(self, b, M::ReverseBits, rs)?,
            c::I_FIND_LSB | c::S_FIND_MSB | c::U_FIND_MSB => {
                let (f, ty) = match op.code {
                    c::I_FIND_LSB => (M::FirstTrailingBit, operand(self)?),
                    c::S_FIND_MSB => (M::FirstLeadingBit, Scalar::I32),
                    _ => (M::FirstLeadingBit, Scalar::U32),
                };
                let x = self.get_as(b, r[0], ty)?;
                let v = self.math(b, f, &[x]);
                self.e.bitcast(&mut self.f, b, v, ty, rs)
            }
            c::I_ADD_CARRY | c::I_SUB_BORROW => {
                let x = self.get_as(b, r[0], Scalar::U32)?;
                let y = self.get_as(b, r[1], Scalar::U32)?;
                let add = op.code == c::I_ADD_CARRY;
                let lo = self.bin(b, if add { B::Add } else { B::Subtract }, x, y);
                let flag = if add {
                    self.bin(b, B::Less, lo, x)
                } else {
                    self.bin(b, B::Less, x, y)
                };
                let one = self.lit(b, Literal::U32(1));
                let z = self.lit(b, Literal::U32(0));
                let hi = self.add(
                    b,
                    Expression::Select {
                        condition: flag,
                        accept: one,
                        reject: z,
                    },
                );
                let ty = self.l.vec_ty(2, Scalar::U32)?;
                self.add(
                    b,
                    Expression::Compose {
                        ty,
                        components: vec![lo, hi],
                    },
                )
            }
            c::S_MUL_EXTENDED | c::U_MUL_EXTENDED => {
                return Err(unsupported(crate::refusal::MUL_EXTENDED));
            }
            c::CONVERT_FTO_I => {
                let x = self.get_as(b, r[0], Scalar::F32)?;
                self.add(
                    b,
                    Expression::As {
                        expr: x,
                        kind: rs.kind,
                        convert: Some(Scalar::U32.width),
                    },
                )
            }
            c::CONVERT_ITO_F => {
                let x = self.get(b, r[0])?;
                self.add(
                    b,
                    Expression::As {
                        expr: x,
                        kind: ScalarKind::Float,
                        convert: Some(Scalar::U32.width),
                    },
                )
            }
            c::CONVERT_FTO_F | c::CONVERT_ITO_I | c::CAST => {
                let from = operand(self)?;
                let x = self.get(b, r[0])?;
                if from.kind == rs.kind {
                    x
                } else if from.kind == ScalarKind::Bool || rs.kind == ScalarKind::Bool {
                    self.e.bitcast(&mut self.f, b, x, from, rs)
                } else {
                    self.add(
                        b,
                        Expression::As {
                            expr: x,
                            kind: rs.kind,
                            convert: None,
                        },
                    )
                }
            }
            c::CONVERT_F32TO_PACKED_F16 => {
                // Direct3D rounds toward zero, so only infinities convert to infinities: finite
                // values are clamped to the half's range first. Finiteness is a bit test, which
                // no back end's compiler folds away as it may a float comparison with infinity.
                let n = self.basic_of(r[0])?.size;
                let x = self.get_as(b, r[0], Scalar::F32)?;
                let bits = self.get_as(b, r[0], Scalar::U32)?;
                let m = self.splat_lit(b, Literal::U32(!crate::build::F32_SIGN_MASK), n)?;
                let magnitude = self.bin(b, B::And, bits, m);
                let inf = self.splat_lit(b, Literal::U32(crate::build::F32_EXPONENT_MASK), n)?;
                let finite = self.bin(b, B::Less, magnitude, inf);
                let low = self.splat_lit(b, Literal::F32(-crate::build::HALF_MAX), n)?;
                let high = self.splat_lit(b, Literal::F32(crate::build::HALF_MAX), n)?;
                let clamped = self.math(b, M::Clamp, &[x, low, high]);
                let x = self.add(
                    b,
                    Expression::Select {
                        condition: finite,
                        accept: clamped,
                        reject: x,
                    },
                );
                self.math(b, M::Pack2x16float, &[x])
            }
            c::CONVERT_PACKED_F16TO_F32 => math(self, b, M::Unpack2x16float, Scalar::U32)?,
            other => return Err(malformed(format!("opcode {other}"))),
        })
    }

    fn splat_lit(&mut self, b: &mut Block, l: Literal, n: u8) -> Result<Handle<Expression>> {
        let v = self.lit(b, l);
        if n == 1 {
            return Ok(v);
        }
        Ok(self.add(
            b,
            Expression::Splat {
                size: vector_size(n)?,
                value: v,
            },
        ))
    }
}

fn literal(s: Scalar, bits: u64) -> Literal {
    match s.kind {
        ScalarKind::Float => Literal::F32(f32::from_bits(bits as u32)),
        ScalarKind::Sint => Literal::I32(bits as i32),
        ScalarKind::Bool => Literal::Bool(bits != 0),
        _ => Literal::U32(bits as u32),
    }
}

impl Lowering<'_> {
    /// A module constant holding the vector or array constant `id`.
    fn global_constant(&mut self, id: u32) -> Result<Handle<naga::Constant>> {
        let op = self.ir.op(id)?;
        let ty = self.ir_ty(&op.ty)?;
        let init = self.const_expr(&op.ty, &op.lits)?;
        Ok(self.module.constants.append(
            naga::Constant {
                name: None,
                ty,
                init,
            },
            Span::UNDEFINED,
        ))
    }

    fn const_expr(&mut self, t: &Ty, lits: &[u64]) -> Result<Handle<Expression>> {
        let [m] = t.members[..] else {
            return Err(unsupported(crate::refusal::STRUCT_CONSTANT));
        };
        let s = Self::scalar(m.scalar)?;
        let mut elements = Vec::new();
        for chunk in lits.chunks(m.size as usize) {
            let mut comps = Vec::new();
            for &l in chunk {
                if s == Scalar::F32 && !f32::from_bits(l as u32).is_finite() {
                    return Err(unsupported(crate::refusal::NON_FINITE_CONSTANT_ARRAY));
                }
                comps.push(
                    self.module
                        .global_expressions
                        .append(Expression::Literal(literal(s, l)), Span::UNDEFINED),
                );
            }
            let e = if m.size == 1 {
                comps[0]
            } else {
                let ty = self.basic(m)?;
                self.module.global_expressions.append(
                    Expression::Compose {
                        ty,
                        components: comps,
                    },
                    Span::UNDEFINED,
                )
            };
            elements.push(e);
        }
        if t.dims.is_empty() {
            return Ok(elements[0]);
        }
        if t.dims.len() > 1 {
            return Err(unsupported(crate::refusal::NESTED_ARRAY_CONSTANT));
        }
        let ty = self.ir_ty(t)?;
        Ok(self.module.global_expressions.append(
            Expression::Compose {
                ty,
                components: elements,
            },
            Span::UNDEFINED,
        ))
    }

    fn ivec_constant(&mut self, comps: &[i32]) -> Result<Handle<naga::Constant>> {
        let t = Ty {
            dims: Vec::new(),
            members: vec![Basic {
                scalar: scalar::I32,
                size: comps.len() as u8,
            }],
        };
        let lits: Vec<u64> = comps.iter().map(|&c| c as u32 as u64).collect();
        let ty = self.ir_ty(&t)?;
        let init = self.const_expr(&t, &lits)?;
        Ok(self.module.constants.append(
            naga::Constant {
                name: None,
                ty,
                init,
            },
            Span::UNDEFINED,
        ))
    }

    /// A private variable initialised with the constant array `id`, for indexing at run time, and
    /// its type. A table of f32 holding an infinity or NaN, which no constant expression may hold, is
    /// kept as u32 bits, which loads reinterpret.
    fn constant_array(&mut self, id: u32) -> Result<(Handle<GlobalVariable>, Ty)> {
        let op = self.ir.op(id)?;
        let mut kept = op.ty.clone();
        if non_finite(&op.ty, &op.lits) {
            for m in &mut kept.members {
                m.scalar = scalar::U32;
            }
        }
        if let Some(&v) = self.memory.get(&id) {
            return Ok((v, kept));
        }
        let ty = self.ir_ty(&kept)?;
        let init = self.const_expr(&kept, &op.lits)?;
        let var = self.global(
            format!("icb{id}"),
            AddressSpace::Private,
            None,
            ty,
            Some(init),
        );
        self.memory.insert(id, var);
        Ok((var, kept))
    }
}

/// Whether a constant of f32 holds an infinity or NaN.
fn non_finite(ty: &Ty, lits: &[u64]) -> bool {
    ty.members.iter().all(|m| m.scalar == scalar::F32)
        && lits.iter().any(|&l| !f32::from_bits(l as u32).is_finite())
}

/// A resource array whose last register is past the largest a register number holds.
const ARRAY_PAST_LAST_REGISTER: &str = "a resource array past the last register";

/// The element count of resource declaration `op` of `kind`: its own, or for an array declared
/// without a length (a count of zero), the rest of the layout's register range holding its first
/// register; refused past the slots the kind binds.
fn declared_length(variant: &VariantKey, kind: SlotKind, op: &Op) -> Result<u32> {
    let [space, register, count] = [0, 1, 2].map(|i| op.lits.get(i).map_or(0, |&l| l as u32));
    let length = if count != 0 {
        count
    } else {
        variant
            .register_ranges
            .iter()
            .find(|(k, s, range)| *k == kind && *s == space && range.contains(&register))
            .map(|(_, _, range)| range.end - register)
            .ok_or(unsupported(crate::refusal::UNBOUNDED_RESOURCE_ARRAY))?
    };
    if length > SLOT_KINDS[kind as usize].slots {
        return Err(unsupported(crate::refusal::BINDINGS_BEYOND_LIMIT));
    }
    Ok(length)
}

/// The kind of slot a resource declaration takes, if `op` is one.
fn declared_kind(op: &Op) -> Option<SlotKind> {
    Some(match op.code {
        code::DCL_SAMPLER => SlotKind::Sampler,
        code::DCL_CBV => SlotKind::Uniform,
        code::DCL_SRV => SlotKind::Texture,
        code::DCL_UAV => SlotKind::Storage,
        _ => return None,
    })
}

/// A kind's place in the numbering's order; a counter is numbered by its view.
fn rank(kind: SlotKind) -> u32 {
    match kind {
        SlotKind::Counter => SlotKind::Storage as u32,
        k => k as u32,
    }
}

/// The operands of a sample with Direct3D's per-sample clamp that its gradients take (zero where
/// absent).
struct Clamped {
    bias: u32,
    clamp: u32,
    dx: u32,
    dy: u32,
}
