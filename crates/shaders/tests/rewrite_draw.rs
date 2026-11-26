//! `rewrite`'s results against the native features on the RTX 3080 (Windows, wgpu's Vulkan path): a
//! program sampling, gathering and loading from arrays of textures and samplers picked at run time,
//! its gather's offset given at run time, run once with wgpu's binding arrays and a constant offset,
//! and once rewritten (an element per binding, the offset moved into the coordinate). Every value
//! must be equal. The inputs are a read-only storage buffer: wgpu takes no uniform buffer beside a
//! binding array in one group. Seeded: `WCONTAINERS_REWRITE_SEED` (default 1), `WCONTAINERS_REWRITE_CASES`
//! (default 16).
#![cfg(windows)]

use std::borrow::Cow;

use naga::{Expression, Module};
use wcontainers_gpu::{
    reflect::Reflection, reflect::SLOT_KINDS, reflect::SlotKind, reflect::SourceStage,
    reflect::StageExtras, reflect::VariantKey,
};
use wcontainers_shaders::{ENTRY_POINT, cache, rewrite};

/// Texels on a side of each texture, a power of two so a moved coordinate is exact.
const SIZE: u32 = 8;
/// Textures and samplers in each array.
const ARRAY_LENGTH: usize = 4;
/// The program's outputs: the sample, the gather, the load and the offset read.
const PROGRAM_OUTPUTS: usize = 4;
const VEC4_BYTES: u64 = 16;
const TEXEL_BYTES: u32 = 4;
/// The steps a coordinate takes across a texel.
const QUARTERS: u32 = 4;
/// The range of offsets Vulkan guarantees a gather takes.
const OFFSETS: std::ops::RangeInclusive<i32> = -8..=7;

const SOURCE: &str = "
enable wgpu_binding_array;
struct U { i: u32, pad: u32, o: vec2<i32>, uv: vec2<f32> }
@group(0) @binding(UNIFORM) var<storage, read> u: U;
@group(0) @binding(SAMPLER) var s: binding_array<sampler, ARRAY_LENGTH>;
@group(0) @binding(TEXTURE) var t: binding_array<texture_2d<f32>, ARRAY_LENGTH>;
@group(0) @binding(STORAGE) var<storage, read_write> out: array<vec4<f32>>;
@compute @workgroup_size(1) fn ENTRY() {
    let i = u.i;
    let o = u.o;
    out[3] = vec4<f32>(vec2<f32>(o), 0.0, 0.0);
    out[0] = textureSampleLevel(t[i], s[i], u.uv, 0.0);
    out[1] = textureGather(1, t[i], s[i], u.uv, vec2(OX, OY));
    out[2] = textureLoad(t[i], vec2<u32>(u.uv * SIZEf), 0);
}
";

fn parse(offset: [i32; 2]) -> Module {
    let source = SOURCE
        .replace(
            "UNIFORM",
            &SLOT_KINDS[SlotKind::Uniform as usize]
                .first_binding
                .to_string(),
        )
        .replace(
            "SAMPLER",
            &SLOT_KINDS[SlotKind::Sampler as usize]
                .first_binding
                .to_string(),
        )
        .replace(
            "TEXTURE",
            &SLOT_KINDS[SlotKind::Texture as usize]
                .first_binding
                .to_string(),
        )
        .replace(
            "STORAGE",
            &SLOT_KINDS[SlotKind::Storage as usize]
                .first_binding
                .to_string(),
        )
        .replace("ARRAY_LENGTH", &ARRAY_LENGTH.to_string())
        .replace("SIZE", &SIZE.to_string())
        .replace("OX", &offset[0].to_string())
        .replace("OY", &offset[1].to_string())
        .replace("ENTRY", ENTRY_POINT);
    naga::front::wgsl::parse_str(&source).expect("the test's WGSL parses")
}

