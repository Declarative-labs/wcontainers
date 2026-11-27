//! Vertex formats WebGPU lacks (plan 5.7, slow path `VertexFormatConverted`): a stream with one is
//! read through a shadow stream, every attribute of the stream laid out again in formats WebGPU
//! has, made by a compute program before the draw and kept until the source buffer is written.

use crate::draw::split_pass;
use crate::{buffer_of, held};
use std::collections::HashMap;
use std::rc::Rc;

use wcontainers_gpu::{
    draw::DrawCall, slow::SlowPath, state::VertexFormat, state::VertexLayoutDesc,
};

use crate::{BufferObj, GROUP, Stream, WebGpu, convert, with_group};
use wcontainers_gpu::error::GpuError;

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

@compute @workgroup_size(GROUP)
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
        if o.z == Bytes {
            // Bytes as they are, the last word padded with zeros.
            for (var k = 0u; k < o.w; k += 4u) {
                var v = 0u;
                for (var j = 0u; j < min(4u, o.w - k); j++) {
                    v |= byte(a + k + j) << (j * 8u);
                }
                dst[w + k / 4u] = v;
            }
        } else if o.z == ThreeBytes {
            // Three bytes and the fourth component's one.
            dst[w] = byte(a) | (byte(a + 1u) << 8u) | (byte(a + 2u) << 16u) | (o.w << 24u);
        } else if o.z == ThreeHalves {
            // Three 16-bit values and the fourth component's one.
            dst[w] = word(a);
            dst[w + 1u] = byte(a + 4u) | (byte(a + 5u) << 8u) | (o.w << 16u);
        } else if o.z == Uint10_10_10_2 {
            // 10, 10, 10 and 2 unsigned bits as four 16-bit values.
            let v = word(a);
            dst[w] = (v & 0x3ffu) | (((v >> 10u) & 0x3ffu) << 16u);
            dst[w + 1u] = ((v >> 20u) & 0x3ffu) | ((v >> 30u) << 16u);
        } else {
            // 11, 11 and 10-bit floats as 16-bit floats, and one.
            let v = word(a);
            dst[w] = half(v, 6u) | (half(v >> 11u, 6u) << 16u);
            dst[w + 1u] = half(v >> 22u, 5u) | (0x3c00u << 16u);
        }
    }
}
"#;

/// Bytes of the program's parameters: eight words and 32 operations.
const PARAMS: usize = 32 + 32 * 16;

/// One converted stream: its shadow element and how it is made.
#[derive(Debug)]
pub(crate) struct StreamPlan {
    /// Bytes of a shadow element.
    size: u32,
    /// The source's stride; zero repeats one element.
    src_stride: u32,
    /// Source bytes an element reaches from its start.
    span: u32,
    /// One a attribute, as the program's `vec4u` operations.
    ops: Vec<Operation>,
    /// The attributes as the pipeline reads them from the shadow.
    pub(crate) attributes: Vec<wgpu::VertexAttribute>,
}

impl StreamPlan {
    /// The shadow stream's stride.
    pub(crate) fn stride(&self) -> u64 {
        if self.src_stride == 0 {
            0
        } else {
            u64::from(self.size)
        }
    }
}

/// The streams of one vertex layout that are read through shadows, by stream.
pub(crate) type Plan = Rc<[Option<StreamPlan>]>;

/// What the program does with one attribute (`Operation::code`).
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
enum Code {
    Bytes,
    ThreeBytes,
    ThreeHalves,
    Uint10_10_10_2,
    Float11_11_10,
}

impl Code {
    const ALL: &[Code] = &[
        Code::Bytes,
        Code::ThreeBytes,
        Code::ThreeHalves,
        Code::Uint10_10_10_2,
        Code::Float11_11_10,
    ];
}

/// The program's text with each code defined under its name.
fn program() -> String {
    let codes: String = Code::ALL
        .iter()
        .map(|&code| format!("const {code:?}: u32 = {}u;\n", code as u32))
        .collect();
    with_group(&(codes + CONVERT_WGSL))
}

/// One attribute's operation, as the program reads it.
#[derive(Clone, Copy, Debug)]
struct Operation {
    source_offset: u32,
    shadow_offset: u32,
    code: Code,
    /// Bytes to copy for `Code::Bytes`; otherwise the fourth component's bits.
    argument: u32,
}

impl Operation {
    fn words(self) -> [u32; size_of::<Self>() / size_of::<u32>()] {
        [
            self.source_offset,
            self.shadow_offset,
            self.code as u32,
            self.argument,
        ]
    }
}

