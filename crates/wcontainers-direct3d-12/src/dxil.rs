//! What a DXIL shader declares that a pipeline needs before converting it: its typed unordered
//! access views and typed buffers, and its multisampled array textures (as `dxbc` reads them from
//! DXBC). Read from the
//! module's resource list (`!dx.resources`) in the DXIL chunk's LLVM 3.7 bitcode, laid out as
//! Microsoft's DirectXShaderCompiler documents in `DXIL.rst` ("Metadata", "Resources").

use crate::dxbc::{Kind, TypedView, chunk};
use llvm_bitcode::bitcode::{Payload, Record};
use llvm_bitcode::{BitStreamVisitor, Bitcode};

/// `MODULE_BLOCK_ID`, `CONSTANTS_BLOCK_ID` and `METADATA_BLOCK_ID`.
const MODULE: u32 = 8;
const CONSTANTS: u32 = 11;
const METADATA: u32 = 15;

/// `DXIL::ResourceKind::TypedBuffer`; the texture kinds lie below it.
const TYPED_BUFFER: i64 = 10;
/// `DXIL::ResourceKind::Texture2DMSArray`.
const TEXTURE_2D_MS_ARRAY: i64 = 8;

/// A resource record's fields (`DXIL.rst`, "Resources"): id, variable and name come first.
const SPACE: usize = 3;
const LOWER_BOUND: usize = 4;
const RANGE_SIZE: usize = 5;
const KIND: usize = 6;
/// The extended properties: after the kind, a shader resource view's sample count; an unordered
/// access view's globally coherent, counter and rasterizer ordered flags.
const SRV_EXTENDED: usize = 8;
const UAV_EXTENDED: usize = 10;

/// A module-level metadata entry, as far as the resource list needs.
enum Md {
    /// A node's operands (`None` for a null operand).
    Node(Vec<Option<usize>>),
    /// A constant: the module value it names.
    Value(usize),
    Other,
}

/// Collects the module's values (integer constants by value, the rest as `None`), its metadata
/// entries and its named metadata.
#[derive(Default)]
struct Reader {
    /// The blocks entered, innermost last.
    stack: Vec<u32>,
    values: Vec<Option<i64>>,
    metadata: Vec<Md>,
    /// The name a `METADATA_NAME` record gave the next named node.
    name: Vec<u8>,
    resources: Option<Vec<usize>>,
}

/// A record's operands, its array payload included.
fn operands(mut r: Record) -> Vec<u64> {
    let mut v = r.fields().to_vec();
    match r.take_payload() {
        Some(Payload::Array(a)) => v.extend(a),
        Some(Payload::Char6String(s)) => v.extend(s.bytes().map(u64::from)),
        _ => {}
    }
    v
}

impl BitStreamVisitor for Reader {
    fn should_enter_block(&mut self, id: u32) -> bool {
        let enter = match self.stack.last() {
            None => id == MODULE,
            Some(&MODULE) => id == CONSTANTS || id == METADATA,
            Some(_) => false,
        };
        if enter {
            self.stack.push(id);
        }
        enter
    }

    fn did_exit_block(&mut self, _: u32) {
        self.stack.pop();
    }

    fn visit(&mut self, block: u32, record: Record) {
        let code = record.id;
        match block {
            // Global variables, functions and aliases each define one value, in order.
            MODULE if matches!(code, 7 | 8 | 9 | 14) => self.values.push(None),
            // Every record but `SETTYPE` defines one constant: `NULL` (zero) and `INTEGER` (a
            // signed VBR value) are the integers.
            CONSTANTS if code != 1 => {
                let value = match code {
                    2 => Some(0),
                    4 => record.fields().first().map(|&v| match v & 1 {
                        0 => (v >> 1) as i64,
                        _ => -((v >> 1) as i64),
                    }),
                    _ => None,
                };
                self.values.push(value);
            }
            METADATA => match code {
                // `NAME`, then the `NAMED_NODE` it names (its operands not offset by one).
                4 => self.name = operands(record).into_iter().map(|c| c as u8).collect(),
                10 => {
                    if self.name == b"dx.resources" {
                        let ids = operands(record).into_iter().map(|i| i as usize);
                        self.resources = Some(ids.collect());
                    }
                }
                // `KIND` and `ATTACHMENT` define no entry.
                6 | 11 => {}
                // `VALUE`: [type, value].
                2 => {
                    let v = operands(record);
                    self.metadata.push(match v.get(1) {
                        Some(&id) => Md::Value(id as usize),
                        None => Md::Other,
                    });
                }
                // `NODE` and `DISTINCT_NODE`: operands are entries plus one, zero for null.
                3 | 5 => {
                    let ops = operands(record).into_iter();
                    let ops = ops.map(|i| (i as usize).checked_sub(1)).collect();
                    self.metadata.push(Md::Node(ops));
                }
                _ => self.metadata.push(Md::Other),
            },
            _ => {}
        }
    }
}

