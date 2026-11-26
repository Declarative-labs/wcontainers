//! Vertex capture (`TRANSFORM_FEEDBACK`): a draw whose vertex program also writes its outputs into buffers.
//!
//! Each is drawn as a list: its vertex numbers in list order, its per-vertex streams gathered into that order, and
//! its vertex program rewritten to store its outputs at each vertex's place; one that does not rasterise draws
//! points in an empty pass.

use std::collections::HashMap;
use std::rc::Rc;

use naga::{
    AddressSpace, ArraySize, BinaryOperator, Binding, BuiltIn, Expression, Handle, Literal,
    ResourceBinding, Scalar, ScalarKind, Span, Statement, StorageAccess, Type, TypeInner,
    VectorSize,
};
use wcontainers_gpu::{
    Gpu, MAX_BINDING_SETS,
    draw::{CaptureTargets, DrawCall, DrawRange, EmptyPass, PassDesc, VertexBinding, Viewport},
    error::GpuError,
    resource::{BufferDesc, BufferUsage, MapIntent, WriteMode},
    shader::{ShaderModule, ShaderStage},
    state::{
        DepthStencilDesc, INDEX_FORMATS, ProvokingVertex, StepMode, Topology, VertexAttribute,
        VertexFormat, VertexLayoutDesc, VertexStream,
    },
};
use wcontainers_metal::{Metal, RenderCommand};

use crate::objects::{
    BufferObj, DepthStencilBlock, ProgramObj, SetResource, ShaderObj, Shared, SlotKind,
    VertexLayoutBlock,
};
use crate::{MetalGpu, VERTEX_STREAMS, place};

/// The bind group of a capture variant's own buffers: one past the last group a draw binds, so no binding set meets it.
const CAPTURE_GROUP: u32 = MAX_BINDING_SETS as u32;

/// The most targets of one capture, as Direct3D's stream output takes.
///
/// https://learn.microsoft.com/en-us/windows/win32/direct3d11/d3d10-graphics-programming-guide-output-stream-stage
pub(crate) const CAPTURE_BUFFERS: u32 = 4;

/// The most 32-bit components a capture writes per vertex into one target, as Direct3D's stream output takes.
///
/// https://learn.microsoft.com/en-us/windows/win32/direct3d11/d3d10-graphics-programming-guide-output-stream-stage
pub(crate) const CAPTURE_COMPONENTS: u32 = 128;

/// The binding in `CAPTURE_GROUP` of the words a capture variant reads its place from: after the targets'.
const INFO_BINDING: u32 = CAPTURE_BUFFERS;

/// The bytes of one captured 32-bit component, which the variant writes a word at a time.
const COMPONENT_BYTES: u32 = u32::BITS / u8::BITS;

/// The words of the info a capture variant reads as one `vec4<u32>`: the first instance, the list's length, two unread.
///
/// https://www.w3.org/TR/WGSL/#vector-types
const INFO_WORDS: usize = 4;

/// The components of a scalar output, against a vector's size.
///
/// https://www.w3.org/TR/WGSL/#scalar-types
const SCALAR_COMPONENTS: u32 = 1;

/// The corners of a line, which a list of lines takes two vertices at a time.
///
/// https://developer.apple.com/documentation/metal/mtlprimitivetype/line
const LINE_CORNERS: usize = 2;

/// The corners of a triangle, which a list of triangles takes three vertices at a time.
///
/// https://developer.apple.com/documentation/metal/mtlprimitivetype/triangle
const TRIANGLE_CORNERS: usize = 3;

/// The side of the empty pass a capture that does not rasterise draws its points in.
///
/// https://developer.apple.com/documentation/metal/mtlrenderpassdescriptor/rendertargetwidth
const ONE_PIXEL: u32 = 1;

/// The far end of the depth range of that pass's viewport.
///
/// https://developer.apple.com/documentation/metal/mtlviewport/zfar
const FAR_DEPTH: f32 = 1.0;

/// The words of the info a capture variant reads, in their order in its `vec4<u32>`.
#[derive(Clone, Copy)]
enum InfoWord {
    /// The draw's first instance, which the variant's places count from.
    FirstInstance,
    /// The vertices of each instance in list order.
    PerInstance,
}

/// One location output of a vertex program, as a capture writes it.
#[derive(Clone, Copy, Debug)]
struct Output {
    /// The member of the program's result that holds it, or none where the result is the output.
    member: Option<u32>,
    location: u32,
    /// Its 32-bit components: one for a scalar, the size of a vector.
    components: u32,
    kind: ScalarKind,
    vector: bool,
}

/// A program rewritten to capture, and the input location of its vertex numbers if it reads `vertex_index`.
pub(crate) struct Variant {
    program: Shared<ProgramObj>,
    numbers_location: Option<u32>,
}

