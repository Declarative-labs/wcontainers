//! The Direct3D shader container (`DXBC`) both routes' bytecode comes in: a header, the offsets of
//! its chunks, and chunks of a tag and a size. Offsets are taken from these types' layouts. The one
//! reader of it: the converters, both Direct3D fronts and the root signature reader use it.

use core::mem::{offset_of, size_of};
use md5::block_api::Md5Core;
use md5::digest::array::Array;
use md5::digest::block_api::{BlockSizeUser, OutputSizeUser, UpdateCore};
use md5::digest::common::hazmat::SerializableState;
use md5::digest::typenum::Unsigned;
use serde::{Deserialize, Serialize};
use wcontainers_gpu::{ScalarKind as ValueKind, StageValue, SystemValue};

/// A chunk's tag: four characters, read as one word.
pub type Tag = [u8; size_of::<u32>()];

/// A container's checksum: an MD5 digest.
pub type Checksum = [u8; <Md5Core as OutputSizeUser>::OutputSize::USIZE];

/// The container's header; the chunks' offsets (u32 each) follow it.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct Header {
    magic: Tag,
    digest: Checksum,
    version: [u16; 2],
    size: u32,
    chunks: u32,
}

/// The tag a container starts with.
const MAGIC: Tag = *b"DXBC";

/// A chunk's header; its contents follow it.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct ChunkHeader {
    tag: Tag,
    size: u32,
}

/// A signature chunk's header: the element count and the first element's offset in
/// the chunk.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct SignatureHeader {
    count: u32,
    first: u32,
}

/// A signature element's fields of every form; alone, the `ISGN`/`OSGN` form.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct SignatureElement {
    name: u32,
    index: u32,
    system: u32,
    component_type: u32,
    register: u32,
    mask: u8,
    used: u8,
    padding: [u8; 2],
}

/// The `OSG5` form: the geometry stream first.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct StreamElement {
    stream: u32,
    element: SignatureElement,
}

/// The `ISG1`/`OSG1` form: the geometry stream first, the minimum precision last.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct PrecisionElement {
    stream: u32,
    element: SignatureElement,
    min_precision: u32,
}

pub(crate) const ISGN: Tag = *b"ISGN";
pub(crate) const OSGN: Tag = *b"OSGN";
pub(crate) const OSG5: Tag = *b"OSG5";
pub(crate) const ISG1: Tag = *b"ISG1";
pub(crate) const OSG1: Tag = *b"OSG1";
/// The tag of a DXIL program's chunk.
const DXIL: Tag = *b"DXIL";

/// A signature chunk's form: its tag, whether it is an input signature, its elements' size, where
/// their common fields start, and where their geometry stream is, in the forms that have one.
struct SignatureForm {
    tag: Tag,
    input: bool,
    size: usize,
    common: usize,
    stream: Option<usize>,
}

/// Every signature chunk's form, inputs and outputs each in the order their elements are listed.
const SIGNATURE_FORMS: [SignatureForm; 5] = [
    SignatureForm {
        tag: ISGN,
        input: true,
        size: size_of::<SignatureElement>(),
        common: 0,
        stream: None,
    },
    SignatureForm {
        tag: ISG1,
        input: true,
        size: size_of::<PrecisionElement>(),
        common: offset_of!(PrecisionElement, element),
        stream: Some(offset_of!(PrecisionElement, stream)),
    },
    SignatureForm {
        tag: OSGN,
        input: false,
        size: size_of::<SignatureElement>(),
        common: 0,
        stream: None,
    },
    SignatureForm {
        tag: OSG5,
        input: false,
        size: size_of::<StreamElement>(),
        common: offset_of!(StreamElement, element),
        stream: Some(offset_of!(StreamElement, stream)),
    },
    SignatureForm {
        tag: OSG1,
        input: false,
        size: size_of::<PrecisionElement>(),
        common: offset_of!(PrecisionElement, element),
        stream: Some(offset_of!(PrecisionElement, stream)),
    },
];

/// A program's code chunk's start: the version token (the major shader model, then the minor, in
/// the low [`VERSION_PART_BITS`] each), then the program's length in words.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct CodeHeader {
    version: u32,
    length: u32,
}

const VERSION_PART_BITS: u32 = 4;
const VERSION_PART_MASK: u32 = (1 << VERSION_PART_BITS) - 1;

/// A word's bytes: a token, an offset or a size.
const TOKEN_BYTES: usize = size_of::<u32>();

fn word(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at.checked_add(TOKEN_BYTES)?)
        .map(|w| u32::from_le_bytes(w.try_into().expect("a word's bytes")))
}

/// The bytes do not hold together as a container, a signature or a program: a field lies past their
/// end, or an instruction has no length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Malformed;

