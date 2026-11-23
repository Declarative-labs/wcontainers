//! DXBC handed to the device: its stage from the container, and its conversion through the shaders
//! crate into the common form.

use wcontainers_gpu::{
    AddedBinding, CLAMPED_SAMPLER_BINDING, DrawBaseSource, Format, FormatKind, HELPER_SET,
    HelperSlot, HelperValue, LinkedValues, Reflection, ShaderModule, ShaderStage, SlotKind,
    SlowPath, SourceStage, StageValue, SystemValue, TexelBufferRoute, VariantFlags, VariantKey,
    ViewComponent, helper_binding,
};
use wcontainers_shaders::container::{
    self, Chunk, Tag, dxbc_dimension, dxbc_op, dxbc_sampled, token,
};
use wcontainers_shaders::{ConvertError, Converted, Source};

use crate::error::{Error, Result, not_implemented};

/// The stage a DXBC container's program is written for, read from the version token of its `SHDR` or
/// `SHEX` chunk; an error if the bytes are not such a container.
pub fn stage(bytecode: &[u8]) -> Result<SourceStage> {
    let bad = Error::InvalidArg("not DXBC bytecode");
    let (_, chunk) = chunks(bytecode)?
        .into_iter()
        .find(|(tag, _)| container::is_program(*tag))
        .ok_or(bad)?;
    match program_version(chunk)? >> 16 {
        0 => Ok(SourceStage::Pixel),
        1 => Ok(SourceStage::Vertex),
        2 => Ok(SourceStage::Geometry),
        3 => Ok(SourceStage::Hull),
        4 => Ok(SourceStage::Domain),
        5 => Ok(SourceStage::Compute),
        _ => Err(bad),
    }
}

/// Whether the device's feature level takes a shader of this DXBC container's model: 9_x takes
/// 4_0 programs carrying their Direct3D 9 form (`Aon9`), 10_0 takes 4_0, 10_1 4_1, 11_x 5_0.
pub fn check_model(bytecode: &[u8], level: u32) -> Result<()> {
    let mut version = None;
    let mut aon9 = false;
    for (tag, chunk) in chunks(bytecode)? {
        if container::is_program(tag) {
            version = Some(program_version(chunk)? & 0xff);
        } else if &tag == b"Aon9" {
            aon9 = true;
        }
    }
    let version = version.ok_or(Error::InvalidArg("not DXBC bytecode"))?;
    let (major, minor) = (version >> 4 & 0xf, version & 0xf);
    let taken = match level {
        crate::desc::level::L9_1..=crate::desc::level::L9_3 => (major, minor) == (4, 0) && aon9,
        crate::desc::level::L10_0 => (major, minor) == (4, 0),
        crate::desc::level::L10_1 => major == 4,
        _ => major <= 5,
    };
    if taken {
        Ok(())
    } else {
        Err(Error::InvalidArg(
            "a shader model the device's feature level does not take",
        ))
    }
}

/// The version token a program chunk starts with.
fn program_version(chunk: Chunk) -> Result<u32> {
    chunk
        .contents()
        .and_then(|c| c.first_chunk())
        .map(|w| u32::from_le_bytes(*w))
        .ok_or(Error::InvalidArg("not DXBC bytecode"))
}

/// The chunks of a DXBC container with their tags; an error if the bytes are not such a container.
fn chunks(bytecode: &[u8]) -> Result<Vec<(Tag, Chunk<'_>)>> {
    let bad = Error::InvalidArg("not DXBC bytecode");
    if !container::is_container(bytecode)
        || container::declared_size(bytecode).ok_or(bad)? as usize > bytecode.len()
    {
        return Err(bad);
    }
    container::chunk_list(bytecode)
        .map_err(|_| bad)?
        .map(|chunk| {
            let chunk = chunk.map_err(|_| bad)?;
            Ok((chunk.tag().ok_or(bad)?, chunk))
        })
        .collect()
}

/// A typed view a shader declares, whose format its program takes from the view bound at the draw:
/// a typed buffer at a texture slot (`dcl_resource` of a buffer) or a typed unordered access view
/// of a texture or a buffer (`dcl_uav_typed`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypedView {
    /// At an unordered access slot; otherwise at a texture slot.
    pub storage: bool,
    pub register: u32,
    /// A view of a buffer.
    pub buffer: bool,
    /// The kind of value the declaration reads and writes.
    pub kind: FormatKind,
    /// A storage texture read and written through a buffer of its texels, whatever the bound
    /// view's format, and why: the back end has no atomics on storage textures
    /// (`AtomicsThroughBuffer`).
    pub through: Option<SlowPath>,
    /// A storage texture of a program that loads typed values from storage textures; every
    /// storage texture of such a program counts, its loads' operands are not read.
    pub loaded: bool,
    /// A typed buffer read through a texture copy of its buffer, where the back end has no
    /// storage buffers (`TexelBufferAsTexture`).
    pub textured: bool,
}

impl TypedView {
    /// The format the shader's plain program is converted with, of the view's kind: one 32-bit
    /// channel for unordered access (which WebGPU reads and writes in one view), four for a typed
    /// buffer read, so that an empty slot reads zero in every component as Direct3D's does.
    pub fn placeholder(self) -> Format {
        match (self.kind, self.storage) {
            (FormatKind::Uint, true) => Format::R32Uint,
            (FormatKind::Sint, true) => Format::R32Sint,
            (_, true) => Format::R32Float,
            (FormatKind::Uint, false) => Format::Rgba32Uint,
            (FormatKind::Sint, false) => Format::Rgba32Sint,
            (_, false) => Format::Rgba32Float,
        }
    }
}

use dxbc_op::{DCL_CONSTANT_BUFFER, DCL_RESOURCE, DCL_UAV_TYPED};
/// A constant buffer declaration's flag (in the dimension's bits) for a buffer indexed at run time.
const CONSTANT_BUFFER_DYNAMIC_INDEXED: u32 = 1;

/// A texture, typed storage or constant buffer declaration of a DXBC program: its opcode, resource
/// dimension (a constant buffer's access pattern), register and return type token (zero for a
/// constant buffer).
struct Declared {
    opcode: u32,
    dimension: u32,
    register: u32,
    returns: u32,
}