/// The program rewritten with its gather's offset read from `u.o` at run time.
fn rewritten() -> Module {
    let mut module = parse([0, 0]);
    let f = &mut module.entry_points[0].function;
    // `u.o`'s load: the second load, after `u.i`'s.
    let load = f
        .expressions
        .iter()
        .filter(|(_, e)| matches!(e, Expression::Load { .. }))
        .nth(1)
        .map(|(h, _)| h)
        .expect("the offset's load");
    for (_, e) in f.expressions.iter_mut() {
        if let Expression::ImageSample {
            gather: Some(_),
            offset,
            ..
        } = e
        {
            *offset = Some(load);
        }
    }
    let reflection = Reflection {
        stage: SourceStage::Compute,
        inputs: Vec::new(),
        outputs: Vec::new(),
        bindings: Vec::new(),
        helpers: Vec::new(),
        helper_size: 0,
        sampled_pairs: Vec::new(),
        extras: StageExtras::Compute {
            workgroup_size: module.entry_points[0].workgroup_size,
        },
        slow_paths: Vec::new(),
    };
    let route_key = [0; cache::KEY_BYTES];
    let converted =
        rewrite(module, reflection, &VariantKey::default(), &route_key).expect("rewritten");
    assert!(converted.reflection.helpers.is_empty());
    converted
        .modules
        .into_iter()
        .next()
        .expect("one module")
        .module
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    views: Vec<wgpu::TextureView>,
    samplers: Vec<wgpu::Sampler>,
}

fn gpu(rng: &mut fastrand::Rng) -> Gpu {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::VULKAN;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("an adapter");
    eprintln!("adapter: {}", adapter.get_info().name);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TEXTURE_BINDING_ARRAY,
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .expect("a device with binding arrays");
    let extent = wgpu::Extent3d {
        width: SIZE,
        height: SIZE,
        depth_or_array_layers: 1,
    };
    let views = (0..ARRAY_LENGTH)
        .map(|_| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let texels: Vec<u8> = (0..SIZE * SIZE * TEXEL_BYTES).map(|_| rng.u8(..)).collect();
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &texels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(SIZE * TEXEL_BYTES),
                    rows_per_image: None,
                },
                extent,
            );
            texture.create_view(&Default::default())
        })
        .collect();
    use wgpu::{AddressMode as A, FilterMode as F};
    let kinds = [
        (F::Nearest, A::ClampToEdge),
        (F::Linear, A::Repeat),
        (F::Nearest, A::MirrorRepeat),
        (F::Linear, A::ClampToEdge),
    ];
    let samplers = kinds
        .iter()
        .map(|&(filter, address)| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: address,
                address_mode_v: address,
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            })
        })
        .collect();
    Gpu {
        device,
        queue,
        views,
        samplers,
    }
}