/// Whether `bytes` start as a container does.
pub fn is_container(bytes: &[u8]) -> bool {
    bytes.starts_with(&MAGIC)
}

/// The container's size as its header gives it.
pub fn declared_size(container: &[u8]) -> Option<u32> {
    word(container, offset_of!(Header, size))
}

/// A chunk of a container, where the container's chunk list places it.
#[derive(Clone, Copy, Debug)]
pub struct Chunk<'a> {
    container: &'a [u8],
    at: usize,
}

impl<'a> Chunk<'a> {
    /// Its tag; `None` where that lies past the container's end.
    pub fn tag(&self) -> Option<Tag> {
        let at = self.at.checked_add(offset_of!(ChunkHeader, tag))?;
        self.container.get(at..)?.first_chunk().copied()
    }

    /// Its size as its header gives it; `None` where that lies past the container's end.
    pub fn size(&self) -> Option<u32> {
        word(
            self.container,
            self.at.checked_add(offset_of!(ChunkHeader, size))?,
        )
    }

    /// Its contents; `None` where its header or contents lie past the container's end.
    pub fn contents(&self) -> Option<&'a [u8]> {
        let start = self.at.checked_add(size_of::<ChunkHeader>())?;
        self.container
            .get(start..start.checked_add(self.size()? as usize)?)
    }
}

/// The chunks of `container`'s chunk list, in its order: `Err` for an entry past the container's end,
/// which is the last item then. `Err` where the chunk count is past the end. The magic is not
/// checked ([`is_container`]).
pub fn chunk_list(
    container: &[u8],
) -> Result<impl Iterator<Item = Result<Chunk<'_>, Malformed>>, Malformed> {
    let count = word(container, offset_of!(Header, chunks)).ok_or(Malformed)?;
    let mut ended = false;
    Ok((0..count as usize).map_while(move |i| {
        if ended {
            return None;
        }
        let at = TOKEN_BYTES
            .checked_mul(i)
            .and_then(|offset| offset.checked_add(size_of::<Header>()))
            .and_then(|entry| word(container, entry));
        ended = at.is_none();
        Some(
            at.map(|at| Chunk {
                container,
                at: at as usize,
            })
            .ok_or(Malformed),
        )
    }))
}

/// The contents of every chunk of `container` tagged `tag`, skipping chunks cut short.
fn chunks(container: &[u8], tag: Tag) -> impl Iterator<Item = &[u8]> {
    chunk_list(container)
        .into_iter()
        .flatten()
        .map_while(Result::ok)
        .filter(move |c| c.tag() == Some(tag))
        .filter_map(|c| c.contents())
}

/// The program's shader model, from its `SHEX` or `SHDR` chunk's version token.
pub(crate) fn shader_model(container: &[u8]) -> Option<(u32, u32)> {
    let code = chunks(container, SHEX)
        .chain(chunks(container, SHDR))
        .next()?;
    let version = word(code, offset_of!(CodeHeader, version))?;
    Some((
        version >> VERSION_PART_BITS & VERSION_PART_MASK,
        version & VERSION_PART_MASK,
    ))
}

/// The program's type (`ir::program`), from the version token of its code chunk (`DXIL`, `SHEX` or
/// `SHDR`): the token's high half.
pub(crate) fn program_type(container: &[u8]) -> Option<u32> {
    let code = chunks(container, DXIL)
        .chain(chunks(container, SHEX))
        .chain(chunks(container, SHDR))
        .next()?;
    Some(word(code, offset_of!(CodeHeader, version))? >> u16::BITS)
}

/// DXBC's opcodes (`D3D10_SB_OPCODE_TYPE`), resource dimensions (`D3D10_SB_RESOURCE_DIMENSION`) and
/// resource return types (`D3D10_SB_RESOURCE_RETURN_TYPE`), as dxbc-spirv names them.
pub use crate::ir::{dxbc_dimension, dxbc_op, dxbc_sampled};

/// The fields of a DXBC instruction's first word (d3d11TokenizedProgramFormat.hpp): the opcode in its
/// low bits, the instruction's length in words, a control point count, and a declaration's resource
/// dimension; of an operand's first word, how many indices it has; and of a resource declaration's
/// return type word, the x component's type. An opcode or operand word is followed by an extended
/// word where its top bit is set.
pub mod token {
    pub const OPCODE_FIELD_BITS: u32 = 11;
    pub const LENGTH_SHIFT: u32 = 24;
    pub const LENGTH_BITS: u32 = 7;
    pub const CONTROL_POINTS_SHIFT: u32 = 11;
    pub const CONTROL_POINTS_BITS: u32 = 6;
    pub const RESOURCE_DIMENSION_SHIFT: u32 = 11;
    pub const RESOURCE_DIMENSION_BITS: u32 = 5;
    pub const INDEX_DIMENSION_SHIFT: u32 = 20;
    pub const INDEX_DIMENSION_BITS: u32 = 2;
    pub const RETURN_TYPE_BITS: u32 = 4;
    pub const EXTENDED_SHIFT: u32 = 31;

