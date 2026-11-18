//! DXBC handed to the device: its stage from the container, and its conversion through the shaders
//! crate into the common form.

use wcontainers_gpu::{
    AddedBinding, CLAMPED_SAMPLER_BINDING, DrawBaseSource, Format, FormatKind, HELPER_SET,
    HelperSlot, HelperValue, LinkedValues, SlotKind, SourceStage, TexelBufferRoute, VariantFlags,
    VariantKey,
};
use wcontainers_shaders::{ConvertError, Converted, Source};

use crate::error::{Error, Result, not_implemented};

/// The stage a DXBC container's program is written for, read from the version token of its `SHDR` or
/// `SHEX` chunk; an error if the bytes are not such a container.
pub fn stage(bytecode: &[u8]) -> Result<SourceStage> {
    let bad = Error::InvalidArg("not DXBC bytecode");
    let (_, at) = chunks(bytecode)?
        .into_iter()
        .find(|(tag, _)| tag == b"SHDR" || tag == b"SHEX")
        .ok_or(bad)?;
    match word(bytecode, at + 8)? >> 16 {
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
    for (tag, at) in chunks(bytecode)? {
        match &tag {
            b"SHDR" | b"SHEX" => version = Some(word(bytecode, at + 8)? & 0xff),
            b"Aon9" => aon9 = true,
            _ => {}
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

/// The word at byte `at` of `bytecode`.
fn word(bytecode: &[u8], at: usize) -> Result<u32> {
    bytecode
        .get(at..)
        .and_then(|b| b.get(..4))
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .ok_or(Error::InvalidArg("not DXBC bytecode"))
}

/// The tags of a DXBC container's chunks and where each starts.
fn chunks(bytecode: &[u8]) -> Result<Vec<([u8; 4], usize)>> {
    if bytecode.len() < 32
        || &bytecode[..4] != b"DXBC"
        || word(bytecode, 24)? as usize > bytecode.len()
    {
        return Err(Error::InvalidArg("not DXBC bytecode"));
    }
    (0..word(bytecode, 28)? as usize)
        .map(|i| {
            let at = word(bytecode, 32 + 4 * i)? as usize;
            let tag = bytecode
                .get(at..at.saturating_add(4))
                .ok_or(Error::InvalidArg("not DXBC bytecode"))?;
            Ok((tag.try_into().unwrap(), at))
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
    /// A storage texture read and written through a buffer of its texels, where the back end
    /// has no atomics on storage textures (`AtomicsThroughBuffer`).
    pub through: bool,
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

/// The typed views a DXBC container's program declares, in the token format of Microsoft's
/// `d3d11TokenizedProgramFormat.hpp`.
pub fn typed_views(bytecode: &[u8]) -> Result<Vec<TypedView>> {
    const DCL_RESOURCE: u32 = 88;
    const DCL_UAV_TYPED: u32 = 156;
    const CUSTOMDATA: u32 = 53;
    const DIMENSION_BUFFER: u32 = 1;
    let bad = Error::InvalidArg("malformed DXBC program");
    let (_, at) = chunks(bytecode)?
        .into_iter()
        .find(|(tag, _)| tag == b"SHDR" || tag == b"SHEX")
        .ok_or(bad)?;
    let words = word(bytecode, at + 4)? as usize / 4;
    let token = |i: usize| word(bytecode, at + 8 + 4 * i);
    let mut views = Vec::new();
    // Past the version and length tokens.
    let mut i = 2;
    while i < words {
        let opcode = token(i)?;
        let length = match opcode & 0x7ff {
            CUSTOMDATA => token(i + 1)? as usize,
            _ => (opcode >> 24 & 0x7f) as usize,
        };
        if length == 0 || i + length > words {
            return Err(bad);
        }
        let dimension = opcode >> 11 & 0x1f;
        let storage = match opcode & 0x7ff {
            DCL_RESOURCE if dimension == DIMENSION_BUFFER => Some(false),
            DCL_UAV_TYPED => Some(true),
            _ => None,
        };
        if let Some(storage) = storage {
            // The opcode's extended tokens, the operand and its own, its indices (one immediate in
            // shader model 5.0; the identifier and the range's bounds in 5.1), the return type.
            let mut j = i + 1;
            let mut extended = opcode >> 31 != 0;
            while extended {
                extended = token(j)? >> 31 != 0;
                j += 1;
            }
            let operand = token(j)?;
            j += 1;
            let mut extended = operand >> 31 != 0;
            while extended {
                extended = token(j)? >> 31 != 0;
                j += 1;
            }
            let indices = (operand >> 20 & 3) as usize;
            let register = match indices {
                1 => token(j)?,
                3 => token(j + 1)?,
                _ => return Err(bad),
            };
            let kind = match token(j + indices)? & 0xf {
                1 | 2 | 5 => FormatKind::Float,
                3 => FormatKind::Sint,
                4 => FormatKind::Uint,
                _ => return Err(bad),
            };
            views.push(TypedView {
                storage,
                register,
                buffer: dimension == DIMENSION_BUFFER,
                kind,
                through: false,
            });
        }
        i += length;
    }
    Ok(views)
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
}

/// The input signature (`ISGN`, or `ISG1` of shader model 5.1) of a DXBC container; empty where it
/// has none. An error if the bytes are not such a container or the chunk is malformed.
pub fn input_signature(bytecode: &[u8]) -> Result<Vec<SignatureElement>> {
    signature(bytecode, &[(*b"ISGN", 24, false), (*b"ISG1", 32, true)])
}

/// The output signature (`OSGN`, `OSG5` of geometry shaders with streams, or `OSG1`).
pub fn output_signature(bytecode: &[u8]) -> Result<Vec<SignatureElement>> {
    signature(
        bytecode,
        &[
            (*b"OSGN", 24, false),
            (*b"OSG5", 28, true),
            (*b"OSG1", 32, true),
        ],
    )
}

/// The first chunk of `tags` (tag, bytes per element, whether each starts with its stream).
fn signature(bytecode: &[u8], tags: &[([u8; 4], usize, bool)]) -> Result<Vec<SignatureElement>> {
    let bad = Error::InvalidArg("not DXBC bytecode");
    let word = |at: usize| -> Result<u32> {
        bytecode
            .get(at..)
            .and_then(|b| b.get(..4))
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .ok_or(bad)
    };
    if bytecode.len() < 32 || &bytecode[..4] != b"DXBC" {
        return Err(bad);
    }
    for i in 0..word(28)? as usize {
        let at = word(32 + 4 * i)? as usize;
        let tag = bytecode.get(at..).and_then(|b| b.get(..4)).ok_or(bad)?;
        let Some(&(_, stride, streams)) = tags.iter().find(|t| t.0 == tag) else {
            continue;
        };
        let first = if streams { 4 } else { 0 };
        let data = at + 8;
        let mut elements = Vec::new();
        for e in 0..word(data)? as usize {
            let start = data + 8 + e * stride;
            let base = start + first;
            let name_at = data + word(base)? as usize;
            let name = bytecode
                .get(name_at..)
                .and_then(|b| b.split(|&c| c == 0).next())
                .ok_or(bad)?;
            elements.push(SignatureElement {
                semantic: String::from_utf8_lossy(name).into_owned(),
                index: word(base + 4)?,
                system: word(base + 8)?,
                register: word(base + 16)?,
                mask: word(base + 20)? as u8,
                stream: if streams { word(start)? } else { 0 },
            });
        }
        return Ok(elements);
    }
    Ok(Vec::new())
}

/// Direct3D's system value names of the vertex and instance ids, which no input element feeds.
const VERTEX_ID: u32 = 6;
const INSTANCE_ID: u32 = 8;

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
    /// Depth is clamped in the programs (`DepthClampInShader`), their added value at this
    /// location; zero where the back end clamps.
    pub linked: u32,
}

impl StageVariant {
    /// The shaders crate's key for this variant of a shader of `stage` declaring `typed`, whose
    /// bound views have `formats`.
    pub fn key(self, stage: SourceStage, typed: &[TypedView], formats: &[Format]) -> VariantKey {
        let mut key = base_key(stage, typed);
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
        key.border_in_shader = self.border_in_shader;
        key.mirror_once_in_shader = self.mirror_once_in_shader;
        key.filter_in_shader = self.filter_in_shader;
        key.min_lod_clamped = self.min_lod_clamped;
        key.samples = self.samples;
        if self.dual_source {
            key.flags |= VariantFlags::DUAL_SOURCE;
        }
        if self.depth_bias {
            key.flags |= VariantFlags::DEPTH_BIAS;
        }
        if self.from_arguments {
            key.draw_base = DrawBaseSource::Arguments;
        }
        if self.linked != 0 {
            key.flags |= VariantFlags::DEPTH_CLAMP;
            key.linked = LinkedValues {
                location: self.linked,
                clip_distances: 0,
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
        Helpers {
            size: c.reflection.helper_size,
            values: c.reflection.helpers.clone(),
            merged,
            pulled,
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
) -> Result<(Converted, bool)> {
    let mut samples = false;
    loop {
        let variant = StageVariant {
            samples: u8::from(samples),
            ..StageVariant::default()
        };
        match raw(bytecode, expected, &variant.key(expected, typed, &[]))? {
            Err(ConvertError::Unsupported("SampleCount")) if !samples => samples = true,
            Err(ConvertError::Unsupported("StorageImageAtomics"))
                if typed.iter().any(|t| t.storage && !t.buffer && !t.through) =>
            {
                for t in typed.iter_mut().filter(|t| t.storage && !t.buffer) {
                    t.through = true;
                }
            }
            r => return Ok((answer(r, 1)?, samples)),
        }
    }
}

/// The variant every shader of `stage` declaring `typed` is converted as: vertex shaders read the
/// draw's bases from the helper constants, since `SV_VertexID` and `SV_InstanceID` leave them out
/// (programs that read neither get no helper block); typed views take their placeholders, typed
/// buffers read through storage buffers (`TexelBufferAsStorage`).
pub fn base_key(stage: SourceStage, typed: &[TypedView]) -> VariantKey {
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
        texel_buffers: if typed.iter().any(|t| t.buffer) {
            TexelBufferRoute::Storage
        } else {
            TexelBufferRoute::Native
        },
        texels_through_buffer: typed
            .iter()
            .filter(|t| t.through && t.register < 64)
            .fold(0, |m, t| m | 1 << t.register),
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
                )
            }) =>
        {
            not_implemented("shaders that read helper constants the front does not write")
        }
        Ok(c) => Ok(c),
        Err(ConvertError::Rejected(why)) => {
            eprintln!("wcontainers direct3d 11: shader refused by the converter: {why}");
            Err(Error::InvalidArg("bytecode the converter refuses"))
        }
        Err(ConvertError::Unsupported(what)) => not_implemented(what),
        Err(e) => {
            eprintln!("wcontainers direct3d 11: shader conversion failed: {e}");
            not_implemented("shaders the converter does not yet turn into a valid module")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                    through: false,
                },
                TypedView {
                    storage: false,
                    register: 1,
                    buffer: true,
                    kind: FormatKind::Float,
                    through: false,
                },
            ]
        );
        let key = base_key(SourceStage::Compute, &views);
        assert_eq!(key.storage_formats, [(2, Format::R32Uint)]);
        assert_eq!(key.texel_formats, [(1, Format::Rgba32Float)]);
        assert_eq!(key.texel_buffers, TexelBufferRoute::Storage);
        let bound = StageVariant::default().key(
            SourceStage::Compute,
            &views,
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
}