/// Runs `module` once with the uniform block `uniform`; `arrays` binds the textures and samplers as
/// binding arrays, otherwise an element per binding. Gives the outputs' bits.
fn run(g: &Gpu, module: Module, arrays: bool, uniform: &[u8]) -> Vec<u32> {
    let shader = g.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Naga(Cow::Owned(module)),
    });
    let entry = |binding, ty, count| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty,
        count,
    };
    let mut layout_entries = vec![
        entry(
            SLOT_KINDS[SlotKind::Uniform as usize].first_binding,
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            None,
        ),
        entry(
            SLOT_KINDS[SlotKind::Storage as usize].first_binding,
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            None,
        ),
    ];
    let sampler = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
    let texture = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    };
    if arrays {
        let count = std::num::NonZeroU32::new(ARRAY_LENGTH as u32);
        layout_entries.push(entry(
            SLOT_KINDS[SlotKind::Sampler as usize].first_binding,
            sampler,
            count,
        ));
        layout_entries.push(entry(
            SLOT_KINDS[SlotKind::Texture as usize].first_binding,
            texture,
            count,
        ));
    } else {
        for k in 0..ARRAY_LENGTH as u32 {
            layout_entries.push(entry(
                SLOT_KINDS[SlotKind::Sampler as usize].first_binding + k,
                sampler,
                None,
            ));
            layout_entries.push(entry(
                SLOT_KINDS[SlotKind::Texture as usize].first_binding + k,
                texture,
                None,
            ));
        }
    }
    let group_layout = g
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &layout_entries,
        });
    let layout = g
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&group_layout)],
            immediate_size: 0,
        });
    let pipeline = g
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&layout),
            module: &shader,
            entry_point: None,
            compilation_options: Default::default(),
            cache: None,
        });
    let buffer = |size: u64, usage| {
        g.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage,
            mapped_at_creation: false,
        })
    };
    use wgpu::BufferUsages as B;
    let constants = buffer(uniform.len() as u64, B::STORAGE | B::COPY_DST);
    g.queue.write_buffer(&constants, 0, uniform);
    let bytes = PROGRAM_OUTPUTS as u64 * VEC4_BYTES;
    let out = buffer(bytes, B::STORAGE | B::COPY_SRC);
    let read = buffer(bytes, B::MAP_READ | B::COPY_DST);
    let views: Vec<&wgpu::TextureView> = g.views.iter().collect();
    let samplers: Vec<&wgpu::Sampler> = g.samplers.iter().collect();
    let mut entries = vec![
        wgpu::BindGroupEntry {
            binding: SLOT_KINDS[SlotKind::Uniform as usize].first_binding,
            resource: constants.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: SLOT_KINDS[SlotKind::Storage as usize].first_binding,
            resource: out.as_entire_binding(),
        },
    ];
    if arrays {
        entries.push(wgpu::BindGroupEntry {
            binding: SLOT_KINDS[SlotKind::Sampler as usize].first_binding,
            resource: wgpu::BindingResource::SamplerArray(&samplers),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: SLOT_KINDS[SlotKind::Texture as usize].first_binding,
            resource: wgpu::BindingResource::TextureViewArray(&views),
        });
    } else {
        for k in 0..ARRAY_LENGTH as u32 {
            entries.push(wgpu::BindGroupEntry {
                binding: SLOT_KINDS[SlotKind::Sampler as usize].first_binding + k,
                resource: wgpu::BindingResource::Sampler(samplers[k as usize]),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: SLOT_KINDS[SlotKind::Texture as usize].first_binding + k,
                resource: wgpu::BindingResource::TextureView(views[k as usize]),
            });
        }
    }
    let group = g.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &group_layout,
        entries: &entries,
    });
    let mut encoder = g.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out, 0, &read, 0, bytes);
    g.queue.submit([encoder.finish()]);
    read.slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.expect("mapped"));
    g.device
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

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[test]
fn rewritten_arrays_and_gathers_compute_what_the_native_features_do() {
    let seed = knob("WCONTAINERS_REWRITE_SEED", 1);
    let cases = knob("WCONTAINERS_REWRITE_CASES", 16);
    let mut rng = fastrand::Rng::with_seed(seed);
    let g = gpu(&mut rng);
    let ours = rewritten();
    let mut differing = 0;
    for case in 0..cases {
        let i = rng.u32(..ARRAY_LENGTH as u32);
        let o = [rng.i32(OFFSETS), rng.i32(OFFSETS)];
        // Coordinates a quarter texel from every texel's edge and centre, so no rounding decides
        // which texels a moved coordinate picks.
        let quarter = |rng: &mut fastrand::Rng| {
            let quarters = (rng.u32(..SIZE * QUARTERS) | 1) as f32;
            quarters / (SIZE * QUARTERS) as f32
        };
        let uv = [quarter(&mut rng), quarter(&mut rng)];
        let mut uniform = Vec::new();
        uniform.extend(i.to_le_bytes());
        uniform.extend(0u32.to_le_bytes());
        uniform.extend(o[0].to_le_bytes());
        uniform.extend(o[1].to_le_bytes());
        uniform.extend(uv[0].to_le_bytes());
        uniform.extend(uv[1].to_le_bytes());
        let native = run(&g, parse(o), true, &uniform);
        let rewritten = run(&g, ours.clone(), false, &uniform);
        let same = native == rewritten;
        if !same {
            differing += 1;
        }
        eprintln!(
            "case {case} (seed {seed}): element {i}, offset {o:?}, uv {uv:?}: {}",
            if same { "equal" } else { "DIFFERENT" }
        );
        if !same {
            eprintln!("  native    {native:x?}\n  rewritten {rewritten:x?}");
        }
    }
    eprintln!("{cases} cases, {differing} differing");
    assert_eq!(differing, 0);
}