/// The `dcl_resource`, `dcl_uav_typed` and `dcl_constantbuffer` declarations of a DXBC container's
/// program, in the token format of Microsoft's `d3d11TokenizedProgramFormat.hpp`.
fn declarations(bytecode: &[u8]) -> Result<Vec<Declared>> {
    let bad = Error::InvalidArg("malformed DXBC program");
    let mut found = Vec::new();
    for instruction in container::instructions(program(bytecode)?) {
        let instruction = instruction.map_err(|_| bad)?;
        let opcode = token::opcode(instruction.opcode_token());
        if matches!(opcode, DCL_RESOURCE | DCL_UAV_TYPED | DCL_CONSTANT_BUFFER) {
            // The operand's indices: one immediate in shader model 5.0, two for a constant
            // buffer's register and size, the identifier and the range's bounds in 5.1; then the
            // return type.
            let (at, indices) = instruction.operand_indices().ok_or(bad)?;
            let word = |i: usize| instruction.word(i).ok_or(bad);
            let register = match indices {
                1 | 2 => word(at)?,
                3 => word(at + 1)?,
                _ => return Err(bad),
            };
            let returns = match opcode {
                DCL_CONSTANT_BUFFER => 0,
                _ => word(at + indices as usize)?,
            };
            found.push(Declared {
                opcode,
                dimension: token::resource_dimension(instruction.opcode_token()),
                register,
                returns,
            });
        }
    }
    Ok(found)
}

/// Whether a DXBC container's program loads typed values from a storage view (`ld_uav_typed`).
fn loads_typed_storage(bytecode: &[u8]) -> Result<bool> {
    for instruction in container::instructions(program(bytecode)?) {
        let instruction = instruction.map_err(|_| Error::InvalidArg("malformed DXBC program"))?;
        if token::opcode(instruction.opcode_token()) == dxbc_op::LD_UAV_TYPED {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The program chunk of a DXBC container.
fn program(bytecode: &[u8]) -> Result<&[u8]> {
    let (_, chunk) = chunks(bytecode)?
        .into_iter()
        .find(|(tag, _)| container::is_program(*tag))
        .ok_or(Error::InvalidArg("malformed DXBC program"))?;
    chunk
        .contents()
        .ok_or(Error::InvalidArg("not DXBC bytecode"))
}

/// The typed views a DXBC container's program declares.
pub fn typed_views(bytecode: &[u8]) -> Result<Vec<TypedView>> {
    let mut views = Vec::new();
    let loads = loads_typed_storage(bytecode)?;
    for d in declarations(bytecode)? {
        let storage = match d.opcode {
            DCL_RESOURCE if d.dimension == dxbc_dimension::BUFFER => false,
            DCL_UAV_TYPED => true,
            _ => continue,
        };
        let kind = match token::return_type(d.returns) {
            dxbc_sampled::UNORM | dxbc_sampled::SNORM | dxbc_sampled::FLOAT => FormatKind::Float,
            dxbc_sampled::SINT => FormatKind::Sint,
            dxbc_sampled::UINT => FormatKind::Uint,
            _ => return Err(Error::InvalidArg("malformed DXBC program")),
        };
        views.push(TypedView {
            storage,
            register: d.register,
            buffer: d.dimension == dxbc_dimension::BUFFER,
            kind,
            through: None,
            loaded: storage && loads,
            textured: false,
        });
    }
    Ok(views)
}

/// The location past every value `outputs` and `inputs` (of the next stage) list, where the values
/// the programs add for each other pass (`VariantKey::linked`).
pub fn linked_location<'a>(values: impl IntoIterator<Item = &'a StageValue>) -> u32 {
    values
        .into_iter()
        .map(|v| v.location + 1)
        .max()
        .unwrap_or(0)
        .max(1)
}

/// `outputs` name the layer each primitive is drawn into.
pub fn names_layer(outputs: &[StageValue]) -> bool {
    outputs
        .iter()
        .any(|v| v.system == Some(SystemValue::RenderTargetArrayIndex))
}

/// The clip distances `outputs` write.
pub fn clip_distances(outputs: &[StageValue]) -> u8 {
    outputs
        .iter()
        .filter(|v| v.system == Some(SystemValue::ClipDistance))
        .map(|v| v.mask.count_ones() as u8)
        .sum()
}

/// The locations of the user values among `values`, in order, each once.
fn between_stages(values: &[StageValue]) -> Vec<u32> {
    let mut at: Vec<u32> = values
        .iter()
        .filter(|v| v.system.is_none())
        .map(|v| v.location)
        .collect();
    at.sort_unstable();
    at.dedup();
    at
}

/// Where a pixel program's `inputs` move (from, to) when one lies past the back end's `slots`
/// inter-stage locations: the registers Direct3D numbers to 31, one of them the position, taken in
/// order from location 0. Nothing moves otherwise; none when they are more than `slots`.
pub fn input_moves(inputs: &[StageValue], slots: u32) -> Option<Vec<(u32, u32)>> {
    let at = between_stages(inputs);
    if at.last().is_none_or(|&l| l < slots) {
        return Some(Vec::new());
    }
    if at.len() > slots as usize {
        return None;
    }
    Some(at.into_iter().zip(0..).filter(|(f, t)| f != t).collect())
}

/// Where a vertex program's `outputs` move to meet the pixel program reading `inputs` (moved by
/// [`input_moves`]): each value it reads where it reads it, each other at its own location when
/// that is inside `slots` and free, or else at the first free one; none when they do not fit.
pub fn output_moves(
    outputs: &[StageValue],
    inputs: &[StageValue],
    slots: u32,
) -> Option<Vec<(u32, u32)>> {
    let read_moves = input_moves(inputs, slots)?;
    let written = between_stages(outputs);
    if read_moves.is_empty() && written.last().is_none_or(|&l| l < slots) {
        return Some(Vec::new());
    }
    let read = between_stages(inputs);
    let read_at = |l: u32| read_moves.iter().find(|m| m.0 == l).map_or(l, |m| m.1);
    let mut taken: Vec<u32> = read.iter().map(|&l| read_at(l)).collect();
    let mut moves = Vec::new();
    for l in written {
        let to = if read.contains(&l) {
            read_at(l)
        } else if l < slots && !taken.contains(&l) {
            l
        } else {
            (0..slots).find(|t| !taken.contains(t))?
        };
        taken.push(to);
        if to != l {
            moves.push((l, to));
        }
    }
    Some(moves)
}

/// Refused: stage values moved make a type equal to one the program has, which naga keeps once.
const MOVED_ONTO_A_TYPE: &str = "stage values moved onto a type the program has";
/// Refused: a program whose stage values moved fails validation.
const MOVED_INVALID: &str = "a program whose stage values moved does not validate";

/// Moves the values `c`'s program passes between stages (a vertex program's outputs, a pixel
/// program's inputs) from and to the locations `moves` names, in its module and its reflection.
/// The module's content key becomes one of its key and the moves.
pub fn relocate(c: &mut Converted, moves: &[(u32, u32)]) -> Result<()> {
    use naga::{Binding, TypeInner};
    if moves.is_empty() {
        return Ok(());
    }
    let to = |l: u32| moves.iter().find(|m| m.0 == l).map_or(l, |m| m.1);
    let shift = |b: &mut Binding| {
        if let Binding::Location { location, .. } = b {
            *location = to(*location);
        }
    };
    let unmovable = Error::NotImplemented("stage values moved in a program of no such values");
    let m = c.modules.last_mut().ok_or(unmovable)?;
    let module = &mut m.module;
    let ep = module
        .entry_points
        .iter_mut()
        .find(|e| e.name == m.entry_point)
        .ok_or(unmovable)?;
    let mut structs = Vec::new();
    let mut each = |binding: &mut Option<Binding>, ty| match binding {
        Some(b) => shift(b),
        None => structs.push(ty),
    };
    match ep.stage {
        naga::ShaderStage::Vertex => {
            if let Some(r) = ep.function.result.as_mut() {
                each(&mut r.binding, r.ty);
            }
        }
        naga::ShaderStage::Fragment => {
            for a in &mut ep.function.arguments {
                each(&mut a.binding, a.ty);
            }
        }
        _ => return Err(unmovable),
    }
    for ty in structs {
        let mut t = module.types[ty].clone();
        if let TypeInner::Struct { members, .. } = &mut t.inner {
            members
                .iter_mut()
                .filter_map(|m| m.binding.as_mut())
                .for_each(shift);
        }
        if t != module.types[ty] {
            if module.types.get(&t).is_some() {
                return not_implemented(MOVED_ONTO_A_TYPE);
            }
            module.types.replace(ty, t);
        }
    }
    m.info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        wcontainers_shaders::capabilities(module),
    )
    .validate(module)
    .or_else(|_| not_implemented(MOVED_INVALID))?;
    let mut key = blake3::Hasher::new();
    key.update(&m.key);
    for (from, to) in moves {
        key.update(&from.to_le_bytes()).update(&to.to_le_bytes());
    }
    m.key = *key.finalize().as_bytes();
    let values = match m.stage {
        ShaderStage::Vertex => &mut c.reflection.outputs,
        _ => &mut c.reflection.inputs,
    };
    for v in values.iter_mut().filter(|v| v.system.is_none()) {
        v.location = to(v.location);
    }
    Ok(())
}

/// The constant buffer slots a DXBC container's program indexes at run time, a bit each.
pub fn indexed_constants(bytecode: &[u8]) -> Result<u16> {
    Ok(declarations(bytecode)?
        .iter()
        .filter(|d| {
            d.opcode == DCL_CONSTANT_BUFFER
                && d.dimension & CONSTANT_BUFFER_DYNAMIC_INDEXED != 0
                && d.register < u16::BITS
        })
        .fold(0, |m, d| m | 1 << d.register))
}

/// The texture slots a DXBC container's program declares as multisampled arrays, a bit each.
pub fn multisampled_arrays(bytecode: &[u8]) -> Result<u128> {
    Ok(declarations(bytecode)?
        .iter()
        .filter(|d| {
            d.opcode == DCL_RESOURCE
                && d.dimension == dxbc_dimension::TEXTURE_2DMS_ARRAY
                && d.register < 128
        })
        .fold(0, |m, d| m | 1 << d.register))
}

/// One element of a DXBC container's input or output signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignatureElement {
    pub semantic: String,
    pub index: u32,
    pub register: u32,
    /// Direct3D's system value name (`D3D_NAME`): 6 is the vertex id, 8 the instance id.
    pub system: u32,
    /// The components it takes, a bit each (x = 1).
    pub mask: u8,
    /// The geometry stream it is written to (outputs of `OSG5` and `OSG1`; 0 otherwise).
    pub stream: u32,
    /// Its scalar type (`D3D_REGISTER_COMPONENT_TYPE`): 1 unsigned, 2 signed, 3 float.
    pub component: u32,
}