/// What captures keep between calls: the rewritten programs and the layouts with a stream of numbers.
#[derive(Default)]
pub(crate) struct Captures {
    /// By the program's number, the number of targets and whether the capture rasterises.
    variants: HashMap<(u64, u64, usize, bool), Rc<Variant>>,
    layouts: HashMap<VertexLayoutDesc, Shared<VertexLayoutBlock>>,
    no_tests: Option<Shared<DepthStencilBlock>>,
}

/// Returns the location outputs of a vertex entry point in location order, each a 32-bit scalar or vector.
fn outputs(shader: &ShaderObj) -> Result<Vec<Output>, GpuError> {
    let ep = entry_point(&shader.module, &shader.entry_point).ok_or(GpuError::Invalid(
        "a capture of a program with no vertex entry point",
    ))?;
    let result = ep.function.result.as_ref().ok_or(GpuError::Invalid(
        "a capture of a program with no location outputs",
    ))?;
    let types = &shader.module.types;
    let one = |member: Option<u32>, location: u32, ty: Handle<Type>| {
        let (components, scalar, vector) = match types[ty].inner {
            TypeInner::Scalar(scalar) => (SCALAR_COMPONENTS, scalar, false),
            TypeInner::Vector { size, scalar } => (size as u32, scalar, true),
            _ => {
                return Err(GpuError::Invalid(
                    "a captured output that is not a scalar or vector",
                ));
            }
        };
        if u32::from(scalar.width) != COMPONENT_BYTES
            || !matches!(
                scalar.kind,
                ScalarKind::Float | ScalarKind::Sint | ScalarKind::Uint
            )
        {
            return Err(GpuError::Invalid(
                "a captured output whose components are not single words",
            ));
        }
        Ok(Output {
            member,
            location,
            components,
            kind: scalar.kind,
            vector,
        })
    };
    let mut outputs = Vec::new();
    match (&result.binding, &types[result.ty].inner) {
        (Some(Binding::Location { location, .. }), _) => {
            outputs.push(one(None, *location, result.ty)?)
        }
        (None, TypeInner::Struct { members, .. }) => {
            for (n, member) in members.iter().enumerate() {
                if let Some(Binding::Location { location, .. }) = member.binding {
                    let n = u32::try_from(n)
                        .map_err(|_| GpuError::Invalid("a result of too many members"))?;
                    outputs.push(one(Some(n), location, member.ty)?);
                }
            }
        }
        _ => {}
    }
    if outputs.is_empty() {
        return Err(GpuError::Invalid(
            "a capture of a program with no location outputs",
        ));
    }
    outputs.sort_by_key(|o| o.location);
    Ok(outputs)
}

/// Returns a module's vertex entry point of `name`.
fn entry_point<'a>(module: &'a naga::Module, name: &str) -> Option<&'a naga::EntryPoint> {
    module
        .entry_points
        .iter()
        .find(|e| e.name == name && e.stage == naga::ShaderStage::Vertex)
}

/// Where a vertex entry point reads its instance index.
#[derive(Clone, Copy)]
enum Instance {
    Argument(u32),
    Member(u32, u32),
}

/// What the stores at each return of a capture variant need.
struct Stores<'a> {
    outputs: &'a [Output],
    targets: &'a [Handle<naga::GlobalVariable>],
    info: Handle<naga::GlobalVariable>,
    vertex: u32,
    instance: Instance,
}

