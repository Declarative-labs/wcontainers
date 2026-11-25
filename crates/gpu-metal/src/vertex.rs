//! Streams of formats naga's vertex reads lack, widened on the host (slow path `VertexFormatConverted`).
//!
//! Such a stream is read back, each attribute widened into a format naga reads, and drawn from the copy.

use std::rc::Rc;

use wcontainers_gpu::{
    DrawCall, Gpu, GpuError, SlowPath, VertexAttribute, VertexFormat, VertexLayoutDesc,
    VertexStream,
};
use wcontainers_metal::Metal;

use crate::objects::{Shared, VertexLayoutBlock};
use crate::{MetalGpu, convert};

/// The bytes widened attributes start at multiples of, the alignment of naga's vertex reads.
///
/// https://docs.rs/naga/30.0.2/naga/back/msl/struct.AttributeMapping.html
const ATTRIBUTE_ALIGNMENT: u32 = 4;

/// A draw's vertex layout and streams after widening: the layout, and each stream's buffer, offset and length.
pub(crate) struct Widened<M: Metal> {
    pub(crate) layout: Shared<VertexLayoutBlock>,
    pub(crate) streams: Vec<Option<(M::Buffer, u64, u64)>>,
}

/// Returns the format an attribute is widened to, if naga lacks its own.
fn widened(format: VertexFormat) -> Option<VertexFormat> {
    use VertexFormat as V;
    Some(match format {
        V::Uint8x3 => V::Uint8x4,
        V::Sint8x3 => V::Sint8x4,
        V::Unorm8x3 => V::Unorm8x4,
        V::Snorm8x3 => V::Snorm8x4,
        V::Uint16x3 | V::Uint10_10_10_2 => V::Uint16x4,
        V::Sint16x3 => V::Sint16x4,
        V::Unorm16x3 => V::Unorm16x4,
        V::Snorm16x3 => V::Snorm16x4,
        V::Float16x3 => V::Float16x4,
        V::Float11_11_10 => V::Float32x4,
        V::Bgrx8Unorm => V::Unorm8x4,
        _ => return None,
    })
}

/// Returns a float of `mantissa` bits over a 5-bit exponent, with no sign, as an f32, exactly.
fn small_float(bits: u32, mantissa: u32) -> f32 {
    let exponent = (bits >> mantissa) & 0x1f;
    let fraction = bits & ((1 << mantissa) - 1);
    let scale = (1u32 << mantissa) as f32;
    match exponent {
        0 => fraction as f32 / scale * 2f32.powi(-14),
        0x1f if fraction == 0 => f32::INFINITY,
        0x1f => f32::NAN,
        e => (1.0 + fraction as f32 / scale) * 2f32.powi(e as i32 - 15),
    }
}