impl From<container::Element> for SignatureElement {
    fn from(e: container::Element) -> Self {
        SignatureElement {
            semantic: e.semantic,
            index: e.index,
            register: e.register,
            system: e.system,
            mask: e.mask,
            stream: e.stream,
            component: e.component,
        }
    }
}

/// The input signature (`ISGN`, or `ISG1` of shader model 5.1) of a DXBC container; empty where it
/// has none. An error if the bytes are not such a container or the chunk is malformed.
pub fn input_signature(bytecode: &[u8]) -> Result<Vec<SignatureElement>> {
    signature(bytecode, true)
}

/// The output signature (`OSGN`, `OSG5` of geometry shaders with streams, or `OSG1`).
pub fn output_signature(bytecode: &[u8]) -> Result<Vec<SignatureElement>> {
    signature(bytecode, false)
}

/// The first input (`input`) or output signature chunk's elements.
fn signature(bytecode: &[u8], input: bool) -> Result<Vec<SignatureElement>> {
    let bad = Error::InvalidArg("not DXBC bytecode");
    if !container::is_container(bytecode) {
        return Err(bad);
    }
    for chunk in container::chunk_list(bytecode).map_err(|_| bad)? {
        let chunk = chunk.map_err(|_| bad)?;
        let tag = chunk.tag().ok_or(bad)?;
        if container::signature_direction(tag) != Some(input) {
            continue;
        }
        return container::signature_elements(chunk.contents().ok_or(bad)?, tag)
            .map(|e| e.map(SignatureElement::from).map_err(|_| bad))
            .collect();
    }
    Ok(Vec::new())
}

