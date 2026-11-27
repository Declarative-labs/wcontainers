//! What a DXBC shader declares that a pipeline needs before converting it: its typed unordered
//! access views and typed buffers, whose formats the converted program takes from the bound views
//! (`VariantKey::storage_formats`, `VariantKey::texel_formats`), and its multisampled array
//! textures, whose layer counts it takes from them (`VariantKey::texture_layers`). Read from the program chunk's
//! declarations, in the token format of Microsoft's `d3d11TokenizedProgramFormat.hpp`.

use std::ops::Range;
use wcontainers_gpu::SlotKind;
use wcontainers_shaders::container::{
    self, Instruction, Tag, dxbc_dimension, dxbc_op, dxbc_sampled, token,
};

/// The kind of value a typed view's declaration reads and writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Float,
    Uint,
    Sint,
}

/// A typed view a shader declares, or an array of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedView {
    /// An unordered access view (a storage slot), else a shader resource view (a texture slot).
    pub storage: bool,
    /// A typed buffer, else a texture.
    pub buffer: bool,
    /// Its elements' places in their kind's range (`ReflectedBinding::slot`), consecutive, never
    /// none: the registers under Direct3D 11's numbering (DXBC), their places among the program's
    /// views of their class in (space, register) order under Direct3D 12's (DXIL).
    pub slots: Range<u32>,
    pub kind: Kind,
}

use dxbc_op::{
    DCL_RESOURCE, DCL_RESOURCE_RAW, DCL_RESOURCE_STRUCTURED, DCL_SAMPLER, DCL_UAV_RAW,
    DCL_UAV_STRUCTURED, DCL_UAV_TYPED,
};

/// An operand's index dimension with one index (a register, shader model 5.0) and with three
/// (an identifier and the range's bounds, shader model 5.1, whose declarations end in their space).
const ONE_INDEX: u32 = 1;
const RANGE_INDICES: u32 = 3;

/// One view declaration: an unordered access view (else a shader resource view), its (space,
/// register) place, its element count (none for an array without a length), its dimension and
/// its opcode.
struct Declared {
    storage: bool,
    place: (u32, u32),
    length: Option<u32>,
    dimension: u32,
    opcode: u32,
    /// Its return type's x component, for typed views.
    kind: Option<Kind>,
}

/// Every view `code` declares and whether they are numbered as Direct3D 12 numbers them (shader
/// model 5.1); `None` where it holds no DXBC program chunk or its tokens are malformed.
fn views(code: &[u8]) -> Option<(Vec<Declared>, bool)> {
    let mut views = Vec::new();
    let mut spaces = false;
    declarations(code, |opcode, instruction| {
        let storage = match token::opcode(opcode) {
            DCL_UAV_TYPED | DCL_UAV_RAW | DCL_UAV_STRUCTURED => true,
            DCL_RESOURCE | DCL_RESOURCE_RAW | DCL_RESOURCE_STRUCTURED => false,
            _ => return Some(()),
        };
        let (place, length, ranged, kind) = view(instruction)?;
        spaces |= ranged;
        views.push(Declared {
            storage,
            place,
            length,
            dimension: token::resource_dimension(opcode),
            opcode: token::opcode(opcode),
            kind,
        });
        Some(())
    })?;
    Some((views, spaces))
}

/// The slots of the elements of each of `views` (an array without a length takes the rest of the
/// range of `ranges` holding its first register, one element where none does; an array ends at
/// the last register), consecutive: the registers under Direct3D 11's numbering, the places among
/// their kind's elements in (space, register) order under Direct3D 12's (`spaces`), as the
/// converter numbers them. A range is kept as its first slot and its end, however long.
fn element_slots(
    views: &[Declared],
    spaces: bool,
    ranges: &[(SlotKind, u32, Range<u32>)],
) -> Vec<Range<u32>> {
    let elements = |v: &Declared| {
        let (space, register) = v.place;
        let kind = if v.storage {
            SlotKind::Storage
        } else {
            SlotKind::Texture
        };
        let length = v.length.unwrap_or_else(|| {
            ranges
                .iter()
                .find(|(k, s, r)| *k == kind && *s == space && r.contains(&register))
                .map_or(1, |(.., r)| r.end - register)
        });
        (space, register, register.saturating_add(length) - register)
    };
    let all = |storage: bool| {
        let class = views.iter().filter(|v| v.storage == storage);
        crate::dxil::Places::of(class.map(elements))
    };
    let (uavs, srvs) = (all(true), all(false));
    views
        .iter()
        .map(|v| match (spaces, elements(v)) {
            (true, e) => (if v.storage { &uavs } else { &srvs }).slots(e),
            (false, (_, register, count)) => register..register + count,
        })
        .collect()
}