/// Appends one attribute's value, widened, to `out`.
fn widen(format: VertexFormat, src: &[u8], out: &mut Vec<u8>) {
    use VertexFormat as V;
    let word = |at: usize| u32::from_le_bytes([src[at], src[at + 1], src[at + 2], src[at + 3]]);
    match format {
        V::Uint8x3 | V::Sint8x3 => out.extend_from_slice(&[src[0], src[1], src[2], 1]),
        V::Unorm8x3 => out.extend_from_slice(&[src[0], src[1], src[2], u8::MAX]),
        V::Snorm8x3 => out.extend_from_slice(&[src[0], src[1], src[2], i8::MAX as u8]),
        V::Uint16x3 | V::Sint16x3 => {
            out.extend_from_slice(&src[..6]);
            out.extend_from_slice(&1u16.to_le_bytes());
        }
        V::Unorm16x3 => {
            out.extend_from_slice(&src[..6]);
            out.extend_from_slice(&u16::MAX.to_le_bytes());
        }
        V::Snorm16x3 => {
            out.extend_from_slice(&src[..6]);
            out.extend_from_slice(&(i16::MAX as u16).to_le_bytes());
        }
        V::Float16x3 => {
            out.extend_from_slice(&src[..6]);
            out.extend_from_slice(&half_one().to_le_bytes());
        }
        V::Uint10_10_10_2 => {
            let w = word(0);
            for (shift, bits) in [(0, 10), (10, 10), (20, 10), (30, 2)] {
                out.extend_from_slice(&(((w >> shift) & ((1 << bits) - 1)) as u16).to_le_bytes());
            }
        }
        V::Float11_11_10 => {
            let w = word(0);
            for value in [
                small_float(w & 0x7ff, 6),
                small_float((w >> 11) & 0x7ff, 6),
                small_float(w >> 22, 5),
                1.0,
            ] {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        V::Bgrx8Unorm => out.extend_from_slice(&[src[2], src[1], src[0], u8::MAX]),
        _ => out.extend_from_slice(&src[..convert::vertex_bytes(format) as usize]),
    }
}

/// Returns the bits of a 16-bit float one.
fn half_one() -> u16 {
    // Exponent 15, the bias, and no fraction.
    15 << 10
}

impl<M: Metal> MetalGpu<M> {
    /// Returns the draw's layout and streams with each stream of a format naga lacks widened, or none if it has none.
    ///
    /// It ends the open pass, waits for the work before, and opens the pass again.
    pub(crate) fn widen_streams(
        &mut self,
        call: &DrawCall<'_, Self>,
    ) -> Result<Option<Widened<M>>, GpuError> {
        let desc = &call.vertex_layout.desc;
        let lacking = |stream: usize| {
            desc.attributes
                .iter()
                .flatten()
                .any(|a| a.stream as usize == stream && convert::vertex_format(a.format).is_none())
        };
        if !(0..desc.streams.len()).any(lacking) {
            return Ok(None);
        }
        self.record_slow_path(SlowPath::VertexFormatConverted);
        let mut layout = VertexLayoutDesc {
            streams: desc.streams,
            attributes: desc.attributes,
        };
        let mut streams = Vec::new();
        for (stream, s) in desc.streams.iter().enumerate() {
            let binding = call.vertex_buffers.get(stream).and_then(Option::as_ref);
            let (Some(s), Some(binding)) = (s, binding) else {
                streams.push(None);
                continue;
            };
            let raw = binding.buffer.raw.borrow().clone();
            let length = binding.buffer.size.saturating_sub(binding.offset);
            if !lacking(stream) {
                streams.push(Some((raw, binding.offset, length)));
                continue;
            }
            // The stream's attributes in their place order, each at the next aligned offset of the widened element.
            let mut places: Vec<(usize, VertexAttribute)> = desc
                .attributes
                .iter()
                .enumerate()
                .filter_map(|(i, a)| a.filter(|a| a.stream as usize == stream).map(|a| (i, a)))
                .collect();
            places.sort_by_key(|(_, a)| a.offset);
            let mut stride = 0;
            for (i, a) in &places {
                let format = widened(a.format).unwrap_or(a.format);
                layout.attributes[*i] = Some(VertexAttribute {
                    offset: stride,
                    format,
                    ..*a
                });
                stride += convert::vertex_bytes(format).next_multiple_of(ATTRIBUTE_ALIGNMENT);
            }
            let source_stride = u64::from(s.stride);
            let elements = length.checked_div(source_stride).unwrap_or(1);
            let resumed = self.end_open_pass();
            let bytes = self.read_buffer_now(&raw, binding.offset, length);
            self.pass = resumed;
            let mut out = Vec::with_capacity(elements as usize * stride as usize);
            for element in 0..elements as usize {
                let at = element * s.stride as usize;
                for (_, a) in &places {
                    let start = at + a.offset as usize;
                    let end = start + convert::vertex_bytes(a.format) as usize;
                    let Some(src) = bytes.get(start..end) else {
                        out.resize(
                            out.len()
                                + convert::vertex_bytes(widened(a.format).unwrap_or(a.format))
                                    as usize,
                            0,
                        );
                        continue;
                    };
                    widen(a.format, src, &mut out);
                    out.resize(out.len().next_multiple_of(ATTRIBUTE_ALIGNMENT as usize), 0);
                }
            }
            layout.streams[stream] = Some(VertexStream {
                stride: if s.stride == 0 { 0 } else { stride },
                ..*s
            });
            let buffer = self.staging(&out).ok_or(GpuError::OutOfMemory)?;
            streams.push(Some((buffer, 0, out.len() as u64)));
        }
        let id = self.widened_layout(call.vertex_layout.id, layout);
        Ok(Some(Widened {
            layout: id,
            streams,
        }))
    }

    /// Returns the block of a widened layout, one per layout it widens.
    fn widened_layout(&mut self, of: u64, desc: VertexLayoutDesc) -> Shared<VertexLayoutBlock> {
        if let Some(block) = self.widened_layouts.get(&of) {
            return block.clone();
        }
        let block = Shared(Rc::new(VertexLayoutBlock {
            id: self.id(),
            desc,
        }));
        self.widened_layouts.insert(of, block.clone());
        block
    }
}