/// Direct3D's system value names of the vertex and instance ids, which no input element feeds.
const VERTEX_ID: u32 = 6;
const INSTANCE_ID: u32 = 8;
/// Direct3D's system value names of the layer and viewport a primitive is drawn into.
const RENDER_TARGET_ARRAY_INDEX: u32 = 4;
const VIEWPORT_ARRAY_INDEX: u32 = 5;

/// Whether the shader writes the layer or viewport a primitive is drawn into, which only a geometry
/// shader may on a device not offering it from every stage feeding the rasteriser (this one).
pub fn writes_layer_or_viewport(bytecode: &[u8]) -> Result<bool> {
    Ok(output_signature(bytecode)?
        .iter()
        .any(|e| matches!(e.system, RENDER_TARGET_ARRAY_INDEX | VIEWPORT_ARRAY_INDEX)))
}

impl SignatureElement {
    /// The input assembler generates it; no input element feeds it. Direct3D goes by the name too:
    /// a pixel shader's input named so carries no system value.
    pub fn generated(&self) -> bool {
        matches!(self.system, VERTEX_ID | INSTANCE_ID)
            || ["SV_VertexID", "SV_InstanceID"]
                .iter()
                .any(|n| self.semantic.eq_ignore_ascii_case(n))
    }
}

/// What a draw asks of one stage's program beyond the plain conversion, from the state bound at the
/// draw. The default is the plain program.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StageVariant {
    /// Sampler slots whose bound sampler has a level-of-detail bias (`LodBiasInShader`).
    pub lod_bias: u16,
    /// The targets' sample count, for a program that reads it; zero otherwise.
    pub samples: u8,
    /// The pixel program's second output feeds the blend.
    pub dual_source: bool,
    /// The vertex program reads the draw's bases from an indirect draw's arguments
    /// (`DrawBaseFromArguments`).
    pub from_arguments: bool,
    /// Texture slots whose bound view's resource has a minimum level-of-detail clamp past the
    /// view's first level (`MinLodClamp`).
    pub min_lod_clamped: u128,
    /// The input layout whose pulled elements the vertex program fetches (its id; zero for none).
    pub pulled: u64,
    /// Sampler slots addressing by a border the back end lacks: the program filters through them
    /// (`BorderInShader`).
    pub border_in_shader: u16,
    /// Sampler slots mirroring once where the back end does not (`MirrorOnceInShader`).
    pub mirror_once_in_shader: u16,
    /// Texture slots of formats the back end cannot filter, sampled through a filtering sampler
    /// (`FilterInShader`).
    pub filter_in_shader: u128,
    /// The pixel program adds the depth bias (`DepthBiasInShader`).
    pub depth_bias: bool,
    /// The formats of the bound typed views, one for each the shader declares, by the device's
    /// number for the list; zero for the plain program's placeholders.
    pub formats: u32,
    /// Where the values the programs add for each other pass (`VariantKey::linked`); zero where
    /// they add none.
    pub linked: u32,
    /// Depth is clamped in the programs (`DepthClampInShader`), where the back end clips it alone.
    pub depth_clamp: bool,
    /// The vertex program maps positions onto a viewport the back end would round
    /// (`ViewportInShader`).
    pub viewport_adjust: bool,
    /// The clip distances the programs apply themselves, where the back end has none
    /// (`ClipDistanceInShader`).
    pub clip_distances: u8,
    /// The pixel program reads its layer from the one a geometry stage names, passed after the
    /// linked values (`VariantFlags::LAYER_FROM_STAGE`).
    pub layer_from_stage: bool,
    /// The pixel program multiplies target 0's alpha by the line coverage the stage helper's
    /// rectangles pass after the linked values (`AntialiasedLinesAsQuads`).
    pub line_coverage: bool,
    /// Texture slots whose bound view reads stencil, which Direct3D gives in the second component
    /// and the interface's stencil views in the first (`SwizzleInShader`).
    pub stencil: u128,
    /// Level and sample counts are read from the helper constants, where the back end cannot
    /// query them (`TextureInfoInHelpers`).
    pub texture_info: bool,
    /// The pixel program runs its forced early depth and stencil tests itself, against the
    /// pass's read-only depth-stencil attachment (`EarlyTestsInShader`).
    pub early_tests: bool,
    /// Targets the pixel program writes an output of another kind (float, unsigned, signed) than
    /// their format to, as the format's type (`VariantKey::targets_in_shader`), and the device's
    /// number for their formats, in target order.
    pub retyped: u8,
    pub retyped_formats: u32,
    /// Texture slots bound to views of textures held as their integer twins, which the program
    /// decodes (`VariantKey::decoded`), and the device's number for their normalised formats, in
    /// slot order.
    pub decoded: u128,
    pub decoded_formats: u32,
    /// Constant buffer slots the program indexes at run time, bound larger than it declares: it
    /// reads them as storage buffers, past its declared array as Direct3D reads them
    /// (`ConstantsAsStorage`).
    pub constants_as_storage: u16,
    /// Constant buffer slots past the back end's uniform buffers per stage: the program reads them
    /// through textures the front copies the bound windows into (`ConstantsAsTexture`).
    pub constants_as_texture: u16,
    /// Storage texture slots whose bound view's format the back end cannot store into: the
    /// program reads and writes them through a buffer of their texels (`StorageFormatThroughBuffer`).
    pub stored_through: u64,
}

/// The constant buffer slots of `reflection`'s set `set` read as uniform buffers (all but those in
/// `elsewhere`) past `limit` uniform buffers, the highest first, keeping one for the helper
/// constants where the program has them (`helpers`): its program reads them through textures
/// (`ConstantsAsTexture`).
pub fn constants_past_limit(
    reflection: &Reflection,
    set: u32,
    elsewhere: u16,
    helpers: bool,
    limit: u32,
) -> u16 {
    let room = (limit as usize).saturating_sub(usize::from(helpers));
    let mut slots: Vec<u32> = reflection
        .bindings
        .iter()
        .filter(|b| {
            b.kind == SlotKind::Uniform && b.set == set && elsewhere & (1 << b.register) == 0
        })
        .map(|b| b.register)
        .collect();
    if slots.len() <= room {
        return 0;
    }
    slots.sort_unstable();
    slots[room..].iter().fold(0, |m, &slot| m | 1 << slot)
}

