//! Vertex formats WebGPU lacks (plan 5.7, slow path `VertexFormatConverted`): a stream with one is
//! read through a shadow stream, every attribute of the stream laid out again in formats WebGPU
//! has, made by a compute program before the draw and kept until the source buffer is written.

use std::collections::HashMap;
use std::rc::Rc;

use wcontainers_gpu::{DrawCall, SlowPath, VertexFormat, VertexLayoutDesc};

use crate::{BufferObj, Stream, WebGpu, convert};

/// Makes the shadow elements: each operation reads one attribute of a source element and writes
/// it, widened where its format is lacking, at its place in the shadow element.
const CONVERT_WGSL: &str = r#"
struct P {
    src_offset: u32, src_stride: u32, dst_stride: u32, count: u32,
    row: u32, ops: u32, p0: u32, p1: u32,
    op: array<vec4u, 32>,
}
@group(0) @binding(0) var<storage, read> src: array<u32>;
@group(0) @binding(1) var<storage, read_write> dst: array<u32>;
@group(0) @binding(2) var<uniform> p: P;

fn byte(a: u32) -> u32 {
    return (src[a >> 2u] >> ((a & 3u) * 8u)) & 0xffu;
}

fn word(a: u32) -> u32 {
    return byte(a) | (byte(a + 1u) << 8u) | (byte(a + 2u) << 16u) | (byte(a + 3u) << 24u);
}

/// An unsigned float of `m` mantissa bits over a 5-bit exponent as a 16-bit float, exactly.
fn half(v: u32, m: u32) -> u32 {
    return (((v >> m) & 0x1fu) << 10u) | ((v & ((1u << m) - 1u)) << (10u - m));
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let e = id.x + id.y * p.row;
    if e >= p.count {
        return;
    }
    let s = p.src_offset + e * p.src_stride;
    let d = e * p.dst_stride / 4u;
    for (var i = 0u; i < p.ops; i++) {
        let o = p.op[i];
        let a = s + o.x;
        let w = d + o.y / 4u;
        switch o.z {
            // Bytes as they are, the last word padded with zeros.
            case 0u: {
                for (var k = 0u; k < o.w; k += 4u) {
                    var v = 0u;
                    for (var j = 0u; j < min(4u, o.w - k); j++) {
                        v |= byte(a + k + j) << (j * 8u);
                    }
                    dst[w + k / 4u] = v;
                }
            }
            // Three bytes and the fourth component's one.
            case 1u: {
                dst[w] = byte(a) | (byte(a + 1u) << 8u) | (byte(a + 2u) << 16u) | (o.w << 24u);
            }
            // Three 16-bit values and the fourth component's one.
            case 2u: {
                dst[w] = word(a);
                dst[w + 1u] = byte(a + 4u) | (byte(a + 5u) << 8u) | (o.w << 16u);
            }
            // 10, 10, 10 and 2 unsigned bits as four 16-bit values.
            case 3u: {
                let v = word(a);
                dst[w] = (v & 0x3ffu) | (((v >> 10u) & 0x3ffu) << 16u);
                dst[w + 1u] = ((v >> 20u) & 0x3ffu) | ((v >> 30u) << 16u);
            }
            // 11, 11 and 10-bit floats as 16-bit floats, and one.
            default: {
                let v = word(a);
                dst[w] = half(v, 6u) | (half(v >> 11u, 6u) << 16u);
                dst[w + 1u] = half(v >> 22u, 5u) | (0x3c00u << 16u);
            }
        }
    }
}
"#;

/// Threads per workgroup of the program.
const GROUP: u64 = 64;

/// Bytes of the program's parameters: eight words and 32 operations.
const PARAMS: usize = 32 + 32 * 16;

/// One converted stream: its shadow element and how it is made.
#[derive(Debug)]
pub struct StreamPlan {
    /// Bytes of a shadow element.
    size: u32,
    /// The source's stride; zero repeats one element.
    src_stride: u32,
    /// Source bytes an element reaches from its start.
    span: u32,
    /// Per attribute: source offset, shadow offset, operation, and its argument.
    ops: Vec<[u32; 4]>,
    /// The attributes as the pipeline reads them from the shadow.
    pub attributes: Vec<wgpu::VertexAttribute>,
}

impl StreamPlan {
    /// The shadow stream's stride.
    pub fn stride(&self) -> u64 {
        if self.src_stride == 0 {
            0
        } else {
            u64::from(self.size)
        }
    }
}

