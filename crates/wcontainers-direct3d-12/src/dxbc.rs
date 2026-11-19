//! What a DXBC shader declares that a pipeline needs before converting it: its typed unordered
//! access views and typed buffers, whose formats the converted program takes from the bound views
//! (`VariantKey::storage_formats`, `VariantKey::texel_formats`), and its multisampled array
//! textures, whose layer counts it takes from them (`VariantKey::texture_layers`). Read from the program chunk's
//! declarations, in the token format of Microsoft's `d3d11TokenizedProgramFormat.hpp`.

/// The kind of value a typed view's declaration reads and writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Float,
    Uint,
    Sint,
}

/// A typed view a shader declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypedView {
    /// An unordered access view (a storage slot), else a shader resource view (a texture slot).
    pub storage: bool,
    /// A typed buffer, else a texture.
    pub buffer: bool,
    /// Its place in its kind's range (`ReflectedBinding::slot`): the register under Direct3D 11's
    /// numbering (DXBC), its place among the program's views of its class in (space, register)
    /// order under Direct3D 12's (DXIL).
    pub slot: u32,
    pub kind: Kind,
}

/// `D3D10_SB_OPCODE_DCL_RESOURCE`.
const DCL_RESOURCE: u32 = 88;
/// `D3D11_SB_OPCODE_DCL_UNORDERED_ACCESS_VIEW_TYPED`, `..._RAW` and `..._STRUCTURED`.
const DCL_UAV_TYPED: u32 = 156;
const DCL_UAV_RAW: u32 = 157;
const DCL_UAV_STRUCTURED: u32 = 158;
/// `D3D11_SB_OPCODE_DCL_RESOURCE_RAW` and `..._STRUCTURED`.
const DCL_RESOURCE_RAW: u32 = 161;
const DCL_RESOURCE_STRUCTURED: u32 = 162;
/// `D3D10_SB_OPCODE_CUSTOMDATA`, whose length is the word after its opcode.
const CUSTOMDATA: u32 = 53;
/// `D3D10_SB_RESOURCE_DIMENSION_BUFFER` and `..._TEXTURE2DMSARRAY`.
const DIMENSION_BUFFER: u32 = 1;
const DIMENSION_2D_MS_ARRAY: u32 = 9;
/// An operand's index dimension with one index (a register, shader model 5.0) and with three
/// (an identifier and the range's bounds, shader model 5.1, whose declarations end in their space).
const ONE_INDEX: u32 = 1;
const RANGE_INDICES: u32 = 3;
/// `D3D10_SB_OPCODE_TYPE_MASK`.
const OPCODE_TYPE_MASK: u32 = 0x7ff;
/// `D3D10_SB_RESOURCE_DIMENSION_MASK` and its shift.
const RESOURCE_DIMENSION_SHIFT: u32 = 11;
const RESOURCE_DIMENSION_MASK: u32 = 0x1f;
/// The bit of an opcode or operand token saying an extended token follows.
const EXTENDED_SHIFT: u32 = 31;
/// `D3D10_SB_OPERAND_INDEX_DIMENSION_MASK` and its shift.
const INDEX_DIMENSION_SHIFT: u32 = 20;
const INDEX_DIMENSION_MASK: u32 = 3;
/// `D3D10_SB_RESOURCE_RETURN_TYPE_MASK` of the x component and the return types.
const RETURN_TYPE_MASK: u32 = 0xf;
const RETURN_TYPE_UNORM: u32 = 1;
const RETURN_TYPE_SNORM: u32 = 2;
const RETURN_TYPE_SINT: u32 = 3;
const RETURN_TYPE_UINT: u32 = 4;
const RETURN_TYPE_FLOAT: u32 = 5;

/// One view declaration: an unordered access view (else a shader resource view), its (space,
/// register) place, its dimension and its opcode.
struct Declared {
    storage: bool,
    place: (u32, u32),
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
        let storage = match opcode & OPCODE_TYPE_MASK {
            DCL_UAV_TYPED | DCL_UAV_RAW | DCL_UAV_STRUCTURED => true,
            DCL_RESOURCE | DCL_RESOURCE_RAW | DCL_RESOURCE_STRUCTURED => false,
            _ => return Some(()),
        };
        let (place, ranged, kind) = view(instruction)?;
        spaces |= ranged;
        views.push(Declared {
            storage,
            place,
            dimension: (opcode >> RESOURCE_DIMENSION_SHIFT) & RESOURCE_DIMENSION_MASK,
            opcode: opcode & OPCODE_TYPE_MASK,
            kind,
        });
        Some(())
    })?;
    Some((views, spaces))
}

/// The slot of each of `views`: its register under Direct3D 11's numbering, its place among its
/// kind's views in (space, register) order under Direct3D 12's (`spaces`), as the converter
/// numbers them.
fn slots(views: &[Declared], spaces: bool) -> Vec<u32> {
    let all = |storage: bool| {
        let places: Vec<_> = views
            .iter()
            .filter(|v| v.storage == storage)
            .map(|v| v.place)
            .collect();
        crate::dxil::sorted_places(&places)
    };
    let (uavs, srvs) = (all(true), all(false));
    views
        .iter()
        .map(|v| match spaces {
            true => crate::dxil::slot(if v.storage { &uavs } else { &srvs }, v.place),
            false => v.place.1,
        })
        .collect()
}