impl Stores<'_> {
    /// Returns the statements that store the outputs of the result `value` at its vertex's place in the targets.
    fn before_return(
        &self,
        function: &mut naga::Function,
        value: Handle<Expression>,
    ) -> Result<naga::Block, &'static str> {
        let separate = self
            .targets
            .split_first()
            .is_some_and(|(_, rest)| !rest.is_empty());
        let total: u32 = self.outputs.iter().map(|o| o.components).sum();
        let span = Span::UNDEFINED;
        // Expressions that need no emitting first: arguments, globals and the literals.
        let mut pre = |e: Expression| function.expressions.append(e, span);
        let vertex = pre(Expression::FunctionArgument(self.vertex));
        let instance_argument = pre(Expression::FunctionArgument(match self.instance {
            Instance::Argument(a) | Instance::Member(a, _) => a,
        }));
        let info = pre(Expression::GlobalVariable(self.info));
        let targets: Vec<_> = self
            .targets
            .iter()
            .map(|t| pre(Expression::GlobalVariable(*t)))
            .collect();
        let mut literals = HashMap::new();
        let mut words = Vec::new();
        let mut offset = u32::MIN;
        for (n, output) in self.outputs.iter().enumerate() {
            let (stride, first) = if separate {
                (output.components, u32::MIN)
            } else {
                (total, offset)
            };
            for j in 0..output.components {
                let word = first
                    .checked_add(j)
                    .ok_or("an output past the words a capture writes")?;
                for number in [stride, word] {
                    literals
                        .entry(number)
                        .or_insert_with(|| pre(Expression::Literal(Literal::U32(number))));
                }
                words.push((n, j, stride, word));
            }
            offset = offset
                .checked_add(output.components)
                .ok_or("outputs past the words a capture writes")?;
        }
        let start = function.expressions.len();
        let mut emit = |e: Expression| function.expressions.append(e, span);
        let instance = match self.instance {
            Instance::Argument(_) => instance_argument,
            Instance::Member(_, m) => emit(Expression::AccessIndex {
                base: instance_argument,
                index: m,
            }),
        };
        let info = emit(Expression::Load { pointer: info });
        let first_instance = emit(Expression::AccessIndex {
            base: info,
            index: InfoWord::FirstInstance as u32,
        });
        let per_instance = emit(Expression::AccessIndex {
            base: info,
            index: InfoWord::PerInstance as u32,
        });
        let binary = |emit: &mut dyn FnMut(Expression) -> Handle<Expression>, op, left, right| {
            emit(Expression::Binary { op, left, right })
        };
        let after_first = binary(
            &mut emit,
            BinaryOperator::Subtract,
            instance,
            first_instance,
        );
        let before = binary(
            &mut emit,
            BinaryOperator::Multiply,
            after_first,
            per_instance,
        );
        let place = binary(&mut emit, BinaryOperator::Add, before, vertex);
        let mut stores = Vec::new();
        for (n, j, stride, word) in words {
            let output = self.outputs[n];
            let whole = match output.member {
                Some(m) => emit(Expression::AccessIndex {
                    base: value,
                    index: m,
                }),
                None => value,
            };
            let component = if output.vector {
                emit(Expression::AccessIndex {
                    base: whole,
                    index: j,
                })
            } else {
                whole
            };
            let bits = if output.kind == ScalarKind::Uint {
                component
            } else {
                emit(Expression::As {
                    expr: component,
                    kind: ScalarKind::Uint,
                    convert: None,
                })
            };
            let at = binary(
                &mut emit,
                BinaryOperator::Multiply,
                place,
                literals[&stride],
            );
            let at = binary(&mut emit, BinaryOperator::Add, at, literals[&word]);
            let target = targets[if separate { n } else { usize::MIN }];
            let pointer = emit(Expression::Access {
                base: target,
                index: at,
            });
            stores.push(Statement::Store {
                pointer,
                value: bits,
            });
        }
        let mut block = naga::Block::new();
        block.push(
            Statement::Emit(function.expressions.range_from(start)),
            span,
        );
        for store in stores {
            block.push(store, span);
        }
        Ok(block)
    }

    /// Puts the stores before every return of `block` that returns a value, in nested blocks too.
    fn stored_through(
        &self,
        function: &mut naga::Function,
        block: naga::Block,
    ) -> Result<naga::Block, &'static str> {
        let mut out = naga::Block::with_capacity(block.len());
        for (statement, span) in block.span_into_iter() {
            let statement = match statement {
                Statement::Return { value: Some(value) } => {
                    out.extend_block(self.before_return(function, value)?);
                    Statement::Return { value: Some(value) }
                }
                Statement::Block(inner) => Statement::Block(self.stored_through(function, inner)?),
                Statement::If {
                    condition,
                    accept,
                    reject,
                } => Statement::If {
                    condition,
                    accept: self.stored_through(function, accept)?,
                    reject: self.stored_through(function, reject)?,
                },
                Statement::Switch { selector, cases } => Statement::Switch {
                    selector,
                    cases: cases
                        .into_iter()
                        .map(|case| {
                            Ok(naga::SwitchCase {
                                body: self.stored_through(function, case.body)?,
                                ..case
                            })
                        })
                        .collect::<Result<_, &'static str>>()?,
                },
                Statement::Loop {
                    body,
                    continuing,
                    break_if,
                } => Statement::Loop {
                    body: self.stored_through(function, body)?,
                    continuing: self.stored_through(function, continuing)?,
                    break_if,
                },
                other => other,
            };
            out.push(statement, span);
        }
        Ok(out)
    }
}

