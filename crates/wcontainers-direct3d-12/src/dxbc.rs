//! What a DXBC shader declares that a pipeline needs before converting it: its typed unordered
//! access views and typed buffers, whose formats the converted program takes from the bound views
//! (`VariantKey::storage_formats`, `VariantKey::texel_formats`). Read from the program chunk's
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
/// `D3D11_SB_OPCODE_DCL_UNORDERED_ACCESS_VIEW_TYPED`.
const DCL_UAV_TYPED: u32 = 156;
/// `D3D10_SB_OPCODE_CUSTOMDATA`, whose length is the word after its opcode.
const CUSTOMDATA: u32 = 53;
/// `D3D10_SB_RESOURCE_DIMENSION_BUFFER`.
const DIMENSION_BUFFER: u32 = 1;

/// The typed unordered access views and typed buffers `code` declares (shader resource views of
/// textures left out); `None` where `code` holds no DXBC program chunk or its tokens are
/// malformed.
pub fn typed_views(code: &[u8]) -> Option<Vec<TypedView>> {
    let tokens = program(code)?;
    let mut views = Vec::new();
    // Past the version and length words.
    let mut at = 2;
    while at < tokens.len() {
        let opcode = tokens[at];
        let length = match opcode & 0x7ff {
            CUSTOMDATA => *tokens.get(at + 1)? as usize,
            _ => ((opcode >> 24) & 0x7f) as usize,
        };
        if length == 0 {
            return None;
        }
        let instruction = tokens.get(at..at + length)?;
        let buffer = (opcode >> 11) & 0x1f == DIMENSION_BUFFER;
        let storage = match opcode & 0x7ff {
            DCL_UAV_TYPED => Some(true),
            DCL_RESOURCE if buffer => Some(false),
            _ => None,
        };
        if let Some(storage) = storage {
            let (register, kind) = typed_view(instruction)?;
            views.push(TypedView {
                storage,
                buffer,
                slot: register,
                kind,
            });
        }
        at += length;
    }
    Some(views)
}

/// One `dcl_uav_typed` or `dcl_resource`: the opcode, its extended tokens, the operand with its
/// indices (one in shader model 5.0; the identifier and the range's bounds in 5.1), then the
/// return type token.
fn typed_view(instruction: &[u32]) -> Option<(u32, Kind)> {
    let mut at = 1;
    let mut extended = instruction[0] >> 31 != 0;
    while extended {
        extended = *instruction.get(at)? >> 31 != 0;
        at += 1;
    }
    let operand = *instruction.get(at)?;
    at += 1;
    let mut extended = operand >> 31 != 0;
    while extended {
        extended = *instruction.get(at)? >> 31 != 0;
        at += 1;
    }
    // Every index is an immediate word.
    let register = match (operand >> 20) & 3 {
        1 => *instruction.get(at)?,
        3 => *instruction.get(at + 1)?,
        _ => return None,
    };
    at += ((operand >> 20) & 3) as usize;
    // The x component's `D3D10_SB_RESOURCE_RETURN_TYPE`.
    let kind = match *instruction.get(at)? & 0xf {
        1 | 2 | 5 => Kind::Float,
        3 => Kind::Sint,
        4 => Kind::Uint,
        _ => return None,
    };
    Some((register, kind))
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
    fn shader_model_5_1_views_name_their_range_start() {
        // dcl_uav_typed_texture2darray (uint,uint,uint,uint) U0[4:4], space=0.
        let operand = 0x0030_1000 | (30 << 12);
        let decl = DCL_UAV_TYPED | (8 << 11) | (7 << 24);
        let tokens = [0x51, 0, decl, operand, 0, 4, 4, 0x4444, 0];
        assert_eq!(
            typed_views(&container(&tokens)),
            Some(vec![view(true, false, 4, Kind::Uint)])
        );
    }
}