impl Reader {
    fn node(&self, id: Option<usize>) -> Option<&[Option<usize>]> {
        match self.metadata.get(id?)? {
            Md::Node(ops) => Some(ops),
            _ => None,
        }
    }

    fn integer(&self, id: Option<usize>) -> Option<i64> {
        match self.metadata.get(id?)? {
            Md::Value(v) => *self.values.get(*v)?,
            _ => None,
        }
    }

    /// The component kind a resource's extended properties give (tag 0, the element type).
    fn kind(&self, extended: Option<usize>) -> Option<Kind> {
        let tags = self.node(extended)?;
        let at = tags.chunks(2).position(|p| self.integer(p[0]) == Some(0))?;
        // `DXIL::ComponentType`.
        match self.integer(*tags.get(at * 2 + 1)?)? {
            1 | 2 | 4 | 6 => Some(Kind::Sint),
            3 | 5 | 7 => Some(Kind::Uint),
            8..=16 => Some(Kind::Float),
            _ => None,
        }
    }

    /// The resources of one list of the resource list: each one's fields, its resource kind and
    /// the (space, register) places of its elements, with every place of the list sorted for the
    /// slots.
    fn declared(&self, list: Option<usize>) -> Option<Declarations<'_>> {
        let resources = self.node(list).unwrap_or_default();
        let mut declared = Vec::with_capacity(resources.len());
        let mut places = Vec::with_capacity(resources.len());
        for &resource in resources {
            let fields = self.node(resource)?;
            let integer = |at: usize| self.integer(*fields.get(at)?);
            let space = u32::try_from(integer(SPACE)?).ok()?;
            let first = u32::try_from(integer(LOWER_BOUND)?).ok()?;
            // An unbounded range is one place: the converter refuses such arrays.
            let count = u32::try_from(integer(RANGE_SIZE)?)
                .ok()
                .filter(|&c| c != u32::MAX)
                .unwrap_or(1);
            let elements: Vec<(u32, u32)> = (0..count)
                .map(|i| Some((space, first.checked_add(i)?)))
                .collect::<Option<_>>()?;
            places.extend_from_slice(&elements);
            declared.push((fields, integer(KIND)?, elements));
        }
        Some((declared, sorted_places(&places)))
    }

    /// The typed views of one list of the resource list: shader resource views (`storage`
    /// false, typed buffers only) or unordered access views (textures and typed buffers), one
    /// per element of an array, as the converter declares a view per element.
    fn views(&self, list: Option<usize>, storage: bool, out: &mut Vec<TypedView>) -> Option<()> {
        let extended = if storage { UAV_EXTENDED } else { SRV_EXTENDED };
        let (declared, all) = self.declared(list)?;
        for (fields, kind, elements) in declared {
            let typed = match storage {
                true => (1..=TYPED_BUFFER).contains(&kind),
                false => kind == TYPED_BUFFER,
            };
            if !typed {
                continue;
            }
            let component = self.kind(*fields.get(extended)?)?;
            out.extend(elements.into_iter().map(|place| TypedView {
                storage,
                buffer: kind == TYPED_BUFFER,
                slot: slot(&all, place),
                kind: component,
            }));
        }
        Some(())
    }
}

/// A resource of the resource list: its fields, its resource kind and its elements' places.
type Declared<'a> = (&'a [Option<usize>], i64, Vec<(u32, u32)>);

/// A list's resources, and every place of the list sorted.
type Declarations<'a> = (Vec<Declared<'a>>, Vec<(u32, u32)>);

/// A class's (space, register) places, sorted, each once.
pub(crate) fn sorted_places(places: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut all = places.to_vec();
    all.sort_unstable();
    all.dedup();
    all
}

/// The slot of the view at `place` under Direct3D 12's numbering: its place among its class's
/// views in (space, register) order, as the converter numbers them.
pub(crate) fn slot(all: &[(u32, u32)], place: (u32, u32)) -> u32 {
    all.partition_point(|&p| p < place) as u32
}

/// The module's resource list read from `code`'s DXIL chunk, with the node of its lists; `None`
/// where `code` holds no DXIL chunk or its bitcode is malformed.
fn resource_lists(code: &[u8]) -> Option<(Reader, Option<usize>)> {
    let program = chunk(code, *b"DXIL")?;
    // The program header (version, size), then the bitcode header (magic, version, the bitcode's
    // offset from that header, its size).
    let word = |at: usize| -> Option<usize> {
        Some(u32::from_le_bytes(program.get(at..at + 4)?.try_into().ok()?) as usize)
    };
    let start = 8 + word(16)?;
    let bitcode = program.get(start..start + word(20)?)?;
    let mut reader = Reader::default();
    Bitcode::read(bitcode, &mut reader).ok()?;
    let lists = reader.resources.as_ref().and_then(|r| r.first()).copied();
    Some((reader, lists))
}