/// Returns `module` with its vertex entry point `name` rewritten to store its outputs into `targets` targets.
///
/// It also returns the input location its `vertex_index` was moved to, if it reads one; `free` is a location no input
/// of it or of its layout takes.
fn capture_module(
    module: &naga::Module,
    name: &str,
    outputs: &[Output],
    targets: usize,
    free: u32,
) -> Result<(naga::Module, Option<u32>), &'static str> {
    let mut module = module.clone();
    let span = Span::UNDEFINED;
    let index = module
        .entry_points
        .iter()
        .position(|e| e.name == name && e.stage == naga::ShaderStage::Vertex)
        .ok_or("a capture of a program with no vertex entry point")?;
    let word = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Scalar(Scalar::U32),
        },
        span,
    );
    let words = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Array {
                base: word,
                size: ArraySize::Dynamic,
                stride: COMPONENT_BYTES,
            },
        },
        span,
    );
    let quad = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Vector {
                size: VectorSize::Quad,
                scalar: Scalar::U32,
            },
        },
        span,
    );
    let mut target_globals = Vec::new();
    for n in 0..targets {
        let binding = u32::try_from(n).map_err(|_| "more targets than a capture takes")?;
        target_globals.push(module.global_variables.append(
            naga::GlobalVariable {
                name: None,
                space: AddressSpace::Storage {
                    access: StorageAccess::LOAD | StorageAccess::STORE,
                },
                binding: Some(ResourceBinding {
                    group: CAPTURE_GROUP,
                    binding,
                }),
                ty: words,
                init: None,
                memory_decorations: Default::default(),
            },
            span,
        ));
    }
    let info = module.global_variables.append(
        naga::GlobalVariable {
            name: None,
            space: AddressSpace::Uniform,
            binding: Some(ResourceBinding {
                group: CAPTURE_GROUP,
                binding: INFO_BINDING,
            }),
            ty: quad,
            init: None,
            memory_decorations: Default::default(),
        },
        span,
    );
    let numbers = Binding::Location {
        location: free,
        interpolation: None,
        sampling: None,
        blend_src: None,
        per_primitive: false,
    };
    // The vertex index moves to a location the stream of numbers feeds; the instance index is found or added.
    let mut moved = false;
    let mut instance = None;
    let arguments = module.entry_points[index].function.arguments.clone();
    for (a, argument) in arguments.iter().enumerate() {
        let a = u32::try_from(a).map_err(|_| "too many arguments")?;
        match argument.binding {
            Some(Binding::BuiltIn(BuiltIn::VertexIndex)) => {
                module.entry_points[index].function.arguments[a as usize].binding =
                    Some(numbers.clone());
                moved = true;
            }
            Some(Binding::BuiltIn(BuiltIn::InstanceIndex)) => {
                instance = Some(Instance::Argument(a))
            }
            None => {
                let mut ty = module.types[argument.ty].clone();
                let TypeInner::Struct { members, .. } = &mut ty.inner else {
                    continue;
                };
                let mut changed = false;
                for (m, member) in members.iter_mut().enumerate() {
                    match member.binding {
                        Some(Binding::BuiltIn(BuiltIn::VertexIndex)) => {
                            member.binding = Some(numbers.clone());
                            changed = true;
                        }
                        Some(Binding::BuiltIn(BuiltIn::InstanceIndex)) => {
                            let m = u32::try_from(m).map_err(|_| "too many members")?;
                            instance = Some(Instance::Member(a, m));
                        }
                        _ => {}
                    }
                }
                if changed {
                    let ty = module.types.insert(ty, span);
                    module.entry_points[index].function.arguments[a as usize].ty = ty;
                    moved = true;
                }
            }
            _ => {}
        }
    }
    let function = &mut module.entry_points[index].function;
    let mut argument = |built_in| {
        let index = u32::try_from(function.arguments.len()).map_err(|_| "too many arguments");
        function.arguments.push(naga::FunctionArgument {
            name: None,
            ty: word,
            binding: Some(Binding::BuiltIn(built_in)),
        });
        index
    };
    let vertex = argument(BuiltIn::VertexIndex)?;
    let instance = match instance {
        Some(instance) => instance,
        None => Instance::Argument(argument(BuiltIn::InstanceIndex)?),
    };
    let stores = Stores {
        outputs,
        targets: &target_globals,
        info,
        vertex,
        instance,
    };
    let body = std::mem::take(&mut function.body);
    function.body = stores.stored_through(function, body)?;
    Ok((module, moved.then_some(free)))
}

/// Returns the vertex numbers of the whole primitives of `numbers` in list order, strips as the lists they draw.
///
/// A restart (none) ends a piece; more than `most` vertices are refused before more are held.
fn list_order(
    topology: Topology,
    provoking: ProvokingVertex,
    numbers: impl Iterator<Item = Option<u32>>,
    most: usize,
) -> Result<Vec<u32>, GpuError> {
    if matches!(topology, Topology::TriangleFan | Topology::LineLoop) {
        return Err(GpuError::Absent(
            wcontainers_gpu::caps::Features::FAN_AND_LOOP,
        ));
    }
    let mut out = Vec::new();
    // The two vertices before this one in its piece, and how many the piece has had.
    let (mut a, mut b, mut n) = (u32::MIN, u32::MIN, usize::MIN);
    for v in numbers {
        let Some(v) = v else {
            n = usize::MIN;
            continue;
        };
        n = n.saturating_add(1);
        match topology {
            Topology::PointList => out.push(v),
            Topology::LineList if n.is_multiple_of(LINE_CORNERS) => out.extend([b, v]),
            Topology::TriangleList if n.is_multiple_of(TRIANGLE_CORNERS) => out.extend([a, b, v]),
            Topology::LineStrip if n >= LINE_CORNERS => out.extend([b, v]),
            // Triangle n - 3 of the strip: odd ones keep their winding and their provoking vertex.
            Topology::TriangleStrip if n >= TRIANGLE_CORNERS => {
                out.extend(match (n.is_multiple_of(LINE_CORNERS), provoking) {
                    (false, _) => [a, b, v],
                    (true, ProvokingVertex::First) => [a, v, b],
                    (true, ProvokingVertex::Last) => [b, a, v],
                })
            }
            _ => {}
        }
        (a, b) = (b, v);
        if out.len() > most {
            return Err(GpuError::Invalid(
                "capture targets that do not hold every vertex the draw makes",
            ));
        }
    }
    Ok(out)
}

