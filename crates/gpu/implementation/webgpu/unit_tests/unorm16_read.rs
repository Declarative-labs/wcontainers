//! The WGSL of the 16-bit depth read (fill.rs's `unorm16_of_sample`), run on the device for every word
//! under both conversions a device may sample stored 16-bit depth by, against the word itself.
//!
//! A device takes only its own conversion, so the other one's floats reach no draw on this host; a
//! compute pass reads both here.
//!
//! Needs a GPU, or lavapipe as in unit_tests/scenes.rs.

use super::draw;

use crate::fill::unorm16_of_sample;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use wcontainers_gpu_webgpu::WebGpu;

/// Every 16-bit word is run.
const WORDS: u32 = 1 << 16;
/// Invocations in a workgroup.
const GROUP: u32 = 256;

fn device() -> WebGpu {
    let backends = match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    };
    pollster::block_on(draw::device(backends))
        .expect("a GPU adapter")
        .0
}

/// The float a device samples stored 16-bit depth k as: the float nearest k / (2^16 - 1) (Direct3D's
/// rule), or k times the float nearest 1 / (2^16 - 1), rounded once (a product, as llvmpipe's).
fn sampled(k: u32, by_product: bool) -> f32 {
    let most = f64::from(WORDS - 1);
    if by_product {
        k as f32 * (1.0 / most) as f32
    } else {
        (f64::from(k) / most) as f32
    }
}

/// Reads each float of `floats` as the 16-bit read does, on the device.
fn read(gpu: &WebGpu, floats: &[f32]) -> Vec<u32> {
    let device = &gpu.device;
    let count = floats.len() as u32;
    let source = format!(
        "@group(0) @binding(0) var<storage, read_write> lists: array<u32>;\n\
         @compute @workgroup_size({GROUP}) fn read(@builtin(global_invocation_id) id: vec3u) {{\n\
             let x = bitcast<f32>(lists[id.x]);\n\
             lists[{count}u + id.x] = {value};\n\
         }}\n",
        value = unorm16_of_sample("x"),
    );
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: Some("read"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bytes = u64::from(count) * 8;
    let lists = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let out = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let input: Vec<u8> = floats
        .iter()
        .flat_map(|f| f.to_bits().to_le_bytes())
        .collect();
    gpu.queue.write_buffer(&lists, 0, &input);
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: lists.as_entire_binding(),
        }],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(count / GROUP, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&lists, 0, &out, 0, bytes);
    let index = gpu.queue.submit([encoder.finish()]);
    let mapped = Arc::new(AtomicBool::new(false));
    let flag = mapped.clone();
    out.slice(..).map_async(wgpu::MapMode::Read, move |_| {
        flag.store(true, Ordering::Release);
    });
    crate::wait::until(device, Some(index), || mapped.load(Ordering::Acquire));
    let words: Vec<u32> = out
        .slice(..)
        .get_mapped_range()
        .expect("the read buffer maps")
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect();
    words[count as usize..].to_vec()
}

/// The words, under one conversion, that read back as another, at most eight as (word, read), and
/// how many in all.
fn wrong(by_product: bool) -> (usize, Vec<(u32, u32)>) {
    let gpu = device();
    let floats: Vec<f32> = (0..WORDS).map(|k| sampled(k, by_product)).collect();
    let wrong: Vec<(u32, u32)> = read(&gpu, &floats)
        .into_iter()
        .zip(0..WORDS)
        .filter(|(got, k)| got != k)
        .map(|(got, k)| (k, got))
        .collect();
    (wrong.len(), wrong.into_iter().take(8).collect())
}

#[test]
fn every_16_bit_word_reads_back_as_itself_where_a_device_samples_the_nearest_float() {
    assert_eq!(wrong(false), (0, Vec::new()));
}

#[test]
fn every_16_bit_word_reads_back_as_itself_where_a_device_samples_a_product() {
    assert_eq!(wrong(true), (0, Vec::new()));
}