/// The streams of one vertex layout that are read through shadows, by stream.
pub type Plan = Rc<[Option<StreamPlan>]>;

/// The format a lacking one is read as, its operation and argument (the fourth component's bits).
fn widened(f: VertexFormat) -> Option<(wgpu::VertexFormat, u32, u32)> {
    use VertexFormat::*;
    use wgpu::VertexFormat as W;
    Some(match f {
        Uint8x3 => (W::Uint8x4, 1, 1),
        Sint8x3 => (W::Sint8x4, 1, 1),
        Unorm8x3 => (W::Unorm8x4, 1, 0xff),
        Snorm8x3 => (W::Snorm8x4, 1, 0x7f),
        Uint16x3 => (W::Uint16x4, 2, 1),
        Sint16x3 => (W::Sint16x4, 2, 1),
        Unorm16x3 => (W::Unorm16x4, 2, 0xffff),
        Snorm16x3 => (W::Snorm16x4, 2, 0x7fff),
        Float16x3 => (W::Float16x4, 2, 0x3c00),
        Uint10_10_10_2 => (W::Uint16x4, 3, 0),
        Float11_11_10 => (W::Float16x4, 4, 0),
        _ => return None,
    })
}

/// Bytes a lacking format takes in the source.
fn source_size(f: VertexFormat) -> u32 {
    use VertexFormat::*;
    match f {
        Uint8x3 | Sint8x3 | Unorm8x3 | Snorm8x3 => 3,
        Uint10_10_10_2 | Float11_11_10 => 4,
        _ => 6,
    }
}

/// The shadow streams of a layout, or None when WebGPU reads every stream as it is.
pub fn plan(desc: &VertexLayoutDesc) -> Option<Plan> {
    let lacking = |s: usize| {
        desc.attributes
            .iter()
            .flatten()
            .any(|a| usize::from(a.stream) == s && widened(a.format).is_some())
    };
    if !(0..desc.streams.len()).any(lacking) {
        return None;
    }
    let plans = desc
        .streams
        .iter()
        .enumerate()
        .map(|(s, stream)| {
            let stream = (*stream)?;
            if !lacking(s) {
                return None;
            }
            let (mut size, mut span) = (0, 0);
            let mut ops = Vec::new();
            let mut attributes = Vec::new();
            for a in desc
                .attributes
                .iter()
                .flatten()
                .filter(|a| usize::from(a.stream) == s)
            {
                let (format, op, arg, src) = match widened(a.format) {
                    Some((format, op, arg)) => (format, op, arg, source_size(a.format)),
                    None => {
                        let format = convert::vertex_format(a.format)?;
                        let bytes = format.size() as u32;
                        (format, 0, bytes, bytes)
                    }
                };
                ops.push([a.offset, size, op, arg]);
                attributes.push(wgpu::VertexAttribute {
                    format,
                    offset: u64::from(size),
                    shader_location: a.location,
                });
                size += (format.size() as u32).next_multiple_of(4);
                span = span.max(a.offset + src);
            }
            Some(StreamPlan {
                size: size.max(4),
                src_stride: stream.stride,
                span,
                ops,
                attributes,
            })
        })
        .collect();
    Some(plans)
}

/// A shadow kept for a source stream: the source's write count it was made at, and the shadow.
struct Shadow {
    written: u64,
    id: u64,
    buffer: wgpu::Buffer,
}

/// The program, the plans of vertex layouts that need shadows, and the shadows made.
#[derive(Default)]
pub struct Vertices {
    program: Option<(wgpu::ComputePipeline, wgpu::BindGroupLayout)>,
    pub plans: HashMap<u64, Plan>,
    /// By source buffer, offset, layout and stream.
    shadows: HashMap<(u64, u64, u64, usize), Shadow>,
}

impl Vertices {
    /// Drops the shadows of a destroyed buffer.
    pub fn forget(&mut self, buffer: u64) {
        if !self.shadows.is_empty() {
            self.shadows.retain(|k, _| k.0 != buffer);
        }
    }

    /// Drops the plan and shadows of a destroyed vertex layout.
    pub fn forget_layout(&mut self, layout: u64) {
        if self.plans.remove(&layout).is_some() {
            self.shadows.retain(|k, _| k.2 != layout);
        }
    }
}

/// The shadow bound in place of a stream: its slot, a number no other buffer has, and the buffer.
pub type Bound = (usize, u64, wgpu::Buffer);