/// A lacking format: the format it is read as, the program's code and argument (the fourth
/// component's bits), and the bytes it takes in the source.
struct Widened {
    format: wgpu::VertexFormat,
    code: Code,
    argument: u32,
    source_bytes: u32,
}

/// Three components read as the four of `four`, of which `one` is a single one.
fn three_of(
    four: wgpu::VertexFormat,
    one: wgpu::VertexFormat,
    code: Code,
    argument: u32,
) -> Widened {
    Widened {
        format: four,
        code,
        argument,
        source_bytes: (four.size() - one.size()) as u32,
    }
}

/// Bits packed into one word, read as `format`.
fn packed(format: wgpu::VertexFormat, code: Code) -> Widened {
    Widened {
        format,
        code,
        argument: 0,
        source_bytes: size_of::<u32>() as u32,
    }
}

fn widened(f: VertexFormat) -> Option<Widened> {
    use Code::{ThreeBytes, ThreeHalves};
    use VertexFormat::*;
    use wgpu::VertexFormat as W;
    Some(match f {
        Uint8x3 => three_of(W::Uint8x4, W::Uint8, ThreeBytes, 1),
        Sint8x3 => three_of(W::Sint8x4, W::Sint8, ThreeBytes, 1),
        Unorm8x3 => three_of(W::Unorm8x4, W::Unorm8, ThreeBytes, u8::MAX.into()),
        Snorm8x3 => three_of(W::Snorm8x4, W::Snorm8, ThreeBytes, i8::MAX as u32),
        Uint16x3 => three_of(W::Uint16x4, W::Uint16, ThreeHalves, 1),
        Sint16x3 => three_of(W::Sint16x4, W::Sint16, ThreeHalves, 1),
        Unorm16x3 => three_of(W::Unorm16x4, W::Unorm16, ThreeHalves, u16::MAX.into()),
        Snorm16x3 => three_of(W::Snorm16x4, W::Snorm16, ThreeHalves, i16::MAX as u32),
        Float16x3 => three_of(
            W::Float16x4,
            W::Float16,
            ThreeHalves,
            half::f16::ONE.to_bits().into(),
        ),
        Uint10_10_10_2 => packed(W::Uint16x4, Code::Uint10_10_10_2),
        Float11_11_10 => packed(W::Float16x4, Code::Float11_11_10),
        // Its three bytes and a fourth of one, read as BGRA.
        Bgrx8Unorm => three_of(W::Unorm8x4Bgra, W::Unorm8, ThreeBytes, u8::MAX.into()),
        _ => return None,
    })
}

