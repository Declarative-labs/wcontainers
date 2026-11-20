//! The Direct3D shader container (`DXBC`) both routes' bytecode comes in: a header, the offsets of
//! its chunks, and chunks of a tag and a size. Offsets are taken from these types' layouts.

use core::mem::{offset_of, size_of};
use wcontainers_gpu::{StageValue, SystemValue};

/// The container's header; the chunks' offsets (u32 each) follow it.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct Header {
    magic: [u8; 4],
    digest: [u8; 16],
    version: [u16; 2],
    size: u32,
    chunks: u32,
}

/// A chunk's header; its contents follow it.
#[repr(C)]
#[allow(dead_code, reason = "read through its layout")]
struct ChunkHeader {
    tag: [u8; 4],
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

pub(crate) const ISGN: [u8; 4] = *b"ISGN";
pub(crate) const OSGN: [u8; 4] = *b"OSGN";
pub(crate) const OSG5: [u8; 4] = *b"OSG5";
pub(crate) const ISG1: [u8; 4] = *b"ISG1";
pub(crate) const OSG1: [u8; 4] = *b"OSG1";

/// Each signature chunk's tag, its elements' size and where their common fields start.
const SIGNATURE_FORMS: [([u8; 4], usize, usize); 5] = [
    (ISGN, size_of::<SignatureElement>(), 0),
    (OSGN, size_of::<SignatureElement>(), 0),
    (
        OSG5,
        size_of::<StreamElement>(),
        offset_of!(StreamElement, element),
    ),
    (
        ISG1,
        size_of::<PrecisionElement>(),
        offset_of!(PrecisionElement, element),
    ),
    (
        OSG1,
        size_of::<PrecisionElement>(),
        offset_of!(PrecisionElement, element),
    ),
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

fn word(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + size_of::<u32>())
        .map(|w| u32::from_le_bytes(w.try_into().expect("a word's bytes")))
}

/// The contents of every chunk of `container` tagged `tag`.
fn chunks<'a>(container: &'a [u8], tag: &'a [u8; 4]) -> impl Iterator<Item = &'a [u8]> + 'a {
    let count = word(container, offset_of!(Header, chunks)).unwrap_or(0) as usize;
    (0..count)
        .map_while(move |i| {
            let at = word(container, size_of::<Header>() + size_of::<u32>() * i)? as usize;
            Some((at, container))
        })
        .filter_map(move |(at, container)| {
            let head = container.get(at..at + size_of::<ChunkHeader>())?;
            if head[offset_of!(ChunkHeader, tag)..][..tag.len()] != tag[..] {
                return None;
            }
            let size = word(head, offset_of!(ChunkHeader, size))? as usize;
            let start = at + size_of::<ChunkHeader>();
            container.get(start..start + size)
        })
}

/// The program's shader model, from its `SHEX` or `SHDR` chunk's version token.
pub(crate) fn shader_model(container: &[u8]) -> Option<(u32, u32)> {
    let code = chunks(container, b"SHEX")
        .chain(chunks(container, b"SHDR"))
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
    let code = chunks(container, b"DXIL")
        .chain(chunks(container, b"SHEX"))
        .chain(chunks(container, b"SHDR"))
        .next()?;
    Some(word(code, offset_of!(CodeHeader, version))? >> u16::BITS)
}

/// The DXBC instruction token's fields (d3d11TokenizedProgramFormat.hpp): the opcode in its low bits,
/// the instruction's length in words from `LENGTH_SHIFT`, a control point count from
/// `CONTROL_POINTS_SHIFT`.
const OPCODE_BITS: u32 = 11;
const LENGTH_SHIFT: u32 = 24;
const LENGTH_BITS: u32 = 7;
const CONTROL_POINTS_SHIFT: u32 = 11;
const CONTROL_POINTS_BITS: u32 = 6;
/// `D3D10_SB_OPCODE_CUSTOMDATA`, whose length is the word after its token.
const CUSTOM_DATA: u32 = 53;
/// `D3D11_SB_OPCODE_DCL_INPUT_CONTROL_POINT_COUNT`.
const DCL_INPUT_CONTROL_POINT_COUNT: u32 = 147;

/// `bits` bits of `token` from `shift`.
fn field(token: u32, shift: u32, bits: u32) -> u32 {
    token >> shift & ((1 << bits) - 1)
}

