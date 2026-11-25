//! The changes our naga carries in its writers, run on the RTX 3080 (Windows) through wgpu's
//! Direct3D 12 path (naga's HLSL writer) and its Vulkan path (naga's SPIR-V writer): a 3D texture
//! sampled with a texel offset, every texel of an 8x8x8 texture of seeded bytes read through a
//! nearest sampler, against the texel each offset names, clamped to the edge; and a float's `sign`
//! taken as bits (HLSL's own `sign` gives an integer), of seeded scalars and vectors. Seeded:
//! `WCONTAINERS_WRITER_SEED` (default 1), `WCONTAINERS_WRITER_CASES` (default 4).
#![cfg(windows)]

use std::borrow::Cow;

/// Texels on each side of the texture.
const VOLUME_SIDE: u32 = 8;
/// The offset the program samples with.
const OFFSET: [i32; 3] = [1, -2, 3];
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Bytes of one texel of `FORMAT`.
fn texel_bytes() -> u32 {
    FORMAT.block_copy_size(None).expect("a plain format")
}

/// The program: each invocation samples a column of texel centres with `OFFSET`.
fn program() -> String {
    let [x, y, z] = OFFSET;
    format!(
        "
@group(0) @binding(0) var volume: texture_3d<f32>;
@group(0) @binding(1) var nearest: sampler;
@group(0) @binding(2) var<storage, read_write> out: array<u32>;

@compute @workgroup_size({VOLUME_SIDE}, {VOLUME_SIDE}, 1)
fn main(@builtin(global_invocation_id) id: vec3u) {{
    for (var z = 0u; z < {VOLUME_SIDE}u; z++) {{
        let at = (vec3f(f32(id.x), f32(id.y), f32(z)) + 0.5) / {VOLUME_SIDE}.0;
        let texel = textureSampleLevel(volume, nearest, at, 0.0, vec3i({x}, {y}, {z}));
        out[(z * {VOLUME_SIDE}u + id.y) * {VOLUME_SIDE}u + id.x] = pack4x8unorm(texel);
    }}
}}
"
    )
}

/// The device of the high-performance adapter on wgpu's `backends`.
fn device(backends: wgpu::Backends) -> (wgpu::Device, wgpu::Queue) {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("an adapter");
    pollster::block_on(adapter.request_device(&Default::default())).expect("a device")
}

/// The compute pipeline of the WGSL program `source`, through naga's module (so the carried
/// writers write it).
fn pipeline(device: &wgpu::Device, source: &str) -> wgpu::ComputePipeline {
    let module = naga::front::wgsl::parse_str(source).expect("parse");
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Naga(Cow::Owned(module)),
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &shader,
        entry_point: None,
        compilation_options: Default::default(),
        cache: None,
    })
}

/// A storage buffer of `bytes` the program writes and the test reads back.
fn storage(device: &wgpu::Device, bytes: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

/// Submits `encoder` and gives the words of `out` after it.
fn finish(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mut encoder: wgpu::CommandEncoder,
    out: &wgpu::Buffer,
) -> Vec<u32> {
    let read = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: out.size(),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    encoder.copy_buffer_to_buffer(out, 0, &read, 0, out.size());
    queue.submit([encoder.finish()]);
    read.slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.expect("mapped"));
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .expect("the queue ends");
    let mapped = read
        .slice(..)
        .get_mapped_range()
        .expect("the read buffer maps");
    mapped
        .as_chunks::<{ size_of::<u32>() }>()
        .0
        .iter()
        .map(|w| u32::from_le_bytes(*w))
        .collect()
}

/// The words the program writes on wgpu's `backends` for the texture `texels`.
fn run(backends: wgpu::Backends, texels: &[u8]) -> Vec<u32> {
    let (device, queue) = device(backends);
    let pipeline = pipeline(&device, &program());
    let extent = wgpu::Extent3d {
        width: VOLUME_SIDE,
        height: VOLUME_SIDE,
        depth_or_array_layers: VOLUME_SIDE,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        texels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(VOLUME_SIDE * texel_bytes()),
            rows_per_image: Some(VOLUME_SIDE),
        },
        extent,
    );
    let view = texture.create_view(&Default::default());
    let sampler = device.create_sampler(&Default::default());
    let bytes = u64::from(VOLUME_SIDE * VOLUME_SIDE * VOLUME_SIDE * texel_bytes());
    let out = storage(&device, bytes);
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: out.as_entire_binding(),
            },
        ],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    finish(&device, &queue, encoder, &out)
}

/// The texel each position's offset names, clamped to the edge, as one little-endian word.
fn expected(texels: &[u8]) -> Vec<u32> {
    let last = VOLUME_SIDE as i32 - 1;
    let at = |v: u32, o: i32| (v as i32 + o).clamp(0, last) as u32;
    let mut words = Vec::new();
    for z in 0..VOLUME_SIDE {
        for y in 0..VOLUME_SIDE {
            for x in 0..VOLUME_SIDE {
                let [ox, oy, oz] = OFFSET;
                let n = (at(z, oz) * VOLUME_SIDE + at(y, oy)) * VOLUME_SIDE + at(x, ox);
                let t = (n * texel_bytes()) as usize;
                words.push(u32::from_le_bytes(
                    texels[t..t + texel_bytes() as usize].try_into().unwrap(),
                ));
            }
        }
    }
    words
}

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

