//! Buffer copies and fills at any byte, which Metal's blits on macOS take only in multiples of four bytes.

use wcontainers_gpu::{Gpu, ShaderModule, ShaderStage, SlowPath};
use wcontainers_metal::{BlitCommand, ComputeCommand, Metal, Size};

use crate::MetalGpu;

/// The bytes Metal's buffer copies and fills on macOS take their offsets and sizes in multiples of.
///
/// https://developer.apple.com/documentation/metal/mtlblitcommandencoder/copy(from:sourceoffset:to:destinationoffset:size:)
pub(crate) const COPY_GRANULE: u64 = 4;

/// The threads of one threadgroup of the byte programs, as their `workgroup_size` says.
///
/// https://www.w3.org/TR/WGSL/#attribute-workgroup_size
const BYTE_THREADS: u64 = 64;

/// The programs that copy and fill bytes a word at a time, each thread writing one word of the destination.
const BYTE_PROGRAMS: &str = "
struct Span { source: u32, destination: u32, size: u32, value: u32 }
@group(0) @binding(0) var<uniform> span: Span;
@group(0) @binding(1) var<storage, read> source: array<u32>;
@group(0) @binding(2) var<storage, read_write> destination: array<u32>;

fn source_byte(at: u32) -> u32 {
    return (source[at / 4u] >> ((at % 4u) * 8u)) & 0xffu;
}

fn word_of(word: u32, fill: bool) -> u32 {
    let end = span.destination + span.size;
    var value = destination[word];
    for (var i = 0u; i < 4u; i++) {
        let at = word * 4u + i;
        if at >= span.destination && at < end {
            var byte = span.value;
            if !fill {
                byte = source_byte(span.source + at - span.destination);
            }
            value = (value & ~(0xffu << (i * 8u))) | (byte << (i * 8u));
        }
    }
    return value;
}

@compute @workgroup_size(64) fn copy_bytes(@builtin(global_invocation_id) id: vec3<u32>) {
    let word = span.destination / 4u + id.x;
    if word * 4u < span.destination + span.size {
        destination[word] = word_of(word, false);
    }
}

@compute @workgroup_size(64) fn fill_bytes(@builtin(global_invocation_id) id: vec3<u32>) {
    let word = span.destination / 4u + id.x;
    if word * 4u < span.destination + span.size {
        destination[word] = word_of(word, true);
    }
}
";

/// A range of bytes in a buffer whose length is a multiple of four.
pub(crate) struct ByteRange<M: Metal> {
    pub(crate) buffer: M::Buffer,
    /// The bytes the buffer holds, a multiple of four.
    pub(crate) length: u64,
    pub(crate) offset: u64,
}

/// One of the byte programs, made at its first use, and the buffer slots of its bindings.
pub(crate) struct BytePipeline<M: Metal> {
    state: M::ComputePipelineState,
    span: u32,
    source: Option<u32>,
    destination: u32,
}

/// The two byte programs.
pub(crate) struct BytePrograms<M: Metal> {
    copy: Option<BytePipeline<M>>,
    fill: Option<BytePipeline<M>>,
}

impl<M: Metal> Default for BytePrograms<M> {
    fn default() -> Self {
        BytePrograms {
            copy: None,
            fill: None,
        }
    }
}

/// Returns `length` rounded up to a multiple of four, the length of every buffer this implementation makes.
pub(crate) fn rounded(length: u64) -> u64 {
    length.next_multiple_of(COPY_GRANULE)
}

impl<M: Metal> MetalGpu<M> {
    /// Copies `size` bytes, by a blit where every number is a multiple of four and by a program otherwise.
    pub(crate) fn copy_bytes(
        &mut self,
        source: ByteRange<M>,
        destination: ByteRange<M>,
        size: u64,
    ) {
        if size == 0 {
            return;
        }
        if [source.offset, destination.offset, size]
            .iter()
            .all(|n| n.is_multiple_of(COPY_GRANULE))
        {
            self.queue_blit(BlitCommand::CopyFromBufferToBuffer {
                source_buffer: source.buffer,
                source_offset: source.offset,
                destination_buffer: destination.buffer,
                destination_offset: destination.offset,
                size,
            });
            return;
        }
        self.run_bytes(false, Some(&source), &destination, size, 0);
    }

    /// Sets `size` bytes to `value`, by a blit where both numbers are multiples of four and by a program otherwise.
    pub(crate) fn fill_bytes(&mut self, destination: ByteRange<M>, size: u64, value: u8) {
        if size == 0 {
            return;
        }
        if destination.offset.is_multiple_of(COPY_GRANULE) && size.is_multiple_of(COPY_GRANULE) {
            self.queue_blit(BlitCommand::FillBuffer {
                buffer: destination.buffer,
                offset: destination.offset,
                length: size,
                value,
            });
            return;
        }
        self.run_bytes(true, None, &destination, size, value);
    }