/// A DXBC program's declared input control points per patch (hull and domain shaders), from its code
/// chunk.
pub(crate) fn dxbc_input_control_points(container: &[u8]) -> Option<u32> {
    let code = chunks(container, b"SHEX")
        .chain(chunks(container, b"SHDR"))
        .next()?;
    let mut at = size_of::<CodeHeader>();
    while let Some(token) = word(code, at) {
        let opcode = field(token, 0, OPCODE_BITS);
        if opcode == DCL_INPUT_CONTROL_POINT_COUNT {
            return Some(field(token, CONTROL_POINTS_SHIFT, CONTROL_POINTS_BITS));
        }
        let length = match opcode {
            CUSTOM_DATA => word(code, at + size_of::<u32>())?,
            _ => field(token, LENGTH_SHIFT, LENGTH_BITS),
        };
        if length == 0 {
            return None;
        }
        at += length as usize * size_of::<u32>();
    }
    None
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
const PIPELINE_STATE_TAG: [u8; 4] = *b"PSV0";
/// `D3D_PRIMITIVE_1_CONTROL_POINT_PATCH`, the first of the patch primitives.
pub(crate) const FIRST_PATCH_PRIMITIVE: u32 = 8;

/// A DXIL geometry shader's input primitive (`D3D_PRIMITIVE`), from its `PSV0` chunk.
pub(crate) fn geometry_input_primitive(container: &[u8]) -> Option<u32> {
    let psv = chunks(container, &PIPELINE_STATE_TAG).next()?;
    word(psv, offset_of!(PipelineStateGeometry, input_primitive))
}

/// A DXIL hull or domain shader's input control points per patch, from its `PSV0` chunk.
pub(crate) fn patch_input_points(container: &[u8]) -> Option<u32> {
    let psv = chunks(container, &PIPELINE_STATE_TAG).next()?;
    word(
        psv,
        offset_of!(PipelineStateTessellation, input_control_points),
    )
}

/// An element of an input or output signature.
pub(crate) struct Element {
    pub(crate) semantic: alloc::string::String,
    pub(crate) index: u32,
    /// Direct3D's system value name (`D3D_NAME`), `ir::sysval::NONE` for none.
    pub(crate) system: u32,
    pub(crate) register: u32,
    pub(crate) mask: u8,
}

/// The elements of the container's signature chunk `tag` (one of [`SIGNATURE_FORMS`]).
pub(crate) fn signature(container: &[u8], tag: &[u8; 4]) -> alloc::vec::Vec<Element> {
    let mut out = alloc::vec::Vec::new();
    let Some(&(_, size, common)) = SIGNATURE_FORMS.iter().find(|f| f.0 == *tag) else {
        return out;
    };
    for chunk in chunks(container, tag) {
        let count = word(chunk, offset_of!(SignatureHeader, count)).unwrap_or(0);
        let first = word(chunk, offset_of!(SignatureHeader, first)).unwrap_or(0) as usize;
        for e in 0..count as usize {
            let at = first + size * e + common;
            let field = |offset| word(chunk, at + offset);
            let (Some(name), Some(index), Some(system), Some(register), Some(&mask)) = (
                field(offset_of!(SignatureElement, name)),
                field(offset_of!(SignatureElement, index)),
                field(offset_of!(SignatureElement, system)),
                field(offset_of!(SignatureElement, register)),
                chunk.get(at + offset_of!(SignatureElement, mask)),
            ) else {
                break;
            };
            let semantic = chunk
                .get(name as usize..)
                .and_then(|s| s.split(|&c| c == 0).next())
                .map(|s| alloc::string::String::from_utf8_lossy(s).into_owned())
                .unwrap_or_default();
            out.push(Element {
                semantic,
                index,
                system,
                register,
                mask,
            });
        }
    }
    out
}

/// The input (`ISGN`, `ISG1`) or output (`OSGN`, `OSG5`, `OSG1`) signature of a DXBC container.
pub(crate) fn dxbc_signature(container: &[u8], input: bool) -> alloc::vec::Vec<Element> {
    let tags = if input {
        &[ISGN, ISG1][..]
    } else {
        &[OSGN, OSG5, OSG1][..]
    };
    tags.iter().flat_map(|t| signature(container, t)).collect()
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