impl WebGpu {
    /// The shadows a draw reads in place of its streams, made where missing or stale, which splits
    /// an open pass. None when the device was marked lost.
    pub(crate) fn vertex_shadows(&mut self, call: &DrawCall<'_, Self>) -> Option<Vec<Bound>> {
        let layout = call.vertex_layout.0.id;
        let Some(plan) = self.vertices.plans.get(&layout).cloned() else {
            return Some(Vec::new());
        };
        let mut bound = Vec::new();
        for (slot, p) in plan.iter().enumerate() {
            let (Some(p), Some(Some(vb))) = (p, call.vertex_buffers.get(slot)) else {
                continue;
            };
            let src = &vb.buffer.0;
            let key = (src.id, vb.offset, layout, slot);
            let written = src.written.get();
            if let Some(s) = self.vertices.shadows.get(&key)
                && s.written == written
            {
                bound.push((slot, s.id, s.buffer.clone()));
                continue;
            }
            let mut made = None;
            self.split_pass(SlowPath::VertexFormatConverted, |w| {
                made = Some(w.convert_stream(src, vb.offset, p));
            });
            let buffer = match made? {
                Ok(buffer) => buffer,
                Err(why) => {
                    self.fail(why);
                    return None;
                }
            };
            let id = self.id();
            bound.push((slot, id, buffer.clone()));
            self.vertices.shadows.insert(
                key,
                Shadow {
                    written,
                    id,
                    buffer,
                },
            );
        }
        Some(bound)
    }

    /// Makes the shadow of the stream at `offset` of `src`, every whole element from there to the
    /// buffer's end, in the frame's commands outside a pass.
    fn convert_stream(
        &mut self,
        src: &BufferObj,
        offset: u64,
        p: &StreamPlan,
    ) -> Result<wgpu::Buffer, &'static str> {
        let (stride, span) = (u64::from(p.src_stride), u64::from(p.span));
        let size = src.desc.size;
        let count = match (stride, size >= offset + span) {
            (_, false) => 0,
            (0, true) => 1,
            (_, true) => (size - offset - span) / stride + 1,
        };
        let shadow = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (count * u64::from(p.size)).max(4),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        if count == 0 {
            return Ok(shadow);
        }
        let raw = src.raw.borrow().clone();
        let at = src.base.get() + offset;
        let limits = self.device.limits();
        let start = at & !(u64::from(limits.min_storage_buffer_offset_alignment) - 1);
        let window = (raw.size() - start).min(limits.max_storage_buffer_binding_size & !3);
        let reach = at - start + (count - 1) * stride + span;
        let groups = count.div_ceil(GROUP);
        let across = groups.min(u64::from(limits.max_compute_workgroups_per_dimension));
        let rows = groups.div_ceil(across);
        if reach > window || rows > u64::from(limits.max_compute_workgroups_per_dimension) {
            return Err(
                "a converted vertex stream past the largest storage binding or dispatch is not built yet",
            );
        }
        let mut params = Vec::with_capacity(PARAMS);
        let head = [
            (at - start) as u32,
            p.src_stride,
            p.size,
            count as u32,
            (across * GROUP) as u32,
            p.ops.len() as u32,
            0,
            0,
        ];
        params.extend(head.iter().flat_map(|v| v.to_le_bytes()));
        params.extend(p.ops.iter().flatten().flat_map(|v| v.to_le_bytes()));
        params.resize(PARAMS, 0);
        let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: PARAMS as u64,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: true,
        });
        uniform
            .slice(..)
            .get_mapped_range_mut()
            .expect("a buffer mapped at creation")
            .copy_from_slice(&params);
        uniform.unmap();
        if self.vertices.program.is_none() {
            self.vertices.program = Some(self.convert_program());
        }
        let (pipeline, layout) = self.vertices.program.clone().expect("made above");
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &raw,
                        offset: start,
                        size: wgpu::BufferSize::new(window),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: shadow.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        src.used_in.set(self.serial);
        let mut pass = self
            .stream(Stream::Main)
            .begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(across as u32, rows as u32, 1);
        Ok(shadow)
    }

    fn convert_program(&self) -> (wgpu::ComputePipeline, wgpu::BindGroupLayout) {
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(CONVERT_WGSL.into()),
            });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
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
                    entry(0, wgpu::BufferBindingType::Storage { read_only: true }),
                    entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
                    entry(2, wgpu::BufferBindingType::Uniform),
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
        (pipeline, layout)
    }
}