    fn field(word: u32, shift: u32, bits: u32) -> u32 {
        word >> shift & ((1 << bits) - 1)
    }

    pub fn opcode(word: u32) -> u32 {
        field(word, 0, OPCODE_FIELD_BITS)
    }

    /// The instruction's length in words (but for custom data, whose length is the next word).
    pub fn length(word: u32) -> u32 {
        field(word, LENGTH_SHIFT, LENGTH_BITS)
    }

    pub fn control_points(word: u32) -> u32 {
        field(word, CONTROL_POINTS_SHIFT, CONTROL_POINTS_BITS)
    }

    pub fn resource_dimension(word: u32) -> u32 {
        field(word, RESOURCE_DIMENSION_SHIFT, RESOURCE_DIMENSION_BITS)
    }

    /// A return type word's x component.
    pub fn return_type(word: u32) -> u32 {
        field(word, 0, RETURN_TYPE_BITS)
    }

    /// An operand's index count.
    pub fn indices(word: u32) -> u32 {
        field(word, INDEX_DIMENSION_SHIFT, INDEX_DIMENSION_BITS)
    }

    /// An extended word follows this one.
    pub fn extended(word: u32) -> bool {
        word >> EXTENDED_SHIFT != 0
    }
}

/// The tags of a DXBC program's code chunk (shader model 5 and 4).
const SHEX: Tag = *b"SHEX";
const SHDR: Tag = *b"SHDR";

/// Whether `tag` names a DXBC program's code chunk.
pub fn is_program(tag: Tag) -> bool {
    tag == SHEX || tag == SHDR
}

/// One instruction of a DXBC program: its words, the opcode token first.
#[derive(Clone, Copy, Debug)]
pub struct Instruction<'a>(&'a [u8]);

impl<'a> Instruction<'a> {
    /// Its bytes, within the program's.
    pub fn bytes(&self) -> &'a [u8] {
        self.0
    }

    /// Its word `i`.
    pub fn word(&self, i: usize) -> Option<u32> {
        word(self.0, i.checked_mul(TOKEN_BYTES)?)
    }

    /// Its length in words.
    pub fn words(&self) -> usize {
        self.0.len() / TOKEN_BYTES
    }

    /// Its opcode token.
    pub fn opcode_token(&self) -> u32 {
        self.word(0).expect("an instruction's opcode token")
    }

    /// Where its first operand's indices start, in words, and how many there are (every index an
    /// immediate word): past the opcode token, its extended words, the operand token and its
    /// extended words. `None` where those run past the instruction.
    pub fn operand_indices(&self) -> Option<(usize, u32)> {
        let mut at = 1;
        let mut extended = token::extended(self.opcode_token());
        while extended {
            extended = token::extended(self.word(at)?);
            at += 1;
        }
        let operand = self.word(at)?;
        at += 1;
        let mut extended = token::extended(operand);
        while extended {
            extended = token::extended(self.word(at)?);
            at += 1;
        }
        Some((at, token::indices(operand)))
    }
}

/// The instructions of a DXBC program (a code chunk's contents), past its version and length words,
/// in order: `Err` for an instruction of no length or one past the program's end, which is the last
/// item then.
pub fn instructions(program: &[u8]) -> impl Iterator<Item = Result<Instruction<'_>, Malformed>> {
    let words = program.len() / TOKEN_BYTES;
    let mut at = size_of::<CodeHeader>() / TOKEN_BYTES;
    let mut ended = false;
    core::iter::from_fn(move || {
        if ended || at >= words {
            return None;
        }
        ended = true;
        let first = word(program, at * TOKEN_BYTES)?;
        let length = match token::opcode(first) {
            dxbc_op::CUSTOM_DATA => word(program, (at + 1) * TOKEN_BYTES).map(|l| l as usize),
            _ => Some(token::length(first) as usize),
        };
        let Some(end) = length
            .filter(|&l| l != 0)
            .and_then(|l| at.checked_add(l))
            .filter(|&end| end <= words)
        else {
            return Some(Err(Malformed));
        };
        let instruction = Instruction(&program[at * TOKEN_BYTES..end * TOKEN_BYTES]);
        at = end;
        ended = false;
        Some(Ok(instruction))
    })
}