/// What a stencil view's components read in Direct3D: the stencil in the second.
const STENCIL_READ: [ViewComponent; 4] = [
    ViewComponent::Zero,
    ViewComponent::R,
    ViewComponent::Zero,
    ViewComponent::One,
];

impl StageVariant {
    /// The shaders crate's key for this variant of a shader of `stage` declaring `typed`, whose
    /// bound views have `formats`.
    pub fn key(
        self,
        stage: SourceStage,
        typed: &[TypedView],
        layered: u128,
        formats: &[Format],
    ) -> VariantKey {
        let mut key = base_key(stage, typed, layered);
        for (t, &f) in typed.iter().zip(formats) {
            let list = if t.storage {
                &mut key.storage_formats
            } else {
                &mut key.texel_formats
            };
            if let Some(e) = list.iter_mut().find(|e| e.0 == t.register) {
                e.1 = f;
            }
        }
        key.lod_bias = self.lod_bias;
        key.constants_as_storage = self.constants_as_storage;
        key.constants_as_texture = self.constants_as_texture;
        key.border_in_shader = self.border_in_shader;
        key.mirror_once_in_shader = self.mirror_once_in_shader;
        key.filter_in_shader = self.filter_in_shader;
        key.min_lod_clamped = self.min_lod_clamped;
        key.texels_through_buffer |= self.stored_through;
        key.swizzled = (0..128)
            .filter(|i| self.stencil & (1 << i) != 0)
            .map(|i| (i, STENCIL_READ))
            .collect();
        key.samples = self.samples;
        if self.dual_source {
            key.flags |= VariantFlags::DUAL_SOURCE;
        }
        if self.depth_bias {
            key.flags |= VariantFlags::DEPTH_BIAS;
        }
        if self.texture_info {
            key.flags |= VariantFlags::TEXTURE_INFO;
        }
        if self.early_tests {
            key.flags |= VariantFlags::EARLY_TESTS;
        }
        if self.from_arguments {
            key.draw_base = DrawBaseSource::Arguments;
        }
        if self.depth_clamp {
            key.flags |= VariantFlags::DEPTH_CLAMP;
        }
        if self.viewport_adjust {
            key.flags |= VariantFlags::VIEWPORT_ADJUST;
        }
        if self.clip_distances != 0 {
            key.flags |= VariantFlags::CLIP_DISTANCE;
        }
        if self.layer_from_stage {
            key.flags |= VariantFlags::LAYER_FROM_STAGE;
        }
        if self.line_coverage {
            key.flags |= VariantFlags::LINE_COVERAGE;
        }
        if self.linked != 0 {
            key.linked = LinkedValues {
                location: self.linked,
                clip_distances: self.clip_distances,
            };
        }
        key
    }
}

/// Where a converted program reads its helper constants: the block's size and each value's offset;
/// the (texture slot, sampler slot) pairs it samples through merged samplers of a minimum
/// level-of-detail clamp, each bound at `CLAMPED_SAMPLER_BINDING` plus its index; and the binding
/// of each input it fetches itself, by location.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Helpers {
    pub size: u32,
    pub values: Vec<HelperSlot>,
    pub merged: Vec<(u32, (u32, u32))>,
    pub pulled: Vec<(u32, u32)>,
    /// The constant buffers read through another kind of binding.
    pub relocated: Vec<RelocatedConstants>,
}

/// A constant buffer slot a program reads through another kind of binding, at `binding`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelocatedConstants {
    pub slot: u32,
    pub binding: u32,
    pub read_as: ConstantsReadAs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstantsReadAs {
    /// A read-only storage buffer (`AddedBinding::ConstantsAsStorage`).
    Storage,
    /// An RGBA32 unsigned texture of its 16-byte rows (`AddedBinding::ConstantsAsTexture`).
    Texture,
}

impl Helpers {
    pub fn of(c: &Converted) -> Self {
        let pairs = &c.reflection.sampled_pairs;
        let merged = c
            .modules
            .iter()
            .flat_map(|m| m.module.global_variables.iter())
            .filter_map(|(_, g)| g.binding.as_ref())
            .filter(|b| b.group == HELPER_SET && b.binding >= CLAMPED_SAMPLER_BINDING)
            .filter_map(|b| {
                let n = b.binding - CLAMPED_SAMPLER_BINDING;
                pairs.get(n as usize).map(|&p| (b.binding, p))
            })
            .collect();
        let pulled = c
            .reflection
            .bindings
            .iter()
            .filter_map(|b| match b.added {
                Some(AddedBinding::PulledStream(location)) => Some((b.binding, location)),
                _ => None,
            })
            .collect();
        let relocated = c
            .reflection
            .bindings
            .iter()
            .filter_map(|b| {
                let (slot, read_as) = match b.added {
                    Some(AddedBinding::ConstantsAsStorage(slot)) => {
                        (slot, ConstantsReadAs::Storage)
                    }
                    Some(AddedBinding::ConstantsAsTexture(slot)) => {
                        (slot, ConstantsReadAs::Texture)
                    }
                    _ => return None,
                };
                Some(RelocatedConstants {
                    slot,
                    binding: b.binding,
                    read_as,
                })
            })
            .collect();
        Helpers {
            size: c.reflection.helper_size,
            values: c.reflection.helpers.clone(),
            merged,
            pulled,
            relocated,
        }
    }

    /// The helper constants of the vertex program `passthrough` drawing a geometry stage that
    /// chooses a layer or viewport: the run it keeps (`stage::PASSTHROUGH_HELPERS`), in the block
    /// its module declares at the vertex stage's helper binding.
    pub fn of_passthrough(passthrough: &ShaderModule, layer_from_stage: bool) -> Self {
        let module = &passthrough.module;
        let binding = helper_binding(ShaderStage::Vertex);
        let size = module
            .global_variables
            .iter()
            .find(|(_, g)| {
                g.binding
                    .as_ref()
                    .is_some_and(|b| b.group == HELPER_SET && b.binding == binding)
            })
            .map_or(0, |(_, g)| module.types[g.ty].inner.size(module.to_ctx()));
        let mut values = wcontainers_shaders::stage::PASSTHROUGH_HELPERS.to_vec();
        if layer_from_stage {
            values.push(wcontainers_shaders::stage::PASSTHROUGH_LAYER_COUNT);
        }
        Helpers {
            size,
            values,
            ..Helpers::default()
        }
    }
}