#[test]
fn volume_samples_with_offsets_read_the_texel_each_names() {
    let seed = knob("WCONTAINERS_WRITER_SEED", 1);
    let cases = knob("WCONTAINERS_WRITER_CASES", 4);
    let mut failed = Vec::new();
    for case in seed..seed + cases {
        let mut rng = fastrand::Rng::with_seed(case);
        let texels: Vec<u8> = (0..VOLUME_SIDE * VOLUME_SIDE * VOLUME_SIDE * texel_bytes())
            .map(|_| rng.u8(..))
            .collect();
        let want = expected(&texels);
        for (path, backends) in [
            ("direct3d 12", wgpu::Backends::DX12),
            ("vulkan", wgpu::Backends::VULKAN),
        ] {
            let got = run(backends, &texels);
            let apart = got.iter().zip(&want).filter(|(a, b)| a != b).count();
            println!("seed {case} {path}: {apart} of {} texels apart", want.len());
            if apart != 0 {
                failed.push(format!("seed {case} {path}"));
            }
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}

/// The sign program's workgroup size and workgroups.
const SIGN_GROUP: u32 = 64;
const SIGN_GROUPS: u32 = 4;
/// Inputs of the sign program, one an invocation: vectors of four floats, each giving a scalar, a
/// pair, a triple and itself to `sign`, and its first component's bits to the integer `sign`.
const SIGN_INPUTS: u32 = SIGN_GROUP * SIGN_GROUPS;
/// Words the sign program writes for each input: 1 + 2 + 3 + 4 float signs, then the integer's.
const SIGN_WORDS: u32 = 11;

/// The sign program: every `sign` result stored as its bits.
fn sign_program() -> String {
    format!(
        "
@group(0) @binding(0) var<storage, read> inputs: array<vec4f>;
@group(0) @binding(1) var<storage, read_write> out: array<u32>;

@compute @workgroup_size({SIGN_GROUP})
fn main(@builtin(global_invocation_id) id: vec3u) {{
    let v = inputs[id.x];
    let at = id.x * {SIGN_WORDS}u;
    out[at] = bitcast<u32>(sign(v.x));
    let two = bitcast<vec2u>(sign(v.xy));
    let three = bitcast<vec3u>(sign(v.xyz));
    let four = bitcast<vec4u>(sign(v));
    out[at + 1u] = two.x;
    out[at + 2u] = two.y;
    out[at + 3u] = three.x;
    out[at + 4u] = three.y;
    out[at + 5u] = three.z;
    out[at + 6u] = four.x;
    out[at + 7u] = four.y;
    out[at + 8u] = four.z;
    out[at + 9u] = four.w;
    out[at + 10u] = bitcast<u32>(sign(bitcast<i32>(v.x)));
}}
"
    )
}

/// The words the sign program writes on wgpu's `backends` for `inputs`.
fn signs(backends: wgpu::Backends, inputs: &[f32]) -> Vec<u32> {
    use wgpu::util::DeviceExt;
    let (device, queue) = device(backends);
    let pipeline = pipeline(&device, &sign_program());
    let input = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: &inputs
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect::<Vec<u8>>(),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let out = storage(
        &device,
        u64::from(SIGN_INPUTS * SIGN_WORDS) * size_of::<u32>() as u64,
    );
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: out.as_entire_binding(),
            },
        ],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(SIGN_GROUPS, 1, 1);
    }
    finish(&device, &queue, encoder, &out)
}

/// A seeded float: a zero or an infinity of either sign, or a normal number. Subnormals are left
/// out (the devices may flush them, plan 4.12) and NaN too (its sign is not defined).
fn sign_input(rng: &mut fastrand::Rng) -> f32 {
    let negative = if rng.bool() { -1.0 } else { 1.0 };
    let magnitude = match rng.u8(..4) {
        0 => 0.0,
        1 => f32::INFINITY,
        _ => loop {
            let f = f32::from_bits(rng.u32(..)).abs();
            if f.is_normal() {
                break f;
            }
        },
    };
    negative * magnitude
}

/// Each component's sign as a float of its value: 1, -1 or a zero.
fn float_sign(f: f32) -> f32 {
    if f > 0.0 {
        1.0
    } else if f < 0.0 {
        -1.0
    } else {
        0.0
    }
}

#[test]
fn a_floats_sign_is_a_float() {
    let seed = knob("WCONTAINERS_WRITER_SEED", 1);
    let cases = knob("WCONTAINERS_WRITER_CASES", 4);
    let mut failed = Vec::new();
    for case in seed..seed + cases {
        let mut rng = fastrand::Rng::with_seed(case);
        let inputs: Vec<f32> = (0..SIGN_INPUTS * 4).map(|_| sign_input(&mut rng)).collect();
        for (path, backends) in [
            ("direct3d 12", wgpu::Backends::DX12),
            ("vulkan", wgpu::Backends::VULKAN),
        ] {
            let got = signs(backends, &inputs);
            let mut apart = Vec::new();
            for (v, words) in inputs
                .as_chunks::<4>()
                .0
                .iter()
                .zip(got.chunks(SIGN_WORDS as usize))
            {
                let [x, y, z, _] = *v;
                let floats = [x, x, y, x, y, z].into_iter().chain(*v);
                for (n, (f, &word)) in floats.zip(words).enumerate() {
                    // A zero's sign may be either zero.
                    if f32::from_bits(word) != float_sign(f) {
                        apart.push((n, f, word));
                    }
                }
                let want = (x.to_bits() as i32).signum() as u32;
                if let Some(&word) = words.last().filter(|&&w| w != want) {
                    apart.push((words.len() - 1, x, word));
                }
            }
            println!(
                "seed {case} {path}: {} of {} signs apart, first {:?}",
                apart.len(),
                got.len(),
                apart.first()
            );
            if !apart.is_empty() {
                failed.push(format!("seed {case} {path}"));
            }
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}