/// The shadow streams of a layout, or None when WebGPU reads every stream as it is.
pub(crate) fn plan(desc: &VertexLayoutDesc) -> Option<Plan> {
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
                let w = match widened(a.format) {
                    Some(w) => w,
                    None => {
                        let format = convert::vertex_format(a.format)?;
                        let bytes = format.size() as u32;
                        Widened {
                            format,
                            code: Code::Bytes,
                            argument: bytes,
                            source_bytes: bytes,
                        }
                    }
                };
                ops.push(Operation {
                    source_offset: a.offset,
                    shadow_offset: size,
                    code: w.code,
                    argument: w.argument,
                });
                attributes.push(wgpu::VertexAttribute {
                    format: w.format,
                    offset: u64::from(size),
                    shader_location: a.location,
                });
                size += (w.format.size() as u32).next_multiple_of(wgpu::VERTEX_ALIGNMENT as u32);
                span = span.max(a.offset + w.source_bytes);
            }
            Some(StreamPlan {
                size: size.max(wgpu::VERTEX_ALIGNMENT as u32),
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
pub(crate) struct Vertices {
    program: Option<(wgpu::ComputePipeline, wgpu::BindGroupLayout)>,
    pub(crate) plans: HashMap<u64, Plan>,
    /// By source buffer, offset, layout and stream.
    shadows: HashMap<(u64, u64, u64, usize), Shadow>,
}

impl Vertices {
    /// Drops the shadows of a destroyed buffer.
    pub(crate) fn forget(&mut self, buffer: u64) {
        if !self.shadows.is_empty() {
            self.shadows.retain(|k, _| k.0 != buffer);
        }
    }

    /// Drops the plan and shadows of a destroyed vertex layout.
    pub(crate) fn forget_layout(&mut self, layout: u64) {
        if self.plans.remove(&layout).is_some() {
            self.shadows.retain(|k, _| k.2 != layout);
        }
    }
}

/// The shadow bound in place of a stream: its slot, a number no other buffer has, and the buffer.
pub(crate) type Bound = (usize, u64, wgpu::Buffer);

/// The shadows a draw reads in place of its streams, made where missing or stale, which splits
/// an open pass. None when the device was marked lost.
pub(crate) fn vertex_shadows(gpu: &mut WebGpu, call: &DrawCall<'_, WebGpu>) -> Option<Vec<Bound>> {
    let Some(vertex_layout) = held(&gpu.objects.vertex_layouts, call.vertex_layout.0) else {
        gpu.fail(GpuError::GONE);
        return None;
    };
    let layout = vertex_layout.0.id;
    let Some(plan) = gpu.vertices.plans.get(&layout).cloned() else {
        return Some(Vec::new());
    };
    let mut bound = Vec::new();
    for (slot, p) in plan.iter().enumerate() {
        let (Some(p), Some(Some(vb))) = (p, call.vertex_buffers.get(slot)) else {
            continue;
        };
        let Some(src) = buffer_of(gpu, vb.buffer) else {
            gpu.fail(GpuError::GONE);
            return None;
        };
        let key = (src.id, vb.offset, layout, slot);
        let written = src.written.get();
        if let Some(s) = gpu.vertices.shadows.get(&key)
            && s.written == written
        {
            bound.push((slot, s.id, s.buffer.clone()));
            continue;
        }
        let mut made = None;
        split_pass(gpu, SlowPath::VertexFormatConverted, |w| {
            made = Some(convert_stream(w, &src, vb.offset, p));
        });
        let buffer = match made? {
            Ok(buffer) => buffer,
            Err(why) => {
                gpu.fail(why);
                return None;
            }
        };
        let id = gpu.id();
        bound.push((slot, id, buffer.clone()));
        gpu.vertices.shadows.insert(
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
    gpu: &mut WebGpu,
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
    let shadow = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (count * u64::from(p.size)).max(wgpu::COPY_BUFFER_ALIGNMENT),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    if count == 0 {
        return Ok(shadow);
    }
    let raw = src.raw.borrow().clone();
    let at = src.base.get() + offset;
    let limits = gpu.device.limits();
    let start = at & !(u64::from(limits.min_storage_buffer_offset_alignment) - 1);
    let window = (raw.size() - start)
        .min(limits.max_storage_buffer_binding_size & !(wcontainers_gpu::STORAGE_WORD - 1));
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
    params.extend(
        p.ops
            .iter()
            .flat_map(|o| o.words())
            .flat_map(u32::to_le_bytes),
    );
    params.resize(PARAMS, 0);
    let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
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
    if gpu.vertices.program.is_none() {
        gpu.vertices.program = Some(convert_program(gpu));
    }
    let (pipeline, layout) = gpu.vertices.program.clone().expect("made above");
    let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
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
    src.used_in.set(gpu.serial);
    let mut pass = gpu
        .stream(Stream::Main)
        .begin_compute_pass(&wgpu::ComputePassDescriptor::default());
    pass.set_pipeline(&pipeline);
    pass.set_bind_group(0, &group, &[]);
    pass.dispatch_workgroups(across as u32, rows as u32, 1);
    Ok(shadow)
}

fn convert_program(gpu: &WebGpu) -> (wgpu::ComputePipeline, wgpu::BindGroupLayout) {
    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(program().into()),
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
    let layout = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                entry(0, wgpu::BufferBindingType::Storage { read_only: true }),
                entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
                entry(2, wgpu::BufferBindingType::Uniform),
            ],
        });
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: None,
            compilation_options: Default::default(),
            cache: None,
        });
    (pipeline, layout)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The conversion program, its codes defined, is valid WGSL for WebGPU.
    #[test]
    fn conversion_program_is_valid() {
        let source = program();
        let module = naga::front::wgsl::parse_str(&source)
            .unwrap_or_else(|e| panic!("{}\n{source}", e.message()));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{e:?}\n{source}"));
    }

    /// A lacking format's source bytes are its components' bytes.
    #[test]
    fn lacking_formats_take_their_bytes_in_the_source() {
        use VertexFormat::*;
        for (format, components, component) in [
            (Uint8x3, 3, size_of::<u8>()),
            (Snorm8x3, 3, size_of::<i8>()),
            (Unorm16x3, 3, size_of::<u16>()),
            (Float16x3, 3, size_of::<half::f16>()),
            (Uint10_10_10_2, 1, size_of::<u32>()),
            (Float11_11_10, 1, size_of::<u32>()),
            (Bgrx8Unorm, 3, size_of::<u8>()),
        ] {
            let w = widened(format).unwrap_or_else(|| panic!("{format:?} is not widened"));
            assert_eq!(
                w.source_bytes as usize,
                components * component,
                "{format:?}"
            );
        }
    }
}