/// Converts a shader of `expected` stage. Bytecode the converter refuses is invalid to Direct3D too;
/// what it cannot convert yet is not implemented. The flag says the program reads the targets'
/// sample count, which it is converted with at each draw; here it is converted for one sample, so its
/// reflection is known. Where the converter has no atomics on storage textures, the typed texture
/// views are marked to be read through buffers of their texels.
pub fn convert(
    bytecode: &[u8],
    expected: SourceStage,
    typed: &mut [TypedView],
    layered: u128,
    texture_info: bool,
) -> Result<(Converted, bool)> {
    let mut samples = false;
    loop {
        let variant = StageVariant {
            samples: u8::from(samples),
            texture_info,
            ..StageVariant::default()
        };
        match raw(
            bytecode,
            expected,
            &variant.key(expected, typed, layered, &[]),
        )? {
            Err(ConvertError::Unsupported("SampleCount")) if !samples => samples = true,
            Err(ConvertError::Unsupported(why))
                if why == wcontainers_shaders::refusal::STORAGE_IMAGE_ATOMICS
                    && typed
                        .iter()
                        .any(|t| t.storage && !t.buffer && t.through.is_none()) =>
            {
                for t in typed.iter_mut().filter(|t| t.storage && !t.buffer) {
                    t.through = Some(SlowPath::AtomicsThroughBuffer);
                }
            }
            r => return Ok((answer(r, 1)?, samples)),
        }
    }
}

/// The key of a geometry shader declaring `typed`, whose pass-through passes `clip`'s clip
/// distances at its location (`ClipDistanceInShader`); its form `replayed` by a vertex program
/// where the back end has no compute programs (`GeometryAsReplay`).
pub fn geometry_key(
    typed: &[TypedView],
    layered: u128,
    clip: Option<(u32, u8)>,
    replayed: bool,
) -> VariantKey {
    let (linked, clip_distances) = clip.unwrap_or_default();
    let mut key = StageVariant {
        linked,
        clip_distances,
        ..StageVariant::default()
    }
    .key(SourceStage::Geometry, typed, layered, &[]);
    if replayed {
        key.flags |= VariantFlags::GEOMETRY_REPLAY;
    }
    key
}

/// The variant every shader of `stage` declaring `typed` and the multisampled arrays `layered` is
/// converted as: vertex shaders read the draw's bases from the helper constants, since
/// `SV_VertexID` and `SV_InstanceID` leave them out (programs that read neither get no helper
/// block); typed views take their placeholders, typed buffers read through storage buffers
/// (`TexelBufferAsStorage`) or texture copies (`TexelBufferAsTexture`); a multisampled array is one layer, the only kind made
/// (`multisampled_arrays`).
pub fn base_key(stage: SourceStage, typed: &[TypedView], layered: u128) -> VariantKey {
    let formats = |storage: bool| {
        typed
            .iter()
            .filter(|t| t.storage == storage)
            .map(|t| (t.register, t.placeholder()))
            .collect()
    };
    VariantKey {
        draw_base: match stage {
            SourceStage::Vertex => DrawBaseSource::Helpers,
            _ => DrawBaseSource::None,
        },
        storage_formats: formats(true),
        texel_formats: formats(false),
        texel_buffers: if typed.iter().any(|t| t.textured) {
            TexelBufferRoute::Texture
        } else if typed.iter().any(|t| t.buffer) {
            TexelBufferRoute::Storage
        } else {
            TexelBufferRoute::Native
        },
        texels_through_buffer: typed
            .iter()
            .filter(|t| t.through.is_some() && t.register < 64)
            .fold(0, |m, t| m | 1 << t.register),
        texture_layers: (0..128)
            .filter(|i| layered & (1 << i) != 0)
            .map(|i| (i, 1))
            .collect(),
        ..VariantKey::default()
    }
}

/// The sampler slots a converted program reads, one bit each.
pub fn sampler_slots(c: &Converted) -> u16 {
    c.reflection
        .bindings
        .iter()
        .filter(|b| b.kind == SlotKind::Sampler && b.added.is_none() && b.register < 16)
        .fold(0, |m, b| m | (1 << b.register))
}

/// The sampler slots a converted program gathers, compares or offsets through, one bit each: the
/// program can mirror once only around plain samples, so through these it filters itself.
pub fn mirror_filtered_slots(c: &Converted) -> u16 {
    let mut slots = 0;
    for m in &c.modules {
        let module = &m.module;
        let functions = module
            .functions
            .iter()
            .map(|(_, f)| f)
            .chain(module.entry_points.iter().map(|e| &e.function));
        for f in functions {
            for (_, e) in f.expressions.iter() {
                let naga::Expression::ImageSample {
                    sampler,
                    gather,
                    offset,
                    depth_ref,
                    ..
                } = *e
                else {
                    continue;
                };
                if gather.is_none() && offset.is_none() && depth_ref.is_none() {
                    continue;
                }
                let naga::Expression::GlobalVariable(g) = f.expressions[sampler] else {
                    continue;
                };
                let Some(b) = &module.global_variables[g].binding else {
                    continue;
                };
                if let Some(r) = c.reflection.bindings.iter().find(|r| {
                    r.kind == SlotKind::Sampler && r.set == b.group && r.binding == b.binding
                }) && r.register < 16
                {
                    slots |= 1 << r.register;
                }
            }
        }
    }
    slots
}

/// The texture slots a converted program reads as single-sampled depth textures (those it compares
/// through), one bit each.
pub fn depth_slots(c: &Converted) -> u128 {
    let mut slots = 0;
    for m in &c.modules {
        for (_, g) in m.module.global_variables.iter() {
            let naga::TypeInner::Image {
                class: naga::ImageClass::Depth { multi: false },
                ..
            } = m.module.types[g.ty].inner
            else {
                continue;
            };
            let Some(b) = &g.binding else {
                continue;
            };
            if let Some(r) = c.reflection.bindings.iter().find(|r| {
                r.kind == SlotKind::Texture
                    && r.added.is_none()
                    && r.set == b.group
                    && r.binding == b.binding
            }) && r.register < u128::BITS
            {
                slots |= 1 << r.register;
            }
        }
    }
    slots
}