/// A DXBC program's declared input control points per patch (hull and domain shaders), from its code
/// chunk.
pub(crate) fn dxbc_input_control_points(container: &[u8]) -> Option<u32> {
    let code = chunks(container, SHEX)
        .chain(chunks(container, SHDR))
        .next()?;
    instructions(code).map_while(Result::ok).find_map(|i| {
        let first = i.opcode_token();
        (token::opcode(first) == dxbc_op::DCL_INPUT_CONTROL_POINT_COUNT)
            .then(|| token::control_points(first))
    })
}

/// The start of a DXIL container's `PSV0` chunk (DirectXShaderCompiler's
/// DxilPipelineStateValidation.h): the size of its runtime information, then that information, which
/// for a geometry shader starts with `GSInfo`.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct PipelineStateGeometry {
    runtime_info_size: u32,
    input_primitive: u32,
    output_topology: u32,
    output_stream_mask: u32,
}

/// The start of a hull or domain shader's `PSV0` chunk: `HSInfo` and `DSInfo` both start with the
/// input control point count.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct PipelineStateTessellation {
    runtime_info_size: u32,
    input_control_points: u32,
}

/// The tag of a DXIL container's pipeline state chunk.
const PIPELINE_STATE_TAG: Tag = *b"PSV0";
/// `D3D_PRIMITIVE_1_CONTROL_POINT_PATCH`, the first of the patch primitives.
pub(crate) const FIRST_PATCH_PRIMITIVE: u32 = 8;

/// A DXIL geometry shader's input primitive (`D3D_PRIMITIVE`), from its `PSV0` chunk.
pub(crate) fn geometry_input_primitive(container: &[u8]) -> Option<u32> {
    let psv = chunks(container, PIPELINE_STATE_TAG).next()?;
    word(psv, offset_of!(PipelineStateGeometry, input_primitive))
}

/// A DXIL hull or domain shader's input control points per patch, from its `PSV0` chunk.
pub(crate) fn patch_input_points(container: &[u8]) -> Option<u32> {
    let psv = chunks(container, PIPELINE_STATE_TAG).next()?;
    word(
        psv,
        offset_of!(PipelineStateTessellation, input_control_points),
    )
}

/// An element of an input or output signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Element {
    pub semantic: alloc::string::String,
    pub index: u32,
    /// Direct3D's system value name (`D3D_NAME`), `ir::sysval::NONE` for none.
    pub system: u32,
    /// Its scalar type (`D3D_REGISTER_COMPONENT_TYPE`).
    pub component: u32,
    pub register: u32,
    /// The components it takes, a bit each (x = 1).
    pub mask: u8,
    /// The geometry stream it belongs to (0 in the forms without one).
    pub stream: u32,
}

/// Whether a chunk tagged `tag` holds an input signature (`Some(true)`), an output signature
/// (`Some(false)`) or neither.
pub fn signature_direction(tag: Tag) -> Option<bool> {
    SIGNATURE_FORMS
        .iter()
        .find(|f| f.tag == tag)
        .map(|f| f.input)
}

/// The elements of the signature chunk `contents` tagged `tag` (none for a tag of no signature), in
/// order: `Err` where the chunk's header, an element's fields or its name (with its terminating NUL)
/// lie past the chunk's end, which is the last item then.
pub fn signature_elements(
    contents: &[u8],
    tag: Tag,
) -> impl Iterator<Item = Result<Element, Malformed>> + '_ {
    let form = SIGNATURE_FORMS.iter().find(|f| f.tag == tag);
    let header = (
        word(contents, offset_of!(SignatureHeader, count)),
        word(contents, offset_of!(SignatureHeader, first)),
    );
    let (count, first, form) = match (form, header) {
        (Some(form), (Some(count), Some(first))) => (count as usize, first as usize, Some(form)),
        (Some(_), _) => (1, 0, None),
        (None, _) => (0, 0, None),
    };
    let mut ended = false;
    (0..count).map_while(move |e| {
        if ended {
            return None;
        }
        let element = form.and_then(|form| signature_element(contents, form, first, e));
        ended = element.is_none();
        Some(element.ok_or(Malformed))
    })
}

/// Element `e` of a signature chunk's `contents` of `form`, whose elements start at `first`.
fn signature_element(
    contents: &[u8],
    form: &SignatureForm,
    first: usize,
    e: usize,
) -> Option<Element> {
    let start = form.size.checked_mul(e)?.checked_add(first)?;
    let at = start.checked_add(form.common)?;
    let field = |offset: usize| word(contents, at.checked_add(offset)?);
    let name = field(offset_of!(SignatureElement, name))?;
    let name = contents.get(name as usize..)?;
    let semantic = &name[..name.iter().position(|&c| c == 0)?];
    Some(Element {
        semantic: alloc::string::String::from_utf8_lossy(semantic).into_owned(),
        index: field(offset_of!(SignatureElement, index))?,
        system: field(offset_of!(SignatureElement, system))?,
        component: field(offset_of!(SignatureElement, component_type))?,
        register: field(offset_of!(SignatureElement, register))?,
        mask: *contents.get(at.checked_add(offset_of!(SignatureElement, mask))?)?,
        stream: match form.stream {
            Some(offset) => word(contents, start.checked_add(offset)?)?,
            None => 0,
        },
    })
}