/// Returns the list topology that draws the primitives of `topology`.
fn as_list(topology: Topology) -> Topology {
    match topology {
        Topology::LineStrip => Topology::LineList,
        Topology::TriangleStrip => Topology::TriangleList,
        other => other,
    }
}

impl<M: Metal> MetalGpu<M> {
    /// Returns the bytes `size` long from `offset` of a buffer, after the work that uses it.
    ///
    /// That work is committed first if it is being recorded, with the open pass ended for it and resumed after.
    fn bytes_of(
        &mut self,
        buffer: &Shared<BufferObj<M>>,
        offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, GpuError> {
        let mut bytes = vec![u8::MIN; usize::try_from(size).map_err(|_| GpuError::OutOfMemory)?];
        self.read_after_use(buffer, offset, &mut bytes, place::CAPTURE_READS)?;
        Ok(bytes)
    }

    /// Returns the vertex numbers of a draw's whole primitives in list order, refused past `most`.
    fn list_of(&mut self, call: &DrawCall<'_, Self>, most: usize) -> Result<Vec<u32>, GpuError> {
        let provoking = call.raster.desc.provoking;
        match call.range {
            DrawRange::Vertices { first, count, .. } => {
                let end = first
                    .checked_add(count)
                    .ok_or(GpuError::Invalid("a draw's range past the largest vertex"))?;
                list_order(call.topology, provoking, (first..end).map(Some), most)
            }
            DrawRange::Indexed {
                first,
                count,
                base_vertex,
                ..
            } => {
                let index = call
                    .index
                    .as_ref()
                    .ok_or(GpuError::Invalid("an indexed draw with no index buffer"))?;
                let facts = INDEX_FORMATS[index.format as usize];
                let size = u64::from(facts.size);
                let offset = u64::from(first)
                    .checked_mul(size)
                    .and_then(|bytes| index.offset.checked_add(bytes))
                    .ok_or(GpuError::Invalid("indices whose offset overflows"))?;
                let length = u64::from(count)
                    .checked_mul(size)
                    .ok_or(GpuError::Invalid("indices whose length overflows"))?;
                let bytes = self.bytes_of(index.buffer, offset, length)?;
                let width = usize::try_from(facts.size)
                    .map_err(|_| GpuError::Invalid("an index wider than memory"))?;
                let numbers = bytes.chunks_exact(width).map(|c| {
                    let mut word = [u8::MIN; size_of::<u32>()];
                    word[..c.len()].copy_from_slice(c);
                    let value = u32::from_le_bytes(word);
                    // The vertex number wraps past the largest `u32`, as the reference's does.
                    (!(call.restart && value == facts.restart_value))
                        .then(|| value.wrapping_add_signed(base_vertex))
                });
                list_order(call.topology, provoking, numbers, most)
            }
            DrawRange::Indirect { .. } | DrawRange::IndexedIndirect { .. } => {
                Err(GpuError::Invalid(
                    "a capture of an indirect draw, whose vertices are not known before it runs",
                ))
            }
        }
    }

    /// Returns the variant of `program` that captures into `targets` targets, made at its first use.
    fn capture_variant(
        &mut self,
        program: &Shared<ProgramObj>,
        layout: &VertexLayoutBlock,
        outputs: &[Output],
        targets: usize,
        rasterize: bool,
    ) -> Result<Rc<Variant>, GpuError> {
        let ProgramObj::Render {
            id,
            vertex,
            fragment,
        } = &*program.0
        else {
            return Err(GpuError::Invalid("a capture of a compute program"));
        };
        let key = (*id, layout.id, targets, rasterize);
        if let Some(variant) = self.captures.variants.get(&key) {
            return Ok(variant.clone());
        }
        let ep = entry_point(&vertex.module, &vertex.entry_point).ok_or(GpuError::Invalid(
            "a capture of a program with no vertex entry point",
        ))?;
        // A location past every input of the program and of the layout.
        let mut highest = None;
        let mut take = |binding: &Option<Binding>| {
            if let Some(Binding::Location { location, .. }) = binding {
                highest = highest.max(Some(*location));
            }
        };
        for argument in &ep.function.arguments {
            take(&argument.binding);
            if let TypeInner::Struct { members, .. } = &vertex.module.types[argument.ty].inner {
                members.iter().for_each(|m| take(&m.binding));
            }
        }
        for attribute in layout.desc.attributes.iter().flatten() {
            take(&Some(Binding::Location {
                location: attribute.location,
                interpolation: None,
                sampling: None,
                blend_src: None,
                per_primitive: false,
            }));
        }
        let free = highest
            .map_or(Some(u32::MIN), |h| h.checked_add(SCALAR_COMPONENTS))
            .ok_or(GpuError::Invalid("no input location left"))?;
        let (module, numbers_location) =
            capture_module(&vertex.module, &vertex.entry_point, outputs, targets, free)
                .map_err(GpuError::Invalid)?;
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .map_err(|error| {
            eprintln!("metal: a capture variant naga refuses: {error:?}");
            GpuError::Invalid("a capture variant naga refuses")
        })?;
        let rewritten = ShaderModule {
            module,
            info,
            entry_point: vertex.entry_point.clone(),
            stage: ShaderStage::Vertex,
            key: Default::default(),
        };
        let slots = Self::slots(&rewritten)?;
        let shader = Shared(Rc::new(ShaderObj {
            id: self.id(),
            module: rewritten.module,
            info: rewritten.info,
            entry_point: rewritten.entry_point,
            stage: ShaderStage::Vertex,
            slots,
        }));
        let variant = Rc::new(Variant {
            program: Shared(Rc::new(ProgramObj::Render {
                id: self.id(),
                vertex: shader,
                fragment: if rasterize { fragment.clone() } else { None },
            })),
            numbers_location,
        });
        self.captures.variants.insert(key, variant.clone());
        Ok(variant)
    }

    /// Makes a buffer of `bytes`, at least one word long, that draws read streams from.
    fn stream_of(&mut self, bytes: &[u8]) -> Result<Shared<BufferObj<M>>, GpuError> {
        let size = u64::try_from(bytes.len())
            .map_err(|_| GpuError::OutOfMemory)?
            .max(u64::from(COMPONENT_BYTES));
        let buffer = self.create_buffer(&BufferDesc {
            size,
            usage: BufferUsage::VERTEX | BufferUsage::UNIFORM | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })?;
        if !bytes.is_empty() {
            self.write_buffer(&buffer, u64::MIN, bytes, WriteMode::Ordered);
        }
        Ok(buffer)
    }

    /// Draws `call` writing its vertex program's location outputs into `targets`, as `Gpu::capture_vertices` says.
    pub(crate) fn capture(
        &mut self,
        call: &DrawCall<'_, Self>,
        targets: CaptureTargets<'_, Self>,
        rasterize: bool,
    ) -> Result<(), GpuError> {
        if targets.is_empty() || targets.len() > CAPTURE_BUFFERS as usize {
            return Err(GpuError::Invalid(
                "capture targets of none or more than capture_buffers",
            ));
        }
        if rasterize != self.pass.is_some() {
            return Err(GpuError::Invalid(
                "a capture that rasterises outside a pass, or does not inside one",
            ));
        }
        let ProgramObj::Render { vertex, .. } = &*call.program.0 else {
            return Err(GpuError::Invalid("a capture of a compute program"));
        };
        let outputs = outputs(vertex)?;
        let separate = targets
            .split_first()
            .is_some_and(|(_, rest)| !rest.is_empty());
        if separate && targets.len() != outputs.len() {
            return Err(GpuError::Invalid(
                "several capture targets, not one for each output the program writes",
            ));
        }
        let components: u32 = outputs.iter().map(|o| o.components).sum();
        if !separate && components > CAPTURE_COMPONENTS {
            return Err(GpuError::Invalid(
                "a capture into one target of more components than capture_components",
            ));
        }
        // A target the call also reads would be read and written by one draw.
        let reads = |buffer: &Shared<BufferObj<M>>| {
            call.vertex_buffers.iter().flatten().any(|v| Rc::ptr_eq(&v.buffer.0, &buffer.0))
                || call.index.as_ref().is_some_and(|i| Rc::ptr_eq(&i.buffer.0, &buffer.0))
                || call.bindings.iter().flatten().any(|set| {
                    set.entries.iter().any(|(_, r)| {
                        matches!(r, SetResource::Buffer { buffer: b, .. } if Rc::ptr_eq(&b.0, &buffer.0))
                    })
                })
        };
        for t in targets {
            let inside = t
                .offset
                .checked_add(t.size)
                .is_some_and(|end| end <= t.buffer.size);
            if !inside || !t.offset.is_multiple_of(u64::from(COMPONENT_BYTES)) {
                return Err(GpuError::Invalid(
                    "a capture target past its buffer's end or off a word",
                ));
            }
            if !t.buffer.usage.contains(BufferUsage::CAPTURE) {
                return Err(GpuError::Invalid(
                    "a capture target of a buffer made without CAPTURE",
                ));
            }
            if reads(t.buffer) {
                return Err(GpuError::Invalid(
                    "a capture target the call also uses another way",
                ));
            }
        }
        let (first_instance, instances) = match call.range {
            DrawRange::Vertices {
                first_instance,
                instances,
                ..
            }
            | DrawRange::Indexed {
                first_instance,
                instances,
                ..
            } => (first_instance, instances),
            DrawRange::Indirect { .. } | DrawRange::IndexedIndirect { .. } => {
                return Err(GpuError::Invalid(
                    "a capture of an indirect draw, whose vertices are not known before it runs",
                ));
            }
        };
        if instances == u32::MIN {
            return Ok(());
        }
        // The most vertices of one instance every target holds, which bounds the list before it is held.
        let words_of = |n: usize| {
            if separate {
                outputs[n].components
            } else {
                components
            }
        };
        let most = targets
            .iter()
            .enumerate()
            .map(|(n, t)| {
                let bytes = u64::from(words_of(n)).saturating_mul(u64::from(COMPONENT_BYTES));
                t.size.checked_div(bytes).unwrap_or(u64::MAX)
            })
            .min()
            .unwrap_or(u64::MIN)
            .checked_div(u64::from(instances))
            .unwrap_or(u64::MIN);
        let order = self.list_of(call, usize::try_from(most).unwrap_or(usize::MAX))?;
        let per_instance = u32::try_from(order.len())
            .map_err(|_| GpuError::Invalid("a capture of more vertices than a u32 counts"))?;
        let vertices = u64::from(per_instance)
            .checked_mul(u64::from(instances))
            .ok_or(GpuError::Invalid(
                "a capture of more vertices than a u64 counts",
            ))?;
        for (n, t) in targets.iter().enumerate() {
            let needed = vertices
                .checked_mul(u64::from(words_of(n)))
                .and_then(|w| w.checked_mul(u64::from(COMPONENT_BYTES)));
            if needed.is_none_or(|needed| needed > t.size) {
                return Err(GpuError::Invalid(
                    "capture targets that do not hold every vertex the draw makes",
                ));
            }
        }
        if vertices == u64::MIN {
            return Ok(());
        }
        let layout = &call.vertex_layout.desc;
        let variant = self.capture_variant(
            call.program,
            call.vertex_layout,
            &outputs,
            targets.len(),
            rasterize,
        )?;
        // Each stream that advances per vertex, gathered into list order; elements past the buffer read zero, as they do
        // when drawn.
        let most = self.caps.limits.max_buffer_size;
        let mut streams: Vec<Option<Shared<BufferObj<M>>>> = Vec::new();
        for (s, stream) in layout.streams.iter().enumerate() {
            let binding = call.vertex_buffers.get(s).and_then(Option::as_ref);
            let (Some(stream), Some(binding)) = (stream, binding) else {
                streams.push(None);
                continue;
            };
            if stream.stride == u32::MIN || stream.step != StepMode::Vertex {
                streams.push(Some(binding.buffer.clone()));
                continue;
            }
            let stride = u64::from(stream.stride);
            let held = binding
                .buffer
                .size
                .saturating_sub(binding.offset)
                .checked_div(stride)
                .unwrap_or(u64::MIN);
            let read = order
                .iter()
                .max()
                .map_or(u64::MIN, |n| {
                    u64::from(*n).saturating_add(u64::from(SCALAR_COMPONENTS))
                })
                .min(held);
            let source =
                self.bytes_of(binding.buffer, binding.offset, read.saturating_mul(stride))?;
            let length = u64::from(per_instance)
                .checked_mul(stride)
                .filter(|length| *length <= most)
                .ok_or(GpuError::Invalid(
                    "a capture whose streams in list order pass the largest buffer",
                ))?;
            let mut gathered =
                vec![u8::MIN; usize::try_from(length).map_err(|_| GpuError::OutOfMemory)?];
            let width = usize::from(
                u16::try_from(stream.stride)
                    .map_err(|_| GpuError::Invalid("a stride past what a u16 holds"))?,
            );
            for (element, number) in gathered.chunks_exact_mut(width).zip(&order) {
                let at = usize::try_from(*number).unwrap_or(usize::MAX);
                if let Some(bytes) = at
                    .checked_mul(width)
                    .and_then(|start| source.get(start..start.checked_add(width)?))
                {
                    element.copy_from_slice(bytes);
                }
            }
            streams.push(Some(self.stream_of(&gathered)?));
        }
        let mut desc = *layout;
        if let Some(location) = variant.numbers_location {
            let s = (0..VERTEX_STREAMS as usize)
                .find(|s| desc.streams[*s].is_none())
                .ok_or(GpuError::Invalid(
                    "a capture reading its vertex index with every stream taken",
                ))?;
            let a = desc
                .attributes
                .iter()
                .position(Option::is_none)
                .ok_or(GpuError::Invalid(
                    "a capture reading its vertex index with every attribute taken",
                ))?;
            desc.streams[s] = Some(VertexStream {
                stride: COMPONENT_BYTES,
                step: StepMode::Vertex,
            });
            desc.attributes[a] = Some(VertexAttribute {
                location,
                stream: u8::try_from(s)
                    .map_err(|_| GpuError::Invalid("a stream past what a u8 numbers"))?,
                offset: u32::MIN,
                format: VertexFormat::Uint32x1,
            });
            let bytes: Vec<u8> = order.iter().flat_map(|n| n.to_le_bytes()).collect();
            let numbers = self.stream_of(&bytes)?;
            if streams.len() <= s {
                streams.resize(VERTEX_STREAMS as usize, None);
            }
            streams[s] = Some(numbers);
        }
        let captured_layout = match self.captures.layouts.get(&desc) {
            Some(layout) => layout.clone(),
            None => {
                let made = self.create_vertex_layout(&desc)?;
                self.captures.layouts.insert(desc, made.clone());
                made
            }
        };
        let mut words = [u32::MIN; INFO_WORDS];
        words[InfoWord::FirstInstance as usize] = first_instance;
        words[InfoWord::PerInstance as usize] = per_instance;
        let info: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let info = self.stream_of(&info)?;
        let ProgramObj::Render {
            vertex: rewritten, ..
        } = &*variant.program.0
        else {
            return Err(GpuError::Invalid("a capture variant of a compute program"));
        };
        let slot = |binding: u32| match rewritten.slots.get(&(CAPTURE_GROUP, binding)) {
            Some(&(SlotKind::Buffer, index)) => Ok(index),
            _ => Err(GpuError::Invalid(
                "a capture variant past the buffer slots a stage takes",
            )),
        };
        let mut extra = Vec::new();
        for (n, t) in targets.iter().enumerate() {
            self.mark_used(t.buffer);
            extra.push(RenderCommand::SetVertexBuffer {
                buffer: t.buffer.raw.borrow().clone(),
                offset: t.offset,
                length: t.size,
                index: slot(u32::try_from(n).map_err(|_| GpuError::Invalid("too many targets"))?)?,
            });
        }
        self.mark_used(&info);
        extra.push(RenderCommand::SetVertexBuffer {
            buffer: info.raw.borrow().clone(),
            offset: u64::MIN,
            length: u64::from(COMPONENT_BYTES).saturating_mul(INFO_WORDS as u64),
            index: slot(INFO_BINDING)?,
        });
        let bindings: Vec<Option<VertexBinding<'_, Self>>> = streams
            .iter()
            .map(|s| {
                s.as_ref().map(|buffer| VertexBinding {
                    buffer,
                    offset: u64::MIN,
                })
            })
            .collect();
        // Streams kept as they were keep their offsets.
        let bindings: Vec<Option<VertexBinding<'_, Self>>> = bindings
            .into_iter()
            .enumerate()
            .map(|(s, b)| {
                let kept = call.vertex_buffers.get(s).and_then(Option::as_ref);
                match (b, kept) {
                    (Some(b), Some(k)) if Rc::ptr_eq(&b.buffer.0, &k.buffer.0) => {
                        Some(VertexBinding {
                            buffer: k.buffer,
                            offset: k.offset,
                        })
                    }
                    (b, _) => b,
                }
            })
            .collect();
        let no_tests = match &self.captures.no_tests {
            Some(block) => block.clone(),
            None => {
                let block = self.create_depth_stencil(&DepthStencilDesc {
                    depth_compare: None,
                    depth_write: false,
                    stencil: None,
                    stencil_read_mask: [u8::MAX; 2],
                    stencil_write_mask: [u8::MAX; 2],
                    depth_bounds: None,
                })?;
                self.captures.no_tests = Some(block.clone());
                block
            }
        };
        // Points of an empty pass of one pixel: a viewport over it, whatever the call's, and no scissor.
        let one = (ONE_PIXEL as f32).to_bits();
        let over_the_pixel = [Viewport {
            x: u32::MIN,
            y: u32::MIN,
            width: one,
            height: one,
            min_depth: u32::MIN,
            max_depth: FAR_DEPTH.to_bits(),
        }];
        let drawn = DrawCall {
            program: &variant.program,
            viewports: if rasterize {
                call.viewports
            } else {
                &over_the_pixel
            },
            scissors: if rasterize { call.scissors } else { &[] },
            vertex_layout: &captured_layout,
            vertex_buffers: &bindings,
            index: None,
            topology: if rasterize {
                as_list(call.topology)
            } else {
                Topology::PointList
            },
            restart: false,
            depth_stencil: if rasterize {
                call.depth_stencil
            } else {
                &no_tests
            },
            range: DrawRange::Vertices {
                first: u32::MIN,
                count: per_instance,
                first_instance,
                instances,
            },
            ..*call
        };
        if rasterize {
            return self
                .record_draw_with(&drawn, extra)
                .map_err(GpuError::Invalid);
        }
        self.begin_pass(&PassDesc {
            colors: Default::default(),
            depth: None,
            empty: Some(EmptyPass {
                width: ONE_PIXEL,
                height: ONE_PIXEL,
                samples: crate::SINGLE_SAMPLE,
            }),
            occlusion: None,
        });
        let drew = self.record_draw_with(&drawn, extra);
        self.end_pass();
        drew.map_err(GpuError::Invalid)
    }
}