/// The texture slots a converted program reads, one bit each.
pub fn texture_slots(c: &Converted) -> u128 {
    c.reflection
        .bindings
        .iter()
        .filter(|b| b.kind == SlotKind::Texture && b.added.is_none() && b.register < 128)
        .fold(0, |m, b| {
            let n = b.count.clamp(1, 128 - b.register);
            let bits = if n == 128 { !0 } else { (1u128 << n) - 1 };
            m | (bits << b.register)
        })
}

/// Converts a shader of `expected` stage as the variant `key` asks.
pub fn convert_variant(
    bytecode: &[u8],
    expected: SourceStage,
    key: &VariantKey,
) -> Result<Converted> {
    answer(raw(bytecode, expected, key)?, 1)
}

/// The pixel program of a draw without a pixel shader whose variant `key` needs one.
pub fn pixel_program_for_variant(key: &VariantKey) -> Result<Converted> {
    answer(
        wcontainers_shaders::builtin::pixel_program_for_variant(key),
        1,
    )
}

/// Converts a geometry shader as `key` asks: its compute form, which the stage helper runs, and the
/// vertex program drawing what it made.
pub fn convert_geometry(bytecode: &[u8], key: &VariantKey) -> Result<Converted> {
    answer(raw(bytecode, SourceStage::Geometry, key)?, 2)
}

/// The converter's own answer for bytecode of `expected` stage.
fn raw(
    bytecode: &[u8],
    expected: SourceStage,
    key: &VariantKey,
) -> Result<core::result::Result<Converted, ConvertError>> {
    if stage(bytecode)? != expected {
        return Err(Error::InvalidArg(
            "the bytecode is a shader of another stage",
        ));
    }
    Ok(crate::cache::convert(Source::Dxbc(bytecode), key))
}

/// The converter's answer in Direct3D's terms, which should be a program of `modules` stages.
/// The pass-through vertex program of a vertex shader writing cull distances, converted as `key`
/// asks (`CullDistanceAsGeometry`). It depends only on the outputs, so each input is fetched as
/// four 32-bit components of its declared kind.
pub fn cull_passthrough(
    bytecode: &[u8],
    key: VariantKey,
    inputs: &[StageValue],
) -> Result<ShaderModule> {
    use wcontainers_gpu::{ScalarKind, VertexFormat};
    let pulled = inputs
        .iter()
        .filter(|v| v.system.is_none())
        .map(|v| {
            let format = match v.scalar {
                ScalarKind::Float => VertexFormat::Float32x4,
                ScalarKind::Sint => VertexFormat::Sint32x4,
                ScalarKind::Uint => VertexFormat::Uint32x4,
            };
            (v.location, format)
        })
        .collect();
    let vertex = convert_variant(bytecode, SourceStage::Vertex, &VariantKey { pulled, ..key })?;
    match wcontainers_shaders::stage::cull_programs(&vertex) {
        Ok(p) => Ok(p.passthrough),
        Err(e) => refusal(e),
    }
}

fn answer(r: core::result::Result<Converted, ConvertError>, modules: usize) -> Result<Converted> {
    match r {
        Ok(c) if c.modules.len() != modules => {
            not_implemented("shaders converted into several program stages")
        }
        Ok(c)
            if c.reflection.helpers.iter().any(|h| {
                !matches!(
                    h.value,
                    HelperValue::DrawBase
                        | HelperValue::DrawArguments
                        | HelperValue::LodBias { .. }
                        | HelperValue::SamplerState { .. }
                        | HelperValue::BorderColor { .. }
                        | HelperValue::TextureComponents { .. }
                        | HelperValue::MinLod { .. }
                        | HelperValue::DepthBias
                        | HelperValue::PulledStream { .. }
                        | HelperValue::TexelBufferView { .. }
                        | HelperValue::TexelLayout { .. }
                        | HelperValue::DepthRange
                        | HelperValue::ViewportAdjust
                        | HelperValue::TextureLevels { .. }
                        | HelperValue::TextureSamples { .. }
                        | HelperValue::EarlyTests
                        | HelperValue::Layer
                        | HelperValue::Viewport
                        | HelperValue::LayerCount
                )
            }) =>
        {
            not_implemented("shaders that read helper constants the front does not write")
        }
        Ok(c) => Ok(c),
        Err(e) => refusal(e),
    }
}

/// Refused: a shader the converter fails on.
const UNCONVERTED: &str = "shaders the converter does not yet turn into a valid module";