/// The typed unordered access views and typed buffers `code` declares (shader resource views of
/// textures left out); `None` where `code` holds no DXBC program chunk or its tokens are
/// malformed.
pub fn typed_views(code: &[u8]) -> Option<Vec<TypedView>> {
    let (views, spaces) = views(code)?;
    let slots = slots(&views, spaces);
    let typed = views.iter().zip(slots).filter_map(|(v, slot)| {
        let buffer = v.dimension == DIMENSION_BUFFER;
        let typed = match v.opcode {
            DCL_UAV_TYPED => true,
            DCL_RESOURCE => buffer,
            _ => false,
        };
        typed.then_some((v, buffer, slot))
    });
    typed
        .map(|(v, buffer, slot)| {
            Some(TypedView {
                storage: v.storage,
                buffer,
                slot,
                kind: v.kind?,
            })
        })
        .collect()
}

/// The texture slots of the multisampled array textures (`Texture2DMSArray`) `code` declares;
/// `None` where it holds no DXBC program chunk or its tokens are malformed.
pub fn multisampled_arrays(code: &[u8]) -> Option<Vec<u32>> {
    let (views, spaces) = views(code)?;
    let slots = slots(&views, spaces);
    Some(
        views
            .iter()
            .zip(slots)
            .filter(|(v, _)| v.opcode == DCL_RESOURCE && v.dimension == DIMENSION_2D_MS_ARRAY)
            .map(|(_, slot)| slot)
            .collect(),
    )
}

/// Calls `f` with each instruction's opcode token and words; `None` where `code` holds no
/// program chunk, its tokens are malformed or `f` fails.
fn declarations(code: &[u8], mut f: impl FnMut(u32, &[u32]) -> Option<()>) -> Option<()> {
    let tokens = program(code)?;
    // Past the version and length words.
    let mut at = 2;
    while at < tokens.len() {
        let opcode = tokens[at];
        let length = match opcode & OPCODE_TYPE_MASK {
            CUSTOMDATA => *tokens.get(at + 1)? as usize,
            _ => ((opcode >> 24) & 0x7f) as usize,
        };
        if length == 0 {
            return None;
        }
        f(opcode, tokens.get(at..at + length)?)?;
        at += length;
    }
    Some(())
}

/// One view declaration: the opcode, its extended tokens, the operand with its indices (one in
/// shader model 5.0; the identifier and the range's bounds in 5.1), then for typed views the
/// return type token, for structured ones the stride, and in 5.1 the space last. Gives the
/// (space, register) place, whether the operand names a range (5.1), and the return type's x
/// component where the declaration is typed and names one.
fn view(instruction: &[u32]) -> Option<((u32, u32), bool, Option<Kind>)> {
    let mut at = 1;
    let mut extended = instruction[0] >> EXTENDED_SHIFT != 0;
    while extended {
        extended = *instruction.get(at)? >> EXTENDED_SHIFT != 0;
        at += 1;
    }
    let operand = *instruction.get(at)?;
    at += 1;
    let mut extended = operand >> EXTENDED_SHIFT != 0;
    while extended {
        extended = *instruction.get(at)? >> EXTENDED_SHIFT != 0;
        at += 1;
    }
    // Every index is an immediate word.
    let indices = (operand >> INDEX_DIMENSION_SHIFT) & INDEX_DIMENSION_MASK;
    let (register, ranged) = match indices {
        ONE_INDEX => (*instruction.get(at)?, false),
        RANGE_INDICES => (*instruction.get(at + 1)?, true),
        _ => return None,
    };
    at += indices as usize;
    let space = match ranged {
        true => *instruction.last()?,
        false => 0,
    };
    let kind = instruction
        .get(at)
        .and_then(|t| match t & RETURN_TYPE_MASK {
            RETURN_TYPE_UNORM | RETURN_TYPE_SNORM | RETURN_TYPE_FLOAT => Some(Kind::Float),
            RETURN_TYPE_SINT => Some(Kind::Sint),
            RETURN_TYPE_UINT => Some(Kind::Uint),
            _ => None,
        });
    Some(((space, register), ranged, kind))
}

/// The bytes of a container's first chunk named `fourcc`.
pub(crate) fn chunk(code: &[u8], fourcc: [u8; 4]) -> Option<&[u8]> {
    let word = |at: usize| -> Option<usize> {
        Some(u32::from_le_bytes(code.get(at..at + 4)?.try_into().ok()?) as usize)
    };
    if code.get(..4)? != b"DXBC" {
        return None;
    }
    for i in 0..word(28)? {
        let start = word(32 + i * 4)?;
        if code.get(start..start + 4)? == fourcc {
            let size = word(start + 4)?;
            return code.get(start + 8..start + 8 + size);
        }
    }
    None
}

/// The program chunk (`SHEX` or `SHDR`) of a DXBC container as words.
fn program(code: &[u8]) -> Option<Vec<u32>> {
    let bytes = chunk(code, *b"SHEX").or_else(|| chunk(code, *b"SHDR"))?;
    Some(
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| u32::from_le_bytes(*w))
            .collect(),
    )
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
            slot,
            kind,
        }
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
            CUSTOMDATA,
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
            typed_views(&container(&tokens)),
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
        assert_eq!(multisampled_arrays(&container(&tokens)), Some(vec![2]));
        assert_eq!(typed_views(&container(&tokens)), Some(vec![]));
        let code = include_bytes!("../../wcontainers-shaders/tests/own/ps_msarray.dxbc");
        assert_eq!(multisampled_arrays(code), Some(vec![0]));
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
            typed_views(&container(&tokens)),
            Some(vec![
                view(true, false, 1, Kind::Uint),
                view(true, true, 0, Kind::Float),
                view(false, true, 0, Kind::Sint),
            ])
        );
    }
}