/// The typed unordered access views and typed buffers `code` declares (shader resource views of
/// textures left out), with a slot per element of an array, whose unbounded arrays `ranges` sizes
/// (an array of no elements left out);
/// `None` where `code` holds no DXBC program chunk or its tokens are malformed.
pub fn typed_views(code: &[u8], ranges: &[(SlotKind, u32, Range<u32>)]) -> Option<Vec<TypedView>> {
    let (views, spaces) = views(code)?;
    let slots = element_slots(&views, spaces, ranges);
    let mut out = Vec::new();
    for (v, slots) in views.iter().zip(slots) {
        let buffer = v.dimension == dxbc_dimension::BUFFER;
        let typed = match v.opcode {
            DCL_UAV_TYPED => true,
            DCL_RESOURCE => buffer,
            _ => false,
        };
        if !typed || slots.is_empty() {
            continue;
        }
        out.push(TypedView {
            storage: v.storage,
            buffer,
            slots,
            kind: v.kind?,
        });
    }
    Some(out)
}

/// A shader model 5.1 range's upper bound when it has none.
const UNBOUNDED_UPPER: u32 = u32::MAX;

/// Whether `code` declares an array of views or samplers without a length
/// (a shader model 5.1 range with no upper bound), which the root signature's range sizes.
pub fn declares_unbounded_array(code: &[u8]) -> bool {
    let mut found = false;
    let _ = declarations(code, |opcode, instruction| {
        let declares = matches!(
            token::opcode(opcode),
            DCL_RESOURCE
                | DCL_RESOURCE_RAW
                | DCL_RESOURCE_STRUCTURED
                | DCL_UAV_TYPED
                | DCL_UAV_RAW
                | DCL_UAV_STRUCTURED
                | DCL_SAMPLER
        );
        if declares && let Some((at, RANGE_INDICES)) = instruction.operand_indices() {
            found |= instruction.word(at + 2) == Some(UNBOUNDED_UPPER);
        }
        Some(())
    });
    found
}

/// The texture slots of the multisampled array textures (`Texture2DMSArray`) `code` declares, a
/// range per declaration with a slot per element of an array (none empty), whose unbounded arrays
/// `ranges` sizes; `None` where it holds no DXBC program chunk or its tokens are malformed.
pub fn multisampled_arrays(
    code: &[u8],
    ranges: &[(SlotKind, u32, Range<u32>)],
) -> Option<Vec<Range<u32>>> {
    let (views, spaces) = views(code)?;
    let slots = element_slots(&views, spaces, ranges);
    Some(
        views
            .iter()
            .zip(slots)
            .filter(|(v, slots)| {
                v.opcode == DCL_RESOURCE
                    && v.dimension == dxbc_dimension::TEXTURE_2DMS_ARRAY
                    && !slots.is_empty()
            })
            .map(|(_, slots)| slots)
            .collect(),
    )
}

/// Calls `f` with each instruction's opcode token and words; `None` where `code` holds no
/// program chunk, its tokens are malformed or `f` fails.
fn declarations(code: &[u8], mut f: impl FnMut(u32, Instruction) -> Option<()>) -> Option<()> {
    for instruction in container::instructions(program(code)?) {
        let instruction = instruction.ok()?;
        f(instruction.opcode_token(), instruction)?;
    }
    Some(())
}

/// One view declaration: the opcode, its extended tokens, the operand with its indices (one in
/// shader model 5.0; the identifier and the range's bounds in 5.1), then for typed views the
/// return type token, for structured ones the stride, and in 5.1 the space last. Gives the
/// (space, register) place, whether the operand names a range (5.1), and the return type's x
/// component where the declaration is typed and names one.
#[allow(clippy::type_complexity)]
fn view(instruction: Instruction) -> Option<((u32, u32), Option<u32>, bool, Option<Kind>)> {
    let (mut at, indices) = instruction.operand_indices()?;
    let (register, length, ranged) = match indices {
        ONE_INDEX => (instruction.word(at)?, Some(1), false),
        RANGE_INDICES => {
            let (lower, upper) = (instruction.word(at + 1)?, instruction.word(at + 2)?);
            // Wrapping as a release build did before: an upper bound below the lower is no
            // element.
            let length =
                (upper != UNBOUNDED_UPPER).then(|| upper.wrapping_sub(lower).wrapping_add(1));
            (lower, length, true)
        }
        _ => return None,
    };
    at += indices as usize;
    let space = match ranged {
        true => instruction.word(instruction.words() - 1)?,
        false => 0,
    };
    let kind = instruction
        .word(at)
        .and_then(|t| match token::return_type(t) {
            dxbc_sampled::UNORM | dxbc_sampled::SNORM | dxbc_sampled::FLOAT => Some(Kind::Float),
            dxbc_sampled::SINT => Some(Kind::Sint),
            dxbc_sampled::UINT => Some(Kind::Uint),
            _ => None,
        });
    Some(((space, register), length, ranged, kind))
}

