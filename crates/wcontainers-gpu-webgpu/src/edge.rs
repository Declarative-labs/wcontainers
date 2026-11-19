//! Byte-granular buffer copies in command order (plan section 4.2): WebGPU copies, writes and clears
//! buffers in whole 4-byte words, so the words a range only partly covers are merged by a small
//! compute program (slow path `UnalignedBufferEdge`).

use wcontainers_gpu::SlowPath;

use crate::{GROUP, Stream, WebGpu, with_group};

/// Copies bytes of `src` over bytes of `dst` in the words it touches, keeping the rest of each word.
const EDGE_WGSL: &str = r#"
struct P { src_byte: u32, dst_byte: u32, size: u32, first_word: u32, words: u32, p0: u32, p1: u32, p2: u32 }
@group(0) @binding(0) var<storage, read_write> dst: array<u32>;
@group(0) @binding(1) var<storage, read> src: array<u32>;
@group(0) @binding(2) var<uniform> p: P;

fn src_byte(a: u32) -> u32 {
    return (src[a >> 2u] >> ((a & 3u) * 8u)) & 0xffu;
}

@compute @workgroup_size(GROUP)
fn main(@builtin(global_invocation_id) id: vec3u) {
    if id.x >= p.words {
        return;
    }
    let w = p.first_word + id.x;
    var v = dst[w];
    for (var j = 0u; j < 4u; j++) {
        let a = w * 4u + j;
        if a >= p.dst_byte && a - p.dst_byte < p.size {
            let shift = j * 8u;
            v = (v & ~(0xffu << shift)) | (src_byte(a - p.dst_byte + p.src_byte) << shift);
        }
    }
    dst[w] = v;
}
"#;

/// Where and how much a native buffer copy may move.
const ALIGN: u64 = wgpu::COPY_BUFFER_ALIGNMENT;
/// The bytes of a word the program reads and writes.
const WORD: u64 = wcontainers_gpu::STORAGE_WORD;

/// The program, made at its first use.
pub struct EdgeProgram {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
}

impl WebGpu {
    /// Copies `size` bytes from `src` at `src_at` to `dst` at `dst_at`, two different GPU buffers,
    /// in `stream` outside a pass: natively where both ends and the size are whole words; where the
    /// two offsets agree in their word, the middle natively and the edge words by the program;
    /// otherwise every word by the program.
    pub(crate) fn copy_bytes(
        &mut self,
        stream: Stream,
        src: &wgpu::Buffer,
        src_at: u64,
        dst: &wgpu::Buffer,
        dst_at: u64,
        size: u64,
    ) {
        if size == 0 {
            return;
        }
        if [src_at, dst_at, size]
            .iter()
            .all(|v| v.is_multiple_of(ALIGN))
        {
            self.stream(stream)
                .copy_buffer_to_buffer(src, src_at, dst, dst_at, size);
            return;
        }
        self.slow.hit(SlowPath::UnalignedBufferEdge);
        if src_at % ALIGN != dst_at % ALIGN {
            self.merge(stream, src, src_at, dst, dst_at, size);
            return;
        }
        let head = ((ALIGN - dst_at % ALIGN) % ALIGN).min(size);
        let middle = (size - head) & !(ALIGN - 1);
        let tail = size - head - middle;
        self.merge(stream, src, src_at, dst, dst_at, head);
        if middle > 0 {
            self.stream(stream).copy_buffer_to_buffer(
                src,
                src_at + head,
                dst,
                dst_at + head,
                middle,
            );
        }
        self.merge(
            stream,
            src,
            src_at + head + middle,
            dst,
            dst_at + head + middle,
            tail,
        );
    }

    /// Runs the program over the words of `dst` the range touches, in pieces whose workgroups and
    /// bound windows stay inside the device's limits.
    fn merge(
        &mut self,
        stream: Stream,
        src: &wgpu::Buffer,
        src_at: u64,
        dst: &wgpu::Buffer,
        dst_at: u64,
        size: u64,
    ) {
        let limits = self.device.limits();
        let align = u64::from(limits.min_storage_buffer_offset_alignment);
        // A piece's window adds up to `align` bytes before it and a word after it, and one more
        // word may need one more workgroup.
        let piece = ((u64::from(limits.max_compute_workgroups_per_dimension) - 1) * GROUP * WORD)
            .min(limits.max_storage_buffer_binding_size - align - WORD)
            & !(WORD - 1);
        let mut done = 0;
        while done < size {
            let n = (size - done).min(piece);
            self.merge_piece(stream, src, src_at + done, dst, dst_at + done, n);
            done += n;
        }
    }

    fn merge_piece(
        &mut self,
        stream: Stream,
        src: &wgpu::Buffer,
        src_at: u64,
        dst: &wgpu::Buffer,
        dst_at: u64,
        size: u64,
    ) {
        if self.edge.is_none() {
            self.edge = Some(self.edge_program());
        }
        let align = u64::from(self.caps.limits.min_storage_offset_alignment);
        // Each buffer is bound from the aligned offset below the range to the word after it.
        let window = |at: u64| {
            let start = at & !(WORD - 1) & !(align - 1);
            (start, (at + size).next_multiple_of(WORD) - start)
        };
        let (dst_start, dst_len) = window(dst_at);
        let (src_start, src_len) = window(src_at);
        let first_word = (dst_at & !(WORD - 1)) - dst_start;
        let params: Vec<u8> = [
            src_at - src_start,
            dst_at - dst_start,
            size,
            first_word / WORD,
            (dst_len - first_word) / WORD,
            0,
            0,
            0,
        ]
        .iter()
        .flat_map(|v| (*v as u32).to_le_bytes())
        .collect();
        let words = (dst_len - first_word) / WORD;
        let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: params.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: true,
        });
        uniform
            .slice(..)
            .get_mapped_range_mut()
            .expect("a buffer mapped at creation")
            .copy_from_slice(&params);
        uniform.unmap();
        let edge = self.edge.as_ref().expect("made above");
        let binding = |buffer, offset, len| {
            wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset,
                size: wgpu::BufferSize::new(len),
            })
        };
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &edge.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: binding(dst, dst_start, dst_len),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: binding(src, src_start, src_len),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        let pipeline = edge.pipeline.clone();
        let mut pass = self
            .stream(stream)
            .begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(words.div_ceil(GROUP) as u32, 1, 1);
    }

    fn edge_program(&self) -> EdgeProgram {
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(with_group(EDGE_WGSL).into()),
            });
        let storage = |binding, read_only| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    storage(0, false),
                    storage(1, true),
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
        EdgeProgram { pipeline, layout }
    }
}