/// The elements of every chunk of `container` tagged `tag`, each chunk's up to its first malformed
/// element.
pub(crate) fn signature(container: &[u8], tag: &Tag) -> alloc::vec::Vec<Element> {
    chunks(container, *tag)
        .flat_map(|chunk| signature_elements(chunk, *tag).map_while(Result::ok))
        .collect()
}

/// The input (`ISGN`, `ISG1`) or output (`OSGN`, `OSG5`, `OSG1`) signature of a DXBC container.
pub(crate) fn dxbc_signature(container: &[u8], input: bool) -> alloc::vec::Vec<Element> {
    SIGNATURE_FORMS
        .iter()
        .filter(|f| f.input == input)
        .flat_map(|f| signature(container, &f.tag))
        .collect()
}

/// The first byte of MD5's padding.
const MD5_PADDING_START: u8 = 0x80;
/// MD5's block.
const MD5_BLOCK: usize = <Md5Core as BlockSizeUser>::BlockSize::USIZE;

/// The checksum of a DXBC container (its header's digest): MD5's rounds over the bytes after the
/// checksum, with DXBC's own last block (the bit count first, then the rest of the bytes, MD5's
/// padding, and the bit count divided by 4 with its low bit set), no MD5 finalisation.
pub fn checksum(container: &[u8]) -> Checksum {
    let bytes = container
        .get(offset_of!(Header, version)..)
        .unwrap_or_default();
    let bits = (bytes.len() as u32).wrapping_mul(u8::BITS);
    let (whole, rest) = Array::slice_as_chunks(bytes);
    let mut core = Md5Core::default();
    core.update_blocks(whole);
    let mut last = [[0u8; MD5_BLOCK]; 2];
    let tail = if rest.len() + 2 * TOKEN_BYTES >= MD5_BLOCK {
        last[0][..rest.len()].copy_from_slice(rest);
        last[0][rest.len()] = MD5_PADDING_START;
        last[1][..TOKEN_BYTES].copy_from_slice(&bits.to_le_bytes());
        &mut last[..]
    } else {
        last[0][..TOKEN_BYTES].copy_from_slice(&bits.to_le_bytes());
        last[0][TOKEN_BYTES..TOKEN_BYTES + rest.len()].copy_from_slice(rest);
        last[0][TOKEN_BYTES + rest.len()] = MD5_PADDING_START;
        &mut last[..1]
    };
    tail.last_mut().expect("a block")[MD5_BLOCK - TOKEN_BYTES..]
        .copy_from_slice(&((bits >> 2) | 1).to_le_bytes());
    core.update_blocks(Array::cast_slice_from_core(tail));
    let mut out = Checksum::default();
    out.copy_from_slice(&core.serialize()[..size_of::<Checksum>()]);
    out
}

/// Direct3D's system value name (`ir::sysval`) of `system`, where the signature names it.
pub(crate) fn system_name(system: SystemValue) -> Option<u32> {
    use crate::ir::sysval;
    Some(match system {
        SystemValue::Position => sysval::POSITION,
        SystemValue::VertexId => sysval::VERTEX_ID,
        SystemValue::InstanceId => sysval::INSTANCE_ID,
        SystemValue::PrimitiveId => sysval::PRIMITIVE_ID,
        SystemValue::FrontFacing => sysval::IS_FRONT_FACE,
        SystemValue::SampleIndex => sysval::SAMPLE_INDEX,
        SystemValue::Coverage => sysval::COVERAGE,
        SystemValue::Depth => sysval::DEPTH,
        SystemValue::DepthGreaterEqual => sysval::DEPTH_GREATER_EQUAL,
        SystemValue::DepthLessEqual => sysval::DEPTH_LESS_EQUAL,
        SystemValue::StencilRef => sysval::STENCIL_REF,
        SystemValue::ClipDistance => sysval::CLIP_DISTANCE,
        SystemValue::CullDistance => sysval::CULL_DISTANCE,
        SystemValue::RenderTargetArrayIndex => sysval::RENDER_TARGET_ARRAY_INDEX,
        SystemValue::ViewportArrayIndex => sysval::VIEWPORT_INDEX,
        SystemValue::BaseVertex
        | SystemValue::BaseInstance
        | SystemValue::DrawIndex
        | SystemValue::Target(_) => return None,
    })
}