/// The first chunk of a container whose tag `wanted` takes; `None` where the bytes are not a
/// container, or its chunk list or that chunk is cut short before it is found.
fn first_chunk(code: &[u8], wanted: impl Fn(Tag) -> bool) -> Option<&[u8]> {
    if !container::is_container(code) {
        return None;
    }
    for chunk in container::chunk_list(code).ok()? {
        let chunk = chunk.ok()?;
        if wanted(chunk.tag()?) {
            return chunk.contents();
        }
    }
    None
}

/// The bytes of a container's first chunk named `fourcc`.
pub(crate) fn chunk(code: &[u8], fourcc: Tag) -> Option<&[u8]> {
    first_chunk(code, |tag| tag == fourcc)
}

/// The program chunk (`SHEX` or `SHDR`) of a DXBC container: the first in its chunk list.
fn program(code: &[u8]) -> Option<&[u8]> {
    first_chunk(code, container::is_program)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A container holding one program chunk of `tokens`.
    fn container(tokens: &[u32]) -> Vec<u8> {
        let mut code = b"DXBC".to_vec();
        code.resize(28, 0);
        code.extend(1u32.to_le_bytes());
        code.extend(36u32.to_le_bytes());
        code.extend(b"SHEX");
        code.extend(((tokens.len() * 4) as u32).to_le_bytes());
        for t in tokens {
            code.extend(t.to_le_bytes());
        }
        code
    }

    fn view(storage: bool, buffer: bool, slot: u32, kind: Kind) -> TypedView {
        TypedView {
            storage,
            buffer,
            slots: one(slot),
            kind,
        }
    }

    /// The slots of one element at `slot`.
    fn one(slot: u32) -> Range<u32> {
        slot..slot + 1
    }

    #[test]
    fn typed_views_are_read_from_declarations() {
        // ps_5_0: dcl_uav_typed_texture2d (float,float,float,float) u1, a custom data block, a
        // typed buffer at u2, a 3D sint view at u3, a typed buffer at t4 (uint) and a 2D texture
        // at t5 (left out).
        let tex2d = DCL_UAV_TYPED | (3 << 11) | (4 << 24);
        let tex3d = DCL_UAV_TYPED | (5 << 11) | (4 << 24);
        let buffer = DCL_UAV_TYPED | (1 << 11) | (4 << 24);
        let srv_buffer = DCL_RESOURCE | (1 << 11) | (4 << 24);
        let srv_texture = DCL_RESOURCE | (3 << 11) | (4 << 24);
        let operand = 0x0010_1000 | (30 << 12);
        let srv = 0x0010_1000 | (7 << 12);
        let tokens = [
            0x50,
            0,
            tex2d,
            operand,
            1,
            0x5555,
            dxbc_op::CUSTOM_DATA,
            3,
            0,
            buffer,
            operand,
            2,
            0x4444,
            tex3d,
            operand,
            3,
            0x3333,
            srv_buffer,
            srv,
            4,
            0x4444,
            srv_texture,
            srv,
            5,
            0x5555,
        ];
        assert_eq!(
            typed_views(&container(&tokens), &[]),
            Some(vec![
                view(true, false, 1, Kind::Float),
                view(true, true, 2, Kind::Uint),
                view(true, false, 3, Kind::Sint),
                view(false, true, 4, Kind::Uint),
            ])
        );
    }

    #[test]
    fn multisampled_arrays_are_read_from_declarations() {
        // dcl_resource_texture2dmsarray (float,float,float,float) t2, dcl_resource_texture2dms
        // t3 (left out).
        let ms_array = DCL_RESOURCE | (9 << 11) | (4 << 24);
        let ms = DCL_RESOURCE | (4 << 11) | (4 << 24);
        let srv = 0x0010_1000 | (7 << 12);
        let tokens = [0x50, 0, ms_array, srv, 2, 0x5555, ms, srv, 3, 0x5555];
        assert_eq!(
            multisampled_arrays(&container(&tokens), &[]),
            Some(vec![one(2)])
        );
        assert_eq!(typed_views(&container(&tokens), &[]), Some(vec![]));
        let code = include_bytes!("../../../../shaders/tests/own/ps_msarray.dxbc");
        assert_eq!(multisampled_arrays(code, &[]), Some(vec![one(0)]));
    }

    #[test]
    fn shader_model_5_1_views_take_their_place_by_space_and_register() {
        // dcl_uav_typed_texture2darray (uint,uint,uint,uint) U0[4:4], space=0;
        // dcl_uav_raw U1[0:0], space=1; dcl_uav_typed_buffer (float) U2[2:2], space=0;
        // dcl_resource_buffer (sint) T0[7:7], space=3. Unordered access views in (space, register)
        // order: (0, 2), (0, 4), (1, 0); the shader resource view alone.
        let operand = 0x0030_1000 | (30 << 12);
        let srv = 0x0030_1000 | (7 << 12);
        let decl = DCL_UAV_TYPED | (8 << 11) | (7 << 24);
        let raw = DCL_UAV_RAW | (1 << 11) | (6 << 24);
        let buffer = DCL_UAV_TYPED | (1 << 11) | (7 << 24);
        let srv_buffer = DCL_RESOURCE | (1 << 11) | (7 << 24);
        let tokens = [
            0x51, 0, decl, operand, 0, 4, 4, 0x4444, 0, raw, operand, 1, 0, 0, 1, buffer, operand,
            2, 2, 2, 0x5555, 0, srv_buffer, srv, 0, 7, 7, 0x3333, 3,
        ];
        assert_eq!(
            typed_views(&container(&tokens), &[]),
            Some(vec![
                view(true, false, 1, Kind::Uint),
                view(true, true, 0, Kind::Float),
                view(false, true, 0, Kind::Sint),
            ])
        );
    }

    /// Shader model 5.1 tokens declaring a typed 2D texture of uint at `U0[lower:upper]` (no upper
    /// bound where `upper` is `UNBOUNDED_UPPER`), a typed uint buffer at `U1[5:5]` and a
    /// multisampled array texture at `T0[lower:upper]`, all in space 0.
    fn ranges(lower: u32, upper: u32) -> Vec<u8> {
        let operand = 0x0030_1000 | (30 << 12);
        let srv = 0x0030_1000 | (7 << 12);
        let texture = DCL_UAV_TYPED | (3 << 11) | (7 << 24);
        let buffer = DCL_UAV_TYPED | (1 << 11) | (7 << 24);
        let ms_array = DCL_RESOURCE | (9 << 11) | (7 << 24);
        container(&[
            0x51, 0, texture, operand, 0, lower, upper, 0x4444, 0, buffer, operand, 1, 5, 5,
            0x4444, 0, ms_array, srv, 0, lower, upper, 0x5555, 0,
        ])
    }

    /// The elements of a range of every register but the last, and of a range sized by the root
    /// signature's range of every register but the last, are two numbers each: read at once, in
    /// a few bytes.
    #[test]
    fn a_range_of_every_register_is_kept_as_two_numbers() {
        let uint = Kind::Uint;
        let every = 0..u32::MAX;
        let all = TypedView {
            slots: every.clone(),
            ..view(true, false, 0, uint)
        };
        let expected = Some(vec![all, view(true, true, 5, uint)]);
        // U0[0:4294967294]: 2^32 - 1 elements, ending at the last register.
        let bounded = ranges(0, u32::MAX - 1);
        assert_eq!(typed_views(&bounded, &[]), expected);
        assert_eq!(
            multisampled_arrays(&bounded, &[]),
            Some(vec![every.clone()])
        );
        // U0[0:...] with no stated length, in root signature ranges of 2^32 - 1 registers.
        let unbounded = ranges(0, UNBOUNDED_UPPER);
        assert!(declares_unbounded_array(&unbounded));
        let root = [
            (SlotKind::Storage, 0, every.clone()),
            (SlotKind::Texture, 0, every.clone()),
        ];
        assert_eq!(typed_views(&unbounded, &root), expected);
        assert_eq!(multisampled_arrays(&unbounded, &root), Some(vec![every]));
        // Outside any root signature range it is one element.
        assert_eq!(multisampled_arrays(&unbounded, &[]), Some(vec![one(0)]));
        // U0[1:0]: 2^32 elements counted modulo 2^32, none, as a release build counted them
        // before; the views beside it keep their places.
        let wrapped = ranges(1, 0);
        let buffer = view(true, true, 0, uint);
        assert_eq!(typed_views(&wrapped, &[]), Some(vec![buffer]));
        assert_eq!(multisampled_arrays(&wrapped, &[]), Some(vec![]));
    }
}
