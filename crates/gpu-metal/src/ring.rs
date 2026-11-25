//! The rings of per-draw data, written straight into memory the CPU shares with the GPU.
//!
//! A ring is a list of chunks. The current chunk takes writes until anything is encoded, since an encoded pass may read
//! it; then the next chunk that no work uses takes over, so no write waits or goes through a copy on the GPU.

use std::rc::Rc;

use wcontainers_gpu::{GpuError, RingKind};
use wcontainers_metal::Metal;

use crate::MetalGpu;
use crate::bytes::rounded;
use crate::objects::{BufferObj, Shared};

/// The bytes of one chunk of a ring, unless one write needs more.
///
/// https://developer.apple.com/documentation/metal/mtldevice/makebuffer(length:options:)
const CHUNK_BYTES: u64 = 1 << 20;

/// The least alignment of the parts written into a ring, the granule of Metal's buffer copies on macOS.
///
/// https://developer.apple.com/documentation/metal/mtlblitcommandencoder/copy(from:sourceoffset:to:destinationoffset:size:)
const PART_ALIGNMENT: u64 = 4;

/// One chunk of a ring and how far it is written.
struct Chunk<M: Metal> {
    buffer: Shared<BufferObj<M>>,
    head: u64,
}

/// The chunks of one ring, and the current one with the count of encodes when it became current.
pub(crate) struct Ring<M: Metal> {
    chunks: Vec<Chunk<M>>,
    current: Option<(usize, u64)>,
}

impl<M: Metal> Default for Ring<M> {
    fn default() -> Self {
        Ring {
            chunks: Vec::new(),
            current: None,
        }
    }
}

impl<M: Metal> MetalGpu<M> {
    /// Writes the parts into a chunk of the ring of `kind` that no encoded work uses, and returns that chunk.
    pub(crate) fn write_ring(
        &mut self,
        kind: RingKind,
        parts: &[&[u8]],
        align: u32,
        offsets: &mut [u64],
    ) -> Result<Shared<BufferObj<M>>, GpuError> {
        let align = u64::from(align).max(PART_ALIGNMENT);
        let total: u64 = parts
            .iter()
            .map(|p| (p.len() as u64).next_multiple_of(align))
            .sum();
        let slot = kind as usize;
        let encodes = self.encodes;
        let fits =
            |chunk: &Chunk<M>| chunk.head.next_multiple_of(align) + total <= chunk.buffer.size;
        let current = match self.rings[slot].current {
            Some((index, opened)) if opened == encodes && fits(&self.rings[slot].chunks[index]) => {
                Some(index)
            }
            _ => None,
        };
        let index = match current {
            Some(index) => index,
            None => {
                let mut free = None;
                for index in 0..self.rings[slot].chunks.len() {
                    let buffer = self.rings[slot].chunks[index].buffer.clone();
                    if buffer.size >= total && !self.busy(&buffer) {
                        free = Some(index);
                        break;
                    }
                }
                let index = match free {
                    Some(index) => index,
                    None => {
                        let size = total.max(CHUNK_BYTES).next_power_of_two();
                        let raw = self
                            .metal
                            .make_buffer(rounded(size))
                            .map_err(|_| GpuError::OutOfMemory)?;
                        self.rings[slot].chunks.push(Chunk {
                            buffer: Shared(Rc::new(BufferObj {
                                size,
                                raw: raw.into(),
                                last_use: 0.into(),
                            })),
                            head: u64::MIN,
                        });
                        self.rings[slot].chunks.len() - 1
                    }
                };
                self.rings[slot].chunks[index].head = 0;
                self.rings[slot].current = Some((index, encodes));
                index
            }
        };
        let chunk = &self.rings[slot].chunks[index];
        let (buffer, mut head) = (chunk.buffer.clone(), chunk.head);
        let raw = buffer.raw.borrow().clone();
        for (part, offset) in parts.iter().zip(offsets.iter_mut()) {
            head = head.next_multiple_of(align);
            *offset = head;
            self.metal
                .write_contents(&raw, head, part)
                .map_err(|_| GpuError::Invalid("a ring chunk Metal's work still uses"))?;
            head += part.len() as u64;
        }
        if total == 0 {
            offsets
                .iter_mut()
                .for_each(|o| *o = head.next_multiple_of(align));
        }
        self.rings[slot].chunks[index].head = head;
        Ok(buffer)
    }
}