/// Gives each system value of `values` its signature's semantic, register and mask: one value per
/// element naming it, so distances spread over several registers are each listed once, however the
/// program declares them.
pub(crate) fn place_system_values(values: &mut alloc::vec::Vec<StageValue>, signature: &[Element]) {
    let mut placed = alloc::vec::Vec::with_capacity(values.len());
    let mut named = alloc::vec::Vec::new();
    for v in values.drain(..) {
        let name = v.system.and_then(system_name);
        let elements: alloc::vec::Vec<&Element> = match name {
            Some(name) => signature.iter().filter(|e| e.system == name).collect(),
            None => alloc::vec::Vec::new(),
        };
        if elements.is_empty() {
            placed.push(v);
            continue;
        }
        if named.contains(&name) {
            continue;
        }
        named.push(name);
        for e in elements {
            placed.push(StageValue {
                semantic: e.semantic.clone(),
                semantic_index: e.index,
                register: e.register,
                mask: e.mask,
                ..v.clone()
            });
        }
    }
    *values = placed;
}

/// What a view's resource kind (`DXIL::ResourceKind`) is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DxilShape {
    /// A texture of any kind but a multisampled array.
    Texture,
    /// A multisampled array texture (`Texture2DMSArray`).
    MultisampledArray,
    TypedBuffer,
    /// A raw or structured buffer, or another resource.
    Other,
}

/// The range size [`DxilView::count`] gives a range without one.
pub const RANGE_WITHOUT_SIZE: u32 = crate::ir::view_range::UNBOUNDED;

/// A shader resource view or unordered access view of a DXIL module's resource list
/// (`dx.resources`; `DXIL.rst`, "Resources").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DxilView {
    pub shape: DxilShape,
    pub space: u32,
    /// Its range's first register and size ([`RANGE_WITHOUT_SIZE`] for a range without one).
    pub register: u32,
    pub count: u32,
    /// The kind of value its elements hold, where its extended properties give their type.
    pub value: Option<ValueKind>,
}

/// The two view lists of a DXIL module's resource list, each in its order: `None` for a list the
/// resource list lacks or one holding a view whose fields cannot be read.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DxilViews {
    /// The shader resource views.
    pub views: Option<alloc::vec::Vec<DxilView>>,
    /// The unordered access views.
    pub storage: Option<alloc::vec::Vec<DxilView>>,
}

