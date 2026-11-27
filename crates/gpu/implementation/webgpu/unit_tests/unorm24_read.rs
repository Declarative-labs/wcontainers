//! The WGSL of the 24-bit depth read (fill.rs's `unorm24_functions`) and of the float nearest a 24-bit
//! word (`NEAREST_OF_UNORM24`), run on the device for every word under both conversions a device may
//! sample stored depth by, against the values worked out here on the processor.
//!
//! A device takes only the conversion its probe found (lavapipe the product), so the other one's text
//! is reached by no draw on this host; a compute pass runs both here.
//!
//! Needs a GPU, or lavapipe as in unit_tests/scenes.rs.

use super::draw;

use crate::fill::{NEAREST_OF_UNORM24, VALUE, unorm24_functions};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use wcontainers_gpu_webgpu::WebGpu;

/// Every 24-bit word is run.
const WORDS: u32 = 1 << 24;
/// The words one dispatch runs.
const CHUNK: u32 = 1 << 22;
/// Invocations in a workgroup, and workgroups along x: one row of the dispatch runs 2^16 words.
const ROW: u32 = 256;
/// The lists the pass writes: the sampled float's bits, the word read back from it, the nearest float's bits.
const LISTS: usize = 3;
/// The word lavapipe's product samples to the float of another (1.0, as 0xffffff does).
const PRODUCT_COLLIDING: u32 = 0xff_fffe;

fn device() -> WebGpu {
    let backends = match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    };
    let (b, adapter) = pollster::block_on(draw::device(backends)).expect("a GPU adapter");
    eprintln!("adapter: {adapter}");
    b
}

/// The bits of the float nearest c / (2^24 - 1): the quotient in f64, rounded once to f32.
fn nearest_float(c: u32) -> u32 {
    ((f64::from(c) / f64::from(WORDS - 1)) as f32).to_bits()
}

/// The bits of c times the float nearest 1 / (2^24 - 1), 2^-24 (1 + 2^-23), rounded once.
fn product_float(c: u32) -> u32 {
    (c as f32 * f32::from_bits(0x3380_0001)).to_bits()
}

/// The passes for one conversion: `sample` writes, for each word c, `sampled24(c)` and the bits of the
/// nearest float of c; `read`, dispatched after it, writes `unorm24` of the float with the bits
/// `sample` wrote. A read in the same invocation would let the compiler work `unorm24` out from c
/// itself (Mesa regroups the products), which no read of a texture allows.
fn source(by_product: bool) -> String {
    format!(
        "{functions}\
         @group(0) @binding(0) var<uniform> base: vec4<u32>;\n\
         @group(0) @binding(1) var<storage, read_write> lists: array<u32>;\n\
         @compute @workgroup_size({ROW}) fn sample(@builtin(global_invocation_id) id: vec3u) {{\n\
             let i = id.x + id.y * {per_row}u;\n\
             let c = base.x + i;\n\
             lists[i] = sampled24(c);\n\
             lists[{twice}u + i] = bitcast<u32>({nearest});\n\
         }}\n\
         @compute @workgroup_size({ROW}) fn read(@builtin(global_invocation_id) id: vec3u) {{\n\
             let i = id.x + id.y * {per_row}u;\n\
             lists[{CHUNK}u + i] = unorm24(bitcast<f32>(lists[i]));\n\
         }}\n",
        functions = unorm24_functions(by_product),
        per_row = ROW * ROW,
        twice = 2 * CHUNK,
        nearest = NEAREST_OF_UNORM24.replace(VALUE, "c"),
    )
}

/// Runs the passes over every word, calling `check` with each chunk's first word and its three lists.
fn run(gpu: &WebGpu, by_product: bool, mut check: impl FnMut(u32, &[u32])) {
    let device = &gpu.device;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(source(by_product).into()),
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
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            entry(0, wgpu::BufferBindingType::Uniform),
            entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = |name| {
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some(name),
            compilation_options: Default::default(),
            cache: None,
        })
    };
    let passes = [pipeline("sample"), pipeline("read")];
    let bytes = (LISTS as u64) * u64::from(CHUNK) * 4;
    let lists = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let read = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let base = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: base.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: lists.as_entire_binding(),
            },
        ],
    });
    for first in (0..WORDS).step_by(CHUNK as usize) {
        gpu.queue
            .write_buffer(&base, 0, &[first, 0, 0, 0].map(u32::to_le_bytes).concat());
        let mut encoder = device.create_command_encoder(&Default::default());
        for pipeline in &passes {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(ROW, CHUNK / (ROW * ROW), 1);
        }
        encoder.copy_buffer_to_buffer(&lists, 0, &read, 0, bytes);
        let index = gpu.queue.submit([encoder.finish()]);
        let mapped = Arc::new(AtomicBool::new(false));
        let flag = mapped.clone();
        read.slice(..).map_async(wgpu::MapMode::Read, move |_| {
            flag.store(true, Ordering::Release);
        });
        crate::wait::until(device, Some(index), || mapped.load(Ordering::Acquire));
        let words: Vec<u32> = read
            .slice(..)
            .get_mapped_range()
            .expect("the read buffer maps")
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        read.unmap();
        check(first, &words);
    }
}

/// The words of each list that differ from the right value, at most eight of each, as (word, got,
/// right), and how many differ in all.
#[derive(Debug, Default, PartialEq, Eq)]
struct Wrong {
    count: [u32; LISTS],
    first: [Vec<(u32, u32, u32)>; LISTS],
}

impl Wrong {
    fn note(&mut self, list: usize, word: u32, got: u32, right: u32) {
        if got != right {
            self.count[list] += 1;
            if self.first[list].len() < 8 {
                self.first[list].push((word, got, right));
            }
        }
    }
}

/// Runs both conversions and states, for every word c: the sampled float is the nearest float of c
/// (Direct3D's conversion) or c times the rounded reciprocal (lavapipe's); the read gives back c, but
/// for the one word the product samples as another's float, which reads as that word (0xffffff); and
/// the nearest float WGSL works out by integers is the float nearest c / (2^24 - 1).
fn check(by_product: bool) -> Wrong {
    let gpu = device();
    let mut wrong = Wrong::default();
    run(&gpu, by_product, |first, lists| {
        let n = CHUNK as usize;
        for i in 0..CHUNK {
            let c = first + i;
            let k = i as usize;
            let sampled = if by_product {
                product_float(c)
            } else {
                nearest_float(c)
            };
            let read = if by_product && c == PRODUCT_COLLIDING {
                WORDS - 1
            } else {
                c
            };
            wrong.note(0, c, lists[k], sampled);
            wrong.note(1, c, lists[n + k], read);
            wrong.note(2, c, lists[2 * n + k], nearest_float(c));
        }
    });
    wrong
}

#[test]
fn every_24_bit_word_reads_back_as_itself_where_a_device_samples_the_nearest_float() {
    assert_eq!(check(false), Wrong::default());
}

#[test]
fn every_24_bit_word_reads_back_as_itself_but_one_where_a_device_samples_the_product() {
    assert_eq!(check(true), Wrong::default());
}
