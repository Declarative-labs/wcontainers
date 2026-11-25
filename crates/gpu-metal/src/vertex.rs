//! Streams of formats naga's vertex reads lack, widened on the host (slow path `VertexFormatConverted`).
//!
//! Such a stream is read back, each attribute widened into a format naga reads, and drawn from the copy.

use std::rc::Rc;

use wcontainers_gpu::{
    DrawCall, DrawRange, Gpu, GpuError, SlowPath, StepMode, VertexAttribute, VertexFormat,
    VertexLayoutDesc, VertexStream,
};
use wcontainers_metal::Metal;

use crate::objects::{Shared, VertexLayoutBlock};
use crate::{MetalGpu, convert};

/// The bytes widened attributes start at multiples of, the alignment of naga's vertex reads.
///
/// https://docs.rs/naga/30.0.2/naga/back/msl/struct.AttributeMapping.html
const ATTRIBUTE_ALIGNMENT: u32 = 4;

/// The most bytes a widened copy of one stream holds; a draw that needs more is refused.
///
/// The copy runs from the stream's offset to the last element the draw reads, so neither a buffer nor a count the
/// caller chose sizes it. 64 MiB holds four million elements of 16 bytes, the widest widened attribute.
///
/// https://docs.rs/naga/30.0.2/naga/back/msl/enum.VertexFormat.html
const MAX_WIDENED_BYTES: u64 = 64 << 20;

/// Returns the range of a stream's elements a draw reads, before the stream's buffer cuts it.
///
/// An indexed draw may read any element its indices name, so it reads them all.
fn find_elements_read<B: Gpu>(range: &DrawRange<'_, B>, step: StepMode, stride: u32) -> (u64, u64) {
    let all = (u64::MIN, u64::MAX);
    if stride == 0 {
        return (u64::MIN, 1);
    }
    let span = |first: u32, count: u32| {
        (
            u64::from(first),
            u64::from(first).saturating_add(u64::from(count)),
        )
    };
    match (range, step) {
        (DrawRange::Vertices { first, count, .. }, StepMode::Vertex) => span(*first, *count),
        (
            DrawRange::Vertices {
                first_instance,
                instances,
                ..
            }
            | DrawRange::Indexed {
                first_instance,
                instances,
                ..
            },
            StepMode::Instance(rate),
        ) => {
            // The instance of index `i` reads element `first_instance + (i - first_instance) / rate`.
            let elements = instances.checked_div(rate).map_or(1, |whole| {
                whole.saturating_add(u32::from(!instances.is_multiple_of(rate)))
            });
            span(*first_instance, elements.min(*instances))
        }
        _ => all,
    }
}

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
    let fraction = bits & !(u32::MAX << mantissa);
    let scale = (1u32 << mantissa) as f32;
    match exponent {
        0 => fraction as f32 / scale * 2f32.powi(-14),
        0x1f if fraction == 0 => f32::INFINITY,
        0x1f => f32::NAN,
        // The exponent is below 31, so the unbiased one lies within -14 to 15.
        e => {
            (1.0 + fraction as f32 / scale)
                * 2f32.powi(i32::try_from(e).unwrap_or(i32::MIN).saturating_sub(15))
        }
    }
}