/// The views `container`'s DXIL module declares, read by dxil-spirv's reader in one pass over the
/// module; `None` where it holds no `DXIL` chunk, its checksum does not match, or its module does not
/// parse.
pub fn dxil_views(container: &[u8]) -> Option<DxilViews> {
    use crate::ir::{view_shape, view_value, view_word};
    chunks(container, DXIL).next()?;
    let words = crate::dxil::views(container).ok()?;
    let (&held, views) = words.split_first()?;
    let mut lists = [Some(alloc::vec::Vec::new()), Some(alloc::vec::Vec::new())];
    for list in lists.iter_mut().skip(held as usize) {
        *list = None;
    }
    for v in views.as_chunks::<{ view_word::COUNT }>().0 {
        let Some(list) = lists.get_mut(v[view_word::LIST] as usize) else {
            continue;
        };
        if v[view_word::READABLE] == 0 {
            *list = None;
        }
        let Some(list) = list else {
            continue;
        };
        list.push(DxilView {
            shape: match v[view_word::SHAPE] {
                view_shape::TEXTURE => DxilShape::Texture,
                view_shape::MULTISAMPLED_ARRAY => DxilShape::MultisampledArray,
                view_shape::TYPED_BUFFER => DxilShape::TypedBuffer,
                _ => DxilShape::Other,
            },
            space: v[view_word::SPACE],
            register: v[view_word::LOWER_BOUND],
            count: v[view_word::RANGE_SIZE],
            value: match v[view_word::VALUE] {
                view_value::FLOAT => Some(ValueKind::Float),
                view_value::SINT => Some(ValueKind::Sint),
                view_value::UINT => Some(ValueKind::Uint),
                _ => None,
            },
        });
    }
    let [views, storage] = lists;
    Some(DxilViews { views, storage })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// A container of one chunk tagged `tag` holding `contents`.
    fn container(tag: Tag, contents: &[u8]) -> Vec<u8> {
        let mut c = MAGIC.to_vec();
        c.resize(offset_of!(Header, chunks), 0);
        c.extend(1u32.to_le_bytes());
        c.extend(((size_of::<Header>() + TOKEN_BYTES) as u32).to_le_bytes());
        c.extend(tag);
        c.extend((contents.len() as u32).to_le_bytes());
        c.extend(contents);
        let size = c.len() as u32;
        c[offset_of!(Header, size)..][..TOKEN_BYTES].copy_from_slice(&size.to_le_bytes());
        c
    }

    /// An `OSGN` chunk of one element whose name, `name`, is at `offset` (the bytes after the
    /// element).
    fn output_signature(offset: u32, name: &[u8]) -> Vec<u8> {
        let first = size_of::<SignatureHeader>() as u32;
        let mut s: Vec<u8> = [1, first, offset, 0, 0, 3, 0]
            .iter()
            .flat_map(|w: &u32| w.to_le_bytes())
            .collect();
        s.extend([0xf, 0xf, 0, 0]);
        s.extend(name);
        s
    }

    const NAME_AT: u32 = (size_of::<SignatureHeader>() + size_of::<SignatureElement>()) as u32;

    #[test]
    fn a_name_ends_with_its_nul_inside_its_chunk() {
        let whole = output_signature(NAME_AT, b"SV_Target\0");
        let names = |chunk: &[u8]| -> Vec<Result<alloc::string::String, Malformed>> {
            signature_elements(chunk, OSGN)
                .map(|e| e.map(|e| e.semantic))
                .collect()
        };
        assert_eq!(names(&whole), [Ok("SV_Target".into())]);
        // The chunk ends inside the name, before its NUL; then at the name's offset; then before it.
        let cut = &whole[..NAME_AT as usize + 5];
        assert_eq!(names(cut), [Err(Malformed)]);
        assert_eq!(names(&whole[..NAME_AT as usize]), [Err(Malformed)]);
        let past = output_signature(NAME_AT + 64, b"SV_Target\0");
        assert_eq!(names(&past), [Err(Malformed)]);
        // The converters' list keeps the elements before a malformed one, here none.
        let mut cut_container = container(OSGN, cut);
        cut_container.extend(b"rget\0");
        assert_eq!(signature(&cut_container, &OSGN), []);
        assert_eq!(signature(&container(OSGN, &whole), &OSGN).len(), 1);
    }

    /// `dcl_input_control_point_count` with `points` and an instruction length of `length` words.
    fn control_point_declaration(points: u32, length: u32) -> u32 {
        dxbc_op::DCL_INPUT_CONTROL_POINT_COUNT
            | points << token::CONTROL_POINTS_SHIFT
            | length << token::LENGTH_SHIFT
    }

    #[test]
    fn an_instruction_past_its_program_ends_the_walk_before_its_opcode_is_read() {
        let program = |declaration: u32| {
            let words = [0x0005_0050, 3, declaration];
            let bytes: Vec<u8> = words.iter().flat_map(|w: &u32| w.to_le_bytes()).collect();
            container(SHEX, &bytes)
        };
        assert_eq!(
            dxbc_input_control_points(&program(control_point_declaration(3, 1))),
            Some(3)
        );
        assert_eq!(
            dxbc_input_control_points(&program(control_point_declaration(3, 2))),
            None
        );
        let one = program(control_point_declaration(3, 2));
        let code = chunks(&one, SHEX).next().unwrap();
        let walked: Vec<_> = instructions(code).map(|i| i.map(|_| ())).collect();
        assert_eq!(walked, [Err(Malformed)]);
    }

    #[test]
    fn a_chunk_list_entry_past_the_container_is_its_last_item() {
        let mut c = container(OSGN, &output_signature(NAME_AT, b"SV_Target\0"));
        c[offset_of!(Header, chunks)..][..TOKEN_BYTES].copy_from_slice(&3u32.to_le_bytes());
        c.truncate(size_of::<Header>() + TOKEN_BYTES + 2);
        let listed: Vec<bool> = chunk_list(&c).unwrap().map(|c| c.is_ok()).collect();
        assert_eq!(listed, [true, false]);
        assert_eq!(
            chunk_list(&c[..offset_of!(Header, chunks)]).err(),
            Some(Malformed)
        );
    }

    #[test]
    fn a_module_s_views_are_read_once_through_the_cache() {
        use crate::cache::{Cache, dxil_views_cached};
        // texels.hlsl's cs_texels: Buffer<float4> t0 to t3, RWBuffer<float4> u0, RWBuffer<uint>
        // u1 and a structured buffer at u2.
        let code = include_bytes!("../tests/own/cs_texels.dxil");
        let read = dxil_views(code).unwrap();
        let shapes = |list: &Option<Vec<DxilView>>| -> Vec<DxilShape> {
            list.as_ref().unwrap().iter().map(|v| v.shape).collect()
        };
        assert_eq!(shapes(&read.views), [DxilShape::TypedBuffer; 4]);
        let storage = [
            DxilShape::TypedBuffer,
            DxilShape::TypedBuffer,
            DxilShape::Other,
        ];
        assert_eq!(shapes(&read.storage), storage);
        let mut cache = Cache::default();
        let (first, appended) = dxil_views_cached(&mut cache, code).unwrap();
        assert_eq!((first.as_ref(), appended.is_empty()), (Some(&read), false));
        assert_eq!(
            dxil_views_cached(&mut cache, code).unwrap(),
            (Some(read.clone()), Vec::new())
        );
        let (mut loaded, _) = Cache::load(cache.bytes().to_vec()).unwrap();
        assert_eq!(
            dxil_views_cached(&mut loaded, code).unwrap(),
            (Some(read), Vec::new())
        );
        // A container without a module is recorded as declaring none.
        let dxbc = include_bytes!("../tests/own/cs_images.dxbc");
        assert_eq!(dxil_views_cached(&mut loaded, dxbc).unwrap().0, None);
        assert_eq!(
            dxil_views_cached(&mut loaded, dxbc).unwrap(),
            (None, Vec::new())
        );
    }

    #[test]
    fn a_views_record_that_does_not_decode_is_malformed() {
        use crate::cache::{Cache, Kind, dxil_views_cached, views_key};
        let code = include_bytes!("../tests/own/cs_texels.dxil");
        let mut cache = Cache::default();
        assert!(
            !cache
                .insert(Kind::Views, views_key(code), &[u8::MAX])
                .is_empty()
        );
        assert!(matches!(
            dxil_views_cached(&mut cache, code),
            Err(crate::ConvertError::Malformed(_))
        ));
    }

    /// The checksum dxc writes where it does not sign a container.
    const BYPASS: Checksum = [1; size_of::<Checksum>()];

    /// `code` with `digest` as its checksum.
    fn with_digest(code: &[u8], digest: Checksum) -> Vec<u8> {
        let mut c = code.to_vec();
        c[offset_of!(Header, digest)..][..size_of::<Checksum>()].copy_from_slice(&digest);
        c
    }

    #[test]
    fn a_module_s_views_are_read_with_its_checksum_or_the_bypass_value_only() {
        // compute.hlsl's cs_images: four storage textures, signed by the validator.
        let code = include_bytes!("../tests/own/cs_images.dxil");
        assert_eq!(with_digest(code, checksum(code)), code);
        let read = dxil_views(code).unwrap();
        assert_eq!(read.storage.as_ref().map(Vec::len), Some(4));
        assert_eq!(dxil_views(&with_digest(code, BYPASS)), Some(read));
        assert_eq!(dxil_views(&with_digest(code, Checksum::default())), None);
        let mut unsigned = checksum(code);
        unsigned[0] ^= 1;
        assert_eq!(dxil_views(&with_digest(code, unsigned)), None);
    }

    #[test]
    fn modules_read_in_turn_each_give_their_own_lists() {
        use crate::{Source, cache::encode, convert};
        use wcontainers_gpu::VariantKey;
        let codes: [&[u8]; 7] = [
            include_bytes!("../tests/own/cs_images.dxil"),
            include_bytes!("../tests/own/cs_texels.dxil"),
            include_bytes!("../tests/own/cs_image_array.dxil"),
            include_bytes!("../tests/own/cs_view_kinds.dxil"),
            include_bytes!("../tests/own/cs_typed_array.dxil"),
            include_bytes!("../tests/own/ps_msarray.dxil"),
            include_bytes!("../tests/own/cs_atomics.dxil"),
        ];
        let alone: Vec<_> = codes.iter().map(|c| dxil_views(c)).collect();
        assert!(alone.iter().all(Option::is_some));
        // Each read after every other, more modules than a thread keeps parsed.
        for round in 0..2 {
            for (i, code) in codes.iter().enumerate().rev() {
                assert_eq!(dxil_views(code), alone[i], "round {round}, module {i}");
                assert_eq!(dxil_views(codes[0]), alone[0], "round {round}, after {i}");
            }
        }
        // A conversion, which reads its module's declared registers, made after other modules'
        // views and after its own.
        // cs_atomics: five storage buffers, three with counters, converted with the default key.
        let (&code, others) = codes.split_last().unwrap();
        let converted =
            || encode(&convert(Source::Dxil(code), &VariantKey::default(), None).unwrap());
        dxil_views(code);
        let first = converted();
        for other in others {
            dxil_views(other);
        }
        assert_eq!(converted(), first);
        dxil_views(code);
        dxil_views(others[0]);
        assert_eq!(converted(), first);
    }
}
