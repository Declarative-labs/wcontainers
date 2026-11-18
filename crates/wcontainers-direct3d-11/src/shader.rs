//! DXBC handed to the device: its stage from the container, and its conversion through the shaders
//! crate into the common form.

use wcontainers_gpu::{
    AddedBinding, CLAMPED_SAMPLER_BINDING, DrawBaseSource, HELPER_SET, HelperSlot, HelperValue,
    SlotKind, SourceStage, VariantFlags, VariantKey,
};
use wcontainers_shaders::{ConvertError, Converted, Source};

use crate::error::{Error, Result, not_implemented};

/// The stage a DXBC container's program is written for, read from the version token of its `SHDR` or
/// `SHEX` chunk; an error if the bytes are not such a container.
pub fn stage(bytecode: &[u8]) -> Result<SourceStage> {
    let bad = Error::InvalidArg("not DXBC bytecode");
    let word = |at: usize| -> Result<u32> {
        bytecode
            .get(at..)
            .and_then(|b| b.get(..4))
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .ok_or(bad)
    };
    if bytecode.len() < 32 || &bytecode[..4] != b"DXBC" || word(24)? as usize > bytecode.len() {
        return Err(bad);
    }
    for i in 0..word(28)? as usize {
        let at = word(32 + 4 * i)? as usize;
        let tag = bytecode.get(at..).and_then(|b| b.get(..4)).ok_or(bad)?;
        if tag == b"SHDR" || tag == b"SHEX" {
            return match word(at + 8)? >> 16 {
                0 => Ok(SourceStage::Pixel),
                1 => Ok(SourceStage::Vertex),
                2 => Ok(SourceStage::Geometry),
                3 => Ok(SourceStage::Hull),
                4 => Ok(SourceStage::Domain),
                5 => Ok(SourceStage::Compute),
                _ => Err(bad),
            };
        }
    }
    Err(bad)
}

/// One element of a DXBC container's input signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignatureElement {
    pub semantic: String,
    pub index: u32,
    pub register: u32,
    /// Direct3D's system value name (`D3D_NAME`): 6 is the vertex id, 8 the instance id.
    pub system: u32,
}

/// The input signature (`ISGN`, or `ISG1` of shader model 5.1) of a DXBC container; empty where it
/// has none. An error if the bytes are not such a container or the chunk is malformed.
pub fn input_signature(bytecode: &[u8]) -> Result<Vec<SignatureElement>> {
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
        let (stride, first) = match tag {
            b"ISGN" => (24, 0),
            b"ISG1" => (32, 4),
            _ => continue,
        };
        let data = at + 8;
        let mut elements = Vec::new();
        for e in 0..word(data)? as usize {
            let base = data + 8 + e * stride + first;
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
}

impl StageVariant {
    /// The shaders crate's key for this variant of a shader of `stage`.
    pub fn key(self, stage: SourceStage) -> VariantKey {
        let mut key = base_key(stage);
        key.lod_bias = self.lod_bias;
        key.min_lod_clamped = self.min_lod_clamped;
        key.samples = self.samples;
        if self.dual_source {
            key.flags |= VariantFlags::DUAL_SOURCE;
        }
        if self.from_arguments {
            key.draw_base = DrawBaseSource::Arguments;
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
/// reflection is known.
pub fn convert(bytecode: &[u8], expected: SourceStage) -> Result<(Converted, bool)> {
    match raw(bytecode, expected, &base_key(expected))? {
        Err(ConvertError::Unsupported("SampleCount")) => {
            let key = StageVariant {
                samples: 1,
                ..StageVariant::default()
            }
            .key(expected);
            Ok((answer(raw(bytecode, expected, &key)?)?, true))
        }
        r => Ok((answer(r)?, false)),
    }
}

/// The variant every shader of `stage` is converted as: vertex shaders read the draw's bases from
/// the helper constants, since `SV_VertexID` and `SV_InstanceID` leave them out (programs that read
/// neither get no helper block).
pub fn base_key(stage: SourceStage) -> VariantKey {
    VariantKey {
        draw_base: match stage {
            SourceStage::Vertex => DrawBaseSource::Helpers,
            _ => DrawBaseSource::None,
        },
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
    answer(raw(bytecode, expected, key)?)
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
    Ok(wcontainers_shaders::convert(
        Source::Dxbc(bytecode),
        key,
        None,
    ))
}

/// The converter's answer in Direct3D's terms.
fn answer(r: core::result::Result<Converted, ConvertError>) -> Result<Converted> {
    match r {
        Ok(c) if c.modules.len() != 1 => {
            not_implemented("shaders converted into several program stages")
        }
        Ok(c)
            if c.reflection.helpers.iter().any(|h| {
                !matches!(
                    h.value,
                    HelperValue::DrawBase
                        | HelperValue::DrawArguments
                        | HelperValue::LodBias { .. }
                        | HelperValue::MinLod { .. }
                        | HelperValue::PulledStream { .. }
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