/// A conversion's failure as Direct3D's answer.
fn refusal<T>(e: ConvertError) -> Result<T> {
    use crate::error::WARNING_PREFIX;
    match e {
        ConvertError::Rejected(why) => {
            eprintln!("{WARNING_PREFIX}: shader refused by the converter: {why}");
            Err(Error::InvalidArg("bytecode the converter refuses"))
        }
        ConvertError::Unsupported(what) => not_implemented(what),
        e => {
            eprintln!("{WARNING_PREFIX}: shader conversion failed: {e}");
            not_implemented(UNCONVERTED)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values at `locations`, with a position at register 0.
    fn values(locations: impl IntoIterator<Item = u32>) -> Vec<StageValue> {
        let value = |location, system| StageValue {
            semantic: String::new(),
            semantic_index: 0,
            register: location,
            mask: 0xf,
            scalar: wcontainers_gpu::ScalarKind::Float,
            interpolation: wcontainers_gpu::Interpolation::Perspective,
            system,
            location,
            component: 0,
            stream: 0,
        };
        let mut v = vec![value(0, Some(SystemValue::Position))];
        v.extend(locations.into_iter().map(|l| value(l, None)));
        v
    }

    #[test]
    fn stage_values_past_the_locations_move_into_free_ones_both_stages_agree_on() {
        // 31 values beside the position: registers 1 to 31 onto locations 0 to 30.
        let all = values(1..32);
        let read = input_moves(&all, 31);
        assert_eq!(read, Some((1..32).map(|r| (r, r - 1)).collect()));
        assert_eq!(output_moves(&all, &all, 31), read);
        // Within the locations nothing moves.
        assert_eq!(input_moves(&values(1..31), 31), Some(Vec::new()));
        assert_eq!(
            output_moves(&values(1..31), &values(1..31), 31),
            Some(Vec::new())
        );
        // A pixel shader reading two of them: those keep their places, the vertex shader's value
        // at 31 takes the first free location.
        let written = values([1, 2, 31]);
        let some = values([1, 2]);
        assert_eq!(output_moves(&written, &some, 31), Some(vec![(31, 0)]));
        // The pixel shader reading the one past: its inputs from 0, the vertex shader's others
        // after them.
        let last = values([5, 31]);
        assert_eq!(input_moves(&last, 31), Some(vec![(5, 0), (31, 1)]));
        assert_eq!(
            output_moves(&values([3, 5, 31]), &last, 31),
            Some(vec![(5, 0), (31, 1)])
        );
        // More than the locations hold, read or only written: refused.
        assert_eq!(input_moves(&values(0..32), 31), None);
        assert_eq!(output_moves(&values(0..32), &values(0..32), 31), None);
        assert_eq!(output_moves(&values(0..32), &values([1]), 31), None);
    }

    /// A container holding one program chunk of `tokens`.
    fn container(tokens: &[u32]) -> Vec<u8> {
        let mut code = b"DXBC".to_vec();
        code.resize(20, 0);
        code.extend(1u32.to_le_bytes());
        code.extend(((44 + tokens.len() * 4) as u32).to_le_bytes());
        code.extend(1u32.to_le_bytes());
        code.extend(36u32.to_le_bytes());
        code.extend(b"SHEX");
        code.extend(((tokens.len() * 4) as u32).to_le_bytes());
        for t in tokens {
            code.extend(t.to_le_bytes());
        }
        code
    }

    #[test]
    fn typed_views_are_read_from_their_declarations() {
        let one_index = 1 << 20;
        let tokens = [
            0x0005_0050,
            13,
            // dcl_uav_typed_texture2d (uint,uint,uint,uint) u2
            156 | 3 << 11 | 4 << 24,
            30 << 12 | one_index,
            2,
            0x4444,
            // dcl_resource_buffer (float,float,float,float) t1
            88 | 1 << 11 | 4 << 24,
            7 << 12 | one_index,
            1,
            0x5555,
            // dcl_resource_texture2d (sint,...) t0: a texture, not a typed buffer
            88 | 3 << 11 | 4 << 24,
            7 << 12 | one_index,
            0,
        ];
        let mut tokens = tokens.to_vec();
        tokens.push(0x3333);
        tokens[1] = tokens.len() as u32;
        let views = typed_views(&container(&tokens)).unwrap();
        assert_eq!(
            views,
            [
                TypedView {
                    storage: true,
                    register: 2,
                    buffer: false,
                    kind: FormatKind::Uint,
                    through: None,
                    loaded: false,
                    textured: false,
                },
                TypedView {
                    storage: false,
                    register: 1,
                    buffer: true,
                    kind: FormatKind::Float,
                    through: None,
                    loaded: false,
                    textured: false,
                },
            ]
        );
        let key = base_key(SourceStage::Compute, &views, 0);
        assert_eq!(key.storage_formats, [(2, Format::R32Uint)]);
        assert_eq!(key.texel_formats, [(1, Format::Rgba32Float)]);
        assert_eq!(key.texel_buffers, TexelBufferRoute::Storage);
        let bound = StageVariant::default().key(
            SourceStage::Compute,
            &views,
            0,
            &[Format::Rgba8Uint, Format::Rg16Float],
        );
        assert_eq!(bound.storage_formats, [(2, Format::Rgba8Uint)]);
        assert_eq!(bound.texel_formats, [(1, Format::Rg16Float)]);
    }

    #[test]
    fn truncated_declarations_are_refused() {
        let tokens = [0x0005_0050, 4, 156 | 3 << 11 | 4 << 24, 30 << 12 | 1 << 20];
        assert!(typed_views(&container(&tokens)).is_err());
    }

    /// A container of one input signature chunk holding one element named `SV_Position`, its chunk
    /// `cut` bytes short of the name's end (the bytes stay, past the chunk).
    fn input_signature_cut(cut: usize) -> Vec<u8> {
        let mut chunk: Vec<u8> = [1u32, 8, 32, 0, 1, 3, 0]
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect();
        chunk.extend([0xf, 0xf, 0, 0]);
        chunk.extend(b"SV_Position\0");
        let mut code = b"DXBC".to_vec();
        code.resize(20, 0);
        code.extend(1u32.to_le_bytes());
        code.extend(((44 + chunk.len()) as u32).to_le_bytes());
        code.extend(1u32.to_le_bytes());
        code.extend(36u32.to_le_bytes());
        code.extend(b"ISGN");
        code.extend(((chunk.len() - cut) as u32).to_le_bytes());
        code.extend(chunk);
        code
    }

    #[test]
    fn a_signature_element_reaching_past_its_chunk_is_refused() {
        let whole = input_signature(&input_signature_cut(0)).unwrap();
        assert_eq!(whole.len(), 1);
        assert_eq!(whole[0].semantic, "SV_Position");
        // The chunk ends inside the name, before its NUL, and inside the element's fields.
        for cut in [3, 12 + 20] {
            assert!(matches!(
                input_signature(&input_signature_cut(cut)),
                Err(Error::InvalidArg(_))
            ));
        }
    }

    #[test]
    fn multisampled_arrays_are_one_layer_in_every_key() {
        let one_index = 1 << 20;
        let mut tokens = vec![
            0x0005_0050,
            0,
            // dcl_resource_texture2dmsarray(4) (float,float,float,float) t3
            88 | 9 << 11 | 4 << 16 | 4 << 24,
            7 << 12 | one_index,
            3,
            0x5555,
            // dcl_resource_texture2dms(4) (float,float,float,float) t1
            88 | 4 << 11 | 4 << 16 | 4 << 24,
            7 << 12 | one_index,
            1,
            0x5555,
        ];
        tokens[1] = tokens.len() as u32;
        let layered = multisampled_arrays(&container(&tokens)).unwrap();
        assert_eq!(layered, 1 << 3);
        let key = StageVariant::default().key(SourceStage::Pixel, &[], layered, &[]);
        assert_eq!(key.texture_layers, [(3, 1)]);
    }

    #[test]
    fn stencil_views_read_in_the_second_component() {
        let key = StageVariant {
            stencil: 1 << 2,
            ..StageVariant::default()
        }
        .key(SourceStage::Pixel, &[], 0, &[]);
        assert_eq!(key.swizzled, [(2, STENCIL_READ)]);
    }
}