/// The typed unordered access views and typed buffers `code` declares (shader resource views of
/// textures left out); `None` where `code` holds no DXIL chunk or its resource list is malformed.
pub fn typed_views(code: &[u8]) -> Option<Vec<TypedView>> {
    let (reader, list) = resource_lists(code)?;
    let mut views = Vec::new();
    if let Some(list) = list {
        let lists = reader.node(Some(list))?;
        reader.views(*lists.first()?, false, &mut views)?;
        reader.views(*lists.get(1)?, true, &mut views)?;
    }
    Some(views)
}

/// The texture slots of the multisampled array textures (`Texture2DMSArray`) `code` declares;
/// `None` where it holds no DXIL chunk or its resource list is malformed.
pub fn multisampled_arrays(code: &[u8]) -> Option<Vec<u32>> {
    let (reader, list) = resource_lists(code)?;
    let Some(list) = list else {
        return Some(Vec::new());
    };
    let (declared, all) = reader.declared(*reader.node(Some(list))?.first()?)?;
    Some(
        declared
            .into_iter()
            .filter(|d| d.1 == TEXTURE_2D_MS_ARRAY)
            .flat_map(|d| d.2)
            .map(|p| slot(&all, p))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(storage: bool, buffer: bool, slot: u32, kind: Kind) -> TypedView {
        TypedView {
            storage,
            buffer,
            slot,
            kind,
        }
    }

    #[test]
    fn typed_buffers_are_read_from_the_resource_list() {
        // dxc's cs_6_0 of the shaders crate's texels.hlsl: Buffer<float4> t0, Buffer<uint4> t1,
        // Buffer<int4> t2, Buffer<float2> t3, RWBuffer<float4> u0, RWBuffer<uint> u1 and a
        // structured buffer at u2 (left out).
        let code = include_bytes!("../../wcontainers-shaders/tests/own/cs_texels.dxil");
        let mut views = typed_views(code).unwrap();
        views.sort_by_key(|v| (v.storage, v.slot));
        assert_eq!(
            views,
            [
                view(false, true, 0, Kind::Float),
                view(false, true, 1, Kind::Uint),
                view(false, true, 2, Kind::Sint),
                view(false, true, 3, Kind::Float),
                view(true, true, 0, Kind::Float),
                view(true, true, 1, Kind::Uint),
            ]
        );
    }

    #[test]
    fn storage_textures_are_read_from_the_resource_list() {
        // compute.hlsl's cs_images: RWTexture2D<float4> u0, RWTexture2D<float> u1,
        // RWTexture2DArray<uint4> u2, RWTexture2D<int2> u3.
        let code = include_bytes!("../../wcontainers-shaders/tests/own/cs_images.dxil");
        let mut views = typed_views(code).unwrap();
        views.sort_by_key(|v| v.slot);
        assert_eq!(
            views,
            [
                view(true, false, 0, Kind::Float),
                view(true, false, 1, Kind::Float),
                view(true, false, 2, Kind::Uint),
                view(true, false, 3, Kind::Sint),
            ]
        );
    }

    #[test]
    fn arrays_of_storage_textures_are_a_view_per_element() {
        // arrays.hlsl's cs_image_array: RWTexture2D<uint> images[4] at u0.
        let code = include_bytes!("../../wcontainers-shaders/tests/own/cs_image_array.dxil");
        let mut views = typed_views(code).unwrap();
        views.sort_by_key(|v| v.slot);
        let elements: Vec<_> = (0..4).map(|s| view(true, false, s, Kind::Uint)).collect();
        assert_eq!(views, elements);
    }

    #[test]
    fn multisampled_arrays_are_read_from_the_resource_list() {
        // msarray.hlsl's ps_msarray: Texture2DMSArray<float4> t0.
        let code = include_bytes!("../../wcontainers-shaders/tests/own/ps_msarray.dxil");
        assert_eq!(multisampled_arrays(code), Some(vec![0]));
        let code = include_bytes!("../../wcontainers-shaders/tests/own/cs_texels.dxil");
        assert_eq!(multisampled_arrays(code), Some(vec![]));
    }

    #[test]
    fn slots_follow_space_then_register() {
        // u5 and u0 in space 1, u7 in space 0, u5 again in space 1 (a view and its counter).
        let all = sorted_places(&[(1, 5), (1, 0), (0, 7), (1, 5)]);
        let slots: Vec<u32> = [(0, 7), (1, 0), (1, 5)].map(|p| slot(&all, p)).to_vec();
        assert_eq!(slots, [0, 1, 2]);
    }

    #[test]
    fn dxbc_has_no_resource_list() {
        let code = include_bytes!("../../wcontainers-shaders/tests/own/cs_images.dxbc");
        assert_eq!(typed_views(code), None);
    }
}