    /// Returns a byte program, made at its first use.
    fn byte_pipeline(&mut self, fill: bool) -> Result<&BytePipeline<M>, wcontainers_gpu::GpuError> {
        let made = if fill {
            self.byte_programs.fill.is_some()
        } else {
            self.byte_programs.copy.is_some()
        };
        if !made {
            let entry = if fill { "fill_bytes" } else { "copy_bytes" };
            let shader = self.create_shader(&ShaderModule::from_wgsl(
                BYTE_PROGRAMS,
                entry,
                ShaderStage::Compute,
            )?)?;
            let function = self
                .make_function(&shader, &[], false)
                .map_err(wcontainers_gpu::GpuError::Invalid)?;
            let state = self
                .metal
                .make_compute_pipeline_state(&function)
                .map_err(|_| wcontainers_gpu::GpuError::Invalid("a byte program Metal refuses"))?;
            let slot = |binding: u32| shader.slots.get(&(0, binding)).map(|&(_, index)| index);
            let missing = wcontainers_gpu::GpuError::Invalid("a byte program without its bindings");
            let pipeline = BytePipeline {
                state,
                span: slot(0).ok_or(missing)?,
                source: slot(1),
                destination: slot(2).ok_or(missing)?,
            };
            if fill {
                self.byte_programs.fill = Some(pipeline);
            } else {
                self.byte_programs.copy = Some(pipeline);
            }
        }
        let made = if fill {
            &self.byte_programs.fill
        } else {
            &self.byte_programs.copy
        };
        made.as_ref().ok_or(wcontainers_gpu::GpuError::Invalid(
            "a byte program not made",
        ))
    }

    /// Encodes a byte program over a range, ending the open pass for it and opening it again after.
    fn run_bytes(
        &mut self,
        fill: bool,
        source: Option<&ByteRange<M>>,
        destination: &ByteRange<M>,
        size: u64,
        value: u8,
    ) {
        self.record_slow_path(SlowPath::UnalignedBufferEdge);
        // The programs count bytes in 32-bit words.
        let inside = |range: &ByteRange<M>| {
            range
                .offset
                .checked_add(size)
                .is_some_and(|end| end <= range.length && end <= u64::from(u32::MAX))
        };
        if !inside(destination) || source.is_some_and(|s| !inside(s)) {
            self.fail(format_args!("a byte copy or fill outside its buffers"));
            return;
        }
        let (state, span_slot, source_slot, destination_slot) = match self.byte_pipeline(fill) {
            Ok(p) => (p.state.clone(), p.span, p.source, p.destination),
            Err(error) => {
                self.fail(format_args!("no byte program: {error:?}"));
                return;
            }
        };
        let span: Vec<u8> = [
            source.map_or(0, |s| s.offset),
            destination.offset,
            size,
            u64::from(value),
        ]
        .iter()
        .flat_map(|n| (*n as u32).to_le_bytes())
        .collect();
        let mut commands = vec![
            ComputeCommand::SetComputePipelineState(state),
            ComputeCommand::SetBytes {
                bytes: span,
                index: span_slot,
            },
            ComputeCommand::SetBuffer {
                buffer: destination.buffer.clone(),
                offset: 0,
                length: destination.length,
                index: destination_slot,
            },
        ];
        if let (Some(source), Some(index)) = (source, source_slot) {
            commands.push(ComputeCommand::SetBuffer {
                buffer: source.buffer.clone(),
                offset: 0,
                length: source.length,
                index,
            });
        }
        let first_word = destination.offset / COPY_GRANULE;
        let words = (destination.offset + size).div_ceil(COPY_GRANULE) - first_word;
        commands.push(ComputeCommand::DispatchThreadgroups(Size {
            width: words.div_ceil(BYTE_THREADS) as u32,
            height: 1,
            depth: 1,
        }));
        let resumed = if self.pass.is_some() {
            self.record_slow_path(SlowPath::WriteSplitPass);
            self.end_open_pass()
        } else {
            None
        };
        self.flush_blits();
        self.encodes += 1;
        if let Some(command_buffer) = self.command_buffer()
            && let Err(error) = self
                .metal
                .encode_compute_commands(&command_buffer, &commands)
        {
            self.fail(format_args!("a byte program refused: {error:?}"));
        }
        if resumed.is_some() {
            self.pass = resumed;
        }
    }
}