/// Appends one attribute's value, widened, to `out`.
fn widen(format: VertexFormat, src: &[u8], out: &mut Vec<u8>) {
    use VertexFormat as V;
    let word = || {
        src.first_chunk::<4>()
            .map_or(u32::MIN, |w| u32::from_le_bytes(*w))
    };
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
            let w = word();
            for (shift, bits) in [(0, 10), (10, 10), (20, 10), (30, 2)] {
                out.extend_from_slice(&(((w >> shift) & !(u32::MAX << bits)) as u16).to_le_bytes());
            }
        }
        V::Float11_11_10 => {
            let w = word();
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

/// Returns the ranges of a stream a widened copy reads, and the stride its elements lie at once read.
///
/// The elements are those from `first` to `end` at `stride`, `element` bytes of each, in a stream of `length` bytes: one
/// range when they lie close, one an element otherwise, so that no more than twice their bytes are read. None when a
/// place overflows.
pub(crate) fn find_element_reads(
    first: u64,
    end: u64,
    stride: u64,
    element: u64,
    length: u64,
) -> Option<(u64, Vec<(u64, u64)>)> {
    let count = end.saturating_sub(first);
    let start = first.checked_mul(stride)?;
    if count == 0 || start >= length {
        return Some((stride, Vec::new()));
    }
    if stride <= element.saturating_mul(2) {
        let span = count
            .saturating_sub(1)
            .checked_mul(stride)?
            .checked_add(element)?;
        return Some((
            stride,
            vec![(start, span.min(length.saturating_sub(start)))],
        ));
    }
    let mut reads = Vec::new();
    for at in 0..count {
        let offset = at.checked_mul(stride)?.checked_add(start)?;
        if offset >= length {
            break;
        }
        reads.push((offset, element.min(length.saturating_sub(offset))));
    }
    Some((element, reads))
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
            // Every place and length below is checked: one that overflows refuses the draw.
            let overflow = GpuError::Invalid("a vertex stream whose elements overflow");
            let mut stride: u32 = 0;
            for (i, a) in &places {
                let format = widened(a.format).unwrap_or(a.format);
                layout.attributes[*i] = Some(VertexAttribute {
                    offset: stride,
                    format,
                    ..*a
                });
                stride = convert::vertex_bytes(format)
                    .checked_next_multiple_of(ATTRIBUTE_ALIGNMENT)
                    .and_then(|bytes| stride.checked_add(bytes))
                    .ok_or(overflow)?;
            }
            let source_stride = u64::from(s.stride);
            let in_buffer = length.checked_div(source_stride).unwrap_or(1);
            let (first, end) = find_elements_read(&call.range, s.step, s.stride);
            // The elements the draw can read that the buffer holds: past them the copy's bounds checks read zeros, so a
            // draw that reads none of them needs an empty copy.
            let end = end.min(in_buffer);
            let (first, end) = if first < end {
                (first, end)
            } else {
                (u64::MIN, u64::MIN)
            };
            let copy_bytes = end.checked_mul(u64::from(stride)).ok_or(overflow)?;
            if copy_bytes > MAX_WIDENED_BYTES {
                return Err(GpuError::Invalid(
                    "a widened vertex stream of more bytes than MAX_WIDENED_BYTES",
                ));
            }
            // From the first element read to the end of the last element's last attribute, within the buffer.
            let last_attribute = places
                .iter()
                .map(|(_, a)| {
                    u64::from(a.offset).saturating_add(u64::from(convert::vertex_bytes(a.format)))
                })
                .max()
                .unwrap_or(u64::MIN);
            // Only the bytes of the elements the draw reads are read back, each element's alone when they lie apart.
            let (packed, reads) =
                find_element_reads(first, end, source_stride, last_attribute, length)
                    .ok_or(overflow)?;
            let resumed = self.end_open_pass();
            let mut bytes = Vec::new();
            for (at, size) in reads {
                let offset = binding.offset.checked_add(at).ok_or(overflow)?;
                let mut element = self.read_buffer_now(&raw, offset, size);
                if packed != source_stride {
                    element.resize(usize::try_from(packed).map_err(|_| overflow)?, 0);
                }
                bytes.extend_from_slice(&element);
            }
            self.pass = resumed;
            let size = |n: u64| usize::try_from(n).map_err(|_| overflow);
            // The elements before the first the draw reads stay zero.
            let mut out = vec![0; size(first.checked_mul(u64::from(stride)).ok_or(overflow)?)?];
            out.reserve(size(copy_bytes)?.saturating_sub(out.len()));
            for element in 0..size(end.saturating_sub(first))? {
                let at = element.checked_mul(size(packed)?).ok_or(overflow)?;
                for (_, a) in &places {
                    let start = at.checked_add(a.offset as usize).ok_or(overflow)?;
                    let end = start
                        .checked_add(convert::vertex_bytes(a.format) as usize)
                        .ok_or(overflow)?;
                    let Some(src) = bytes.get(start..end) else {
                        let widened_bytes =
                            convert::vertex_bytes(widened(a.format).unwrap_or(a.format)) as usize;
                        out.resize(out.len().checked_add(widened_bytes).ok_or(overflow)?, 0);
                        continue;
                    };
                    widen(a.format, src, &mut out);
                    let aligned = out
                        .len()
                        .checked_next_multiple_of(ATTRIBUTE_ALIGNMENT as usize)
                        .ok_or(overflow)?;
                    out.resize(aligned, 0);
                }
            }
            layout.streams[stream] = Some(VertexStream {
                stride: if s.stride == 0 { 0 } else { stride },
                ..*s
            });
            let buffer = self.staging(&out).ok_or(GpuError::OutOfMemory)?.buffer;
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

#[cfg(test)]
mod tests {
    use super::find_element_reads;

    #[test]
    fn a_widened_copy_reads_each_element_it_needs_and_no_more() {
        // Two elements a gigabyte apart: twelve bytes of each, not the gigabyte between.
        assert_eq!(
            find_element_reads(0, 2, 1 << 30, 12, 2 << 30),
            Some((12, vec![(0, 12), (1 << 30, 12)]))
        );
        // Elements close together: one range, from the first element read to the end of the last.
        assert_eq!(
            find_element_reads(1, 4, 16, 12, 1000),
            Some((16, vec![(16, 44)]))
        );
        // The last element is cut at the stream's end, and one past it is not read.
        assert_eq!(
            find_element_reads(0, 3, 100, 12, 105),
            Some((12, vec![(0, 12), (100, 5)]))
        );
        // No element, or a first one past the end, reads nothing; a place past `u64` refuses.
        assert_eq!(
            find_element_reads(5, 5, 100, 12, 1000),
            Some((100, Vec::new()))
        );
        assert_eq!(
            find_element_reads(20, 21, 100, 12, 1000),
            Some((100, Vec::new()))
        );
        assert_eq!(
            find_element_reads(1 << 40, 1 << 41, 1 << 30, 12, u64::MAX),
            None
        );
    }
}
