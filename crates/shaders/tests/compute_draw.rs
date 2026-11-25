//! Compute shaders with atomic operations, append, consume and counter views, storage textures
//! (`compute.hlsl`), raw and structured shader resource views (`readonly.hlsl`), division and an escape loop (`divide.hlsl`), NaN rules (`compare.hlsl`) and typed buffers (`texels.hlsl`, and `cs_texels` written as another route hands it to `wcontainers_shaders::typed`) on the WebGPU back end, against real Direct3D 11
//! running the same shaders on the same GPU. The values consumed and the texels read are seeded: `WCONTAINERS_COMPUTE_SEED` (default 1) and `WCONTAINERS_COMPUTE_CASES` seeds
//! from it (default 4).

#![cfg(windows)]

mod common;
mod forms;

use common::*;
use wcontainers_gpu::{
    AddressMode, Backend, BindingEntry, BindingResource, BorderColor, BufferRange, BufferUsage,
    DispatchCall, DispatchGroups, Extent, FORMAT_FACTS, Filter, Format, HELPER_SET, HelperValue,
    Origin, Program, ReadSource, SamplerDesc, ShaderStage, SlotKind, StageSet, TexelBufferRoute,
    TextureDesc, TextureDimension, TextureRegion, TextureUsage, TextureViewDesc, VariantKey,
    ViewDimension, WriteMode, helper_binding,
};
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::{ConvertError, Source, convert};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

/// One buffer view: its buffer's bytes before the dispatch, the view's first element (4 bytes)
/// in it, whether it is a raw buffer, the count its counter starts at (none: no counter), whether
/// it is an append or consume view, and whether it is a shader resource view (read only) rather
/// than an unordered access one.
#[derive(Clone)]
struct View {
    bytes: Vec<u8>,
    first: u32,
    raw: bool,
    counter: Option<u32>,
    append: bool,
    read: bool,
}

/// The view a binding of the program stands for: the one of its kind (shader resource views at
/// texture slots, unordered access views and their counters at the others) at its register.
fn view_of(case: &Case, r: &wcontainers_gpu::ReflectedBinding) -> usize {
    let read = r.kind == SlotKind::Texture;
    case.views
        .iter()
        .enumerate()
        .filter(|(_, v)| v.read == read)
        .nth(r.register as usize)
        .map(|(i, _)| i)
        .unwrap()
}

/// One case: a shader, its views by slot and its workgroups.
#[derive(Clone)]
struct Case {
    shader: &'static str,
    views: Vec<View>,
    groups: u32,
}

fn words(w: &[u32]) -> Vec<u8> {
    w.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn rng(seed: u64) -> impl FnMut() -> u32 {
    let mut s = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 16) as u32
    }
}

fn cases(seed: u64) -> Vec<Case> {
    let mut next = rng(seed);
    let mut raw = vec![0u32; 2048];
    raw[642] = !0;
    raw[645] = !0;
    // The signed minimum and maximum on the raw view's unsigned words.
    raw[649] = i32::MAX as u32;
    raw[650] = i32::MIN as u32;
    let consumed: Vec<u32> = (0..256).map(|_| next() >> 4).collect();
    let returned: Vec<u32> = (0..32).map(|_| next() >> 4).collect();
    let structured: Vec<u32> = (0..512).map(|_| next()).collect();
    let raw_read: Vec<u32> = (0..512).map(|_| next()).collect();
    let operands = division_operands(&mut next, DIVIDE_GROUPS * DIVIDE_GROUP_SIZE);
    let points = escape_operands(&mut next, DIVIDE_GROUPS * DIVIDE_GROUP_SIZE);
    vec![
        Case {
            shader: "cs_atomics",
            views: vec![
                View {
                    bytes: words(&raw),
                    raw: true,
                    counter: None,
                    append: false,
                    read: false,
                    first: 0,
                },
                View {
                    bytes: words(&[i32::MAX as u32, i32::MIN as u32, 5, 0]),
                    raw: false,
                    counter: None,
                    append: false,
                    read: false,
                    first: 0,
                },
                View {
                    bytes: vec![0; 2048],
                    raw: false,
                    counter: Some(0),
                    append: false,
                    read: false,
                    first: 0,
                },
                View {
                    bytes: vec![0; 2048],
                    raw: false,
                    counter: Some(0),
                    append: true,
                    read: false,
                    first: 0,
                },
                View {
                    bytes: words(&consumed),
                    raw: false,
                    counter: Some(256),
                    append: true,
                    read: false,
                    first: 0,
                },
            ],
            groups: 4,
        },
        Case {
            shader: "cs_shared",
            views: vec![View {
                bytes: vec![0; 2048],
                raw: true,
                counter: None,
                append: false,
                read: false,
                first: 0,
            }],
            groups: 2,
        },
        Case {
            shader: "cs_decrement",
            views: vec![View {
                bytes: words(&returned),
                raw: false,
                counter: Some(32),
                append: false,
                read: false,
                first: 0,
            }],
            groups: 1,
        },
        Case {
            shader: "cs_readonly",
            views: vec![
                View {
                    bytes: vec![0; 2048],
                    raw: false,
                    counter: None,
                    append: false,
                    read: false,
                    first: 0,
                },
                View {
                    bytes: words(&structured),
                    raw: false,
                    counter: None,
                    append: false,
                    read: true,
                    first: 0,
                },
                View {
                    bytes: words(&raw_read),
                    raw: true,
                    counter: None,
                    append: false,
                    read: true,
                    first: 0,
                },
            ],
            groups: 2,
        },
        Case {
            shader: "cs_divide",
            views: vec![
                View {
                    bytes: vec![0; size_of_val(operands.as_slice())],
                    raw: false,
                    counter: None,
                    append: false,
                    read: false,
                    first: 0,
                },
                View {
                    bytes: words(&operands),
                    raw: false,
                    counter: None,
                    append: false,
                    read: true,
                    first: 0,
                },
            ],
            groups: DIVIDE_GROUPS,
        },
        Case {
            shader: "cs_escape",
            views: vec![
                View {
                    bytes: vec![0; size_of_val(points.as_slice())],
                    raw: false,
                    counter: None,
                    append: false,
                    read: false,
                    first: 0,
                },
                View {
                    bytes: words(&points),
                    raw: false,
                    counter: None,
                    append: false,
                    read: true,
                    first: 0,
                },
            ],
            groups: DIVIDE_GROUPS,
        },
    ]
}

/// `cs_divide`'s workgroups and their size.
const DIVIDE_GROUPS: u32 = 4;
const DIVIDE_GROUP_SIZE: u32 = 64;

/// `cs_divide`'s operands, an integer and a float divisor's bits for each invocation. Even ones
/// divide a multiple of the divisor, so the exact quotient is an integer and its fraction zero.
fn division_operands(next: &mut impl FnMut() -> u32, invocations: u32) -> Vec<u32> {
    const DIVISORS: u32 = 3000;
    const MULTIPLES: u32 = 64;
    (0..invocations)
        .flat_map(|i| {
            let divisor = 1 + next() % DIVISORS;
            if i % 2 == 0 {
                [divisor * (next() % MULTIPLES), (divisor as f32).to_bits()]
            } else {
                [
                    next(),
                    (divisor as f32 / (1 + next() % DIVISORS) as f32).to_bits(),
                ]
            }
        })
        .collect()
}

/// `cs_escape`'s operands, a point's x and y and an integer iteration scale as float bits for each
/// invocation. Even ones are inside the set's main cardioid, so they run to the full count, whose
/// quotient by the scale is an integer; odd ones are on the real axis right of the set, escaping
/// after a few steps. Points near the set's edge are left out: their counts follow every rounding
/// of the iteration, which Direct3D leaves free (one such point, from DXIL, was 4 steps apart).
fn escape_operands(next: &mut impl FnMut() -> u32, invocations: u32) -> Vec<u32> {
    const SCALES: u32 = 400;
    const STEPS: u32 = 1 << 16;
    let unit = |next: &mut dyn FnMut() -> u32| (next() % STEPS) as f32 / STEPS as f32;
    (0..invocations)
        .flat_map(|i| {
            let (x, y) = if i % 2 == 0 {
                (unit(next) * 0.5 - 0.5, unit(next) * 0.5 - 0.25)
            } else {
                (unit(next) * 1.5 + 0.5, 0.0)
            };
            let scale = 1 + next() % SCALES;
            [x.to_bits(), y.to_bits(), (scale as f32).to_bits()]
        })
        .collect()
}

/// The cases with each view moved to a seeded first element past seeded words (a multiple of four
/// for raw views, which Direct3D places at 16 bytes), so no view starts at a multiple of 256 bytes.
fn offset_cases(seed: u64) -> Vec<Case> {
    let mut next = rng(seed ^ 0x5eed);
    let mut cases = cases(seed);
    for v in cases.iter_mut().flat_map(|c| c.views.iter_mut()) {
        let first = 1 + next() % 300;
        v.first = if v.raw { first * 4 } else { first };
        if v.first % 64 == 0 {
            v.first += if v.raw { 4 } else { 1 };
        }
        let before: Vec<u32> = (0..v.first).map(|_| next()).collect();
        let mut bytes = words(&before);
        bytes.extend_from_slice(&v.bytes);
        v.bytes = bytes;
    }
    cases
}

/// What a run leaves: each view's bytes and each counter's count.
struct Ran {
    views: Vec<Vec<u8>>,
    counters: Vec<Option<u32>>,
}

fn direct3d(gpu: &Gpu, case: &Case) -> Ran {
    let shader = gpu.cs(&bytecode(&format!("{}.dxbc", case.shader))).unwrap();
    let mut buffers = Vec::new();
    let mut views = Vec::new();
    let mut read_views = Vec::new();
    for v in &case.views {
        let buffer = gpu
            .buffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: v.bytes.len() as u32,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: if v.read {
                        D3D11_BIND_SHADER_RESOURCE.0 as u32
                    } else {
                        D3D11_BIND_UNORDERED_ACCESS.0 as u32
                    },
                    MiscFlags: if v.raw {
                        D3D11_RESOURCE_MISC_BUFFER_ALLOW_RAW_VIEWS.0 as u32
                    } else {
                        D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0 as u32
                    },
                    StructureByteStride: if v.raw { 0 } else { 4 },
                    ..Default::default()
                },
                Some(&v.bytes),
            )
            .unwrap();
        if v.read {
            let desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
                Format: if v.raw {
                    DXGI_FORMAT_R32_TYPELESS
                } else {
                    DXGI_FORMAT_UNKNOWN
                },
                ViewDimension: D3D_SRV_DIMENSION_BUFFEREX,
                Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                    BufferEx: D3D11_BUFFEREX_SRV {
                        FirstElement: v.first,
                        NumElements: v.bytes.len() as u32 / 4 - v.first,
                        Flags: if v.raw {
                            D3D11_BUFFEREX_SRV_FLAG_RAW.0 as u32
                        } else {
                            0
                        },
                    },
                },
            };
            read_views.push(Some(gpu.srv(&buffer.clone().into(), Some(&desc)).unwrap()));
            views.push(None);
            buffers.push(buffer);
            continue;
        }
        let flags = if v.raw {
            D3D11_BUFFER_UAV_FLAG_RAW.0 as u32
        } else if v.append {
            D3D11_BUFFER_UAV_FLAG_APPEND.0 as u32
        } else if v.counter.is_some() {
            D3D11_BUFFER_UAV_FLAG_COUNTER.0 as u32
        } else {
            0
        };
        let desc = D3D11_UNORDERED_ACCESS_VIEW_DESC {
            Format: if v.raw {
                DXGI_FORMAT_R32_TYPELESS
            } else {
                DXGI_FORMAT_UNKNOWN
            },
            ViewDimension: D3D11_UAV_DIMENSION_BUFFER,
            Anonymous: D3D11_UNORDERED_ACCESS_VIEW_DESC_0 {
                Buffer: D3D11_BUFFER_UAV {
                    FirstElement: v.first,
                    NumElements: v.bytes.len() as u32 / 4 - v.first,
                    Flags: flags,
                },
            },
        };
        views.push(Some(gpu.uav(&buffer.clone().into(), Some(&desc)).unwrap()));
        buffers.push(buffer);
    }
    let initial: Vec<u32> = case
        .views
        .iter()
        .filter(|v| !v.read)
        .map(|v| v.counter.unwrap_or(!0))
        .collect();
    let bound: Vec<_> = views.iter().filter(|v| v.is_some()).cloned().collect();
    let counts = gpu
        .buffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: 4 * case.views.len() as u32,
                Usage: D3D11_USAGE_DEFAULT,
                ..Default::default()
            },
            None,
        )
        .unwrap();
    unsafe {
        let c = &gpu.context;
        c.CSSetShader(&shader, None);
        c.CSSetShaderResources(0, Some(&read_views));
        c.CSSetUnorderedAccessViews(
            0,
            bound.len() as u32,
            Some(bound.as_ptr()),
            Some(initial.as_ptr()),
        );
        c.Dispatch(case.groups, 1, 1);
        for (i, (v, view)) in case.views.iter().zip(&views).enumerate() {
            if v.counter.is_some() {
                c.CopyStructureCount(&counts, 4 * i as u32, view.as_ref().unwrap());
            }
        }
        let none: Vec<Option<ID3D11UnorderedAccessView>> = vec![None; bound.len()];
        let unread: Vec<Option<ID3D11ShaderResourceView>> = vec![None; read_views.len()];
        c.CSSetShaderResources(0, Some(&unread));
        c.CSSetUnorderedAccessViews(0, none.len() as u32, Some(none.as_ptr()), None);
    }
    let counts = gpu.read_buffer(&counts).unwrap();
    Ran {
        views: buffers
            .iter()
            .map(|b| gpu.read_buffer(b).unwrap())
            .collect(),
        counters: case
            .views
            .iter()
            .enumerate()
            .map(|(i, v)| {
                v.counter
                    .map(|_| u32::from_le_bytes(counts[4 * i..4 * i + 4].try_into().unwrap()))
            })
            .collect(),
    }
}

/// The same through the WebGPU back end, from fxc's DXBC or dxc's DXIL (whose views are numbered
/// from u0 without gaps, so a slot is the register on both routes).
fn ours<B: Backend>(b: &mut B, case: &Case, dxil: bool) -> Result<Ran, ConvertError> {
    let moved = case.views.iter().any(|v| v.first != 0);
    let mut key = VariantKey::default();
    if moved {
        key.flags |= wcontainers_gpu::VariantFlags::STORAGE_OFFSET;
    }
    let c = match dxil {
        true => convert(
            Source::Dxil(&bytecode(&format!("{}.dxil", case.shader))),
            &key,
            None,
        )?,
        false => convert(
            Source::Dxbc(&bytecode(&format!("{}.dxbc", case.shader))),
            &key,
            None,
        )?,
    };
    assert!(
        c.reflection
            .helpers
            .iter()
            .all(|h| matches!(h.value, HelperValue::StorageOffset { .. }))
    );
    // Each moved view binds from the largest multiple of the alignment and of the program's element
    // below its start; the rest goes into the helper block.
    let alignment = b.caps().limits.min_storage_offset_alignment as u64;
    let start = |r: &wcontainers_gpu::ReflectedBinding| {
        let first = 4 * case.views[view_of(case, r)].first as u64;
        if r.stride == 0 {
            return (first, 0);
        }
        let step = lcm(alignment, r.stride as u64);
        let base = first / step * step;
        (base, first - base)
    };
    let mut helper_bytes = vec![0u8; c.reflection.helper_size as usize];
    for h in &c.reflection.helpers {
        if let HelperValue::StorageOffset { set, binding } = h.value {
            let r = c
                .reflection
                .bindings
                .iter()
                .find(|r| r.set == set && r.binding == binding)
                .unwrap();
            let at = h.offset as usize;
            helper_bytes[at..at + 4].copy_from_slice(&(start(r).1 as u32).to_le_bytes());
        }
    }
    let usage = BufferUsage::STORAGE | BufferUsage::COPY_SRC;
    let buffers: Vec<_> = case
        .views
        .iter()
        .map(|v| buffer(b, &v.bytes, usage))
        .collect();
    let counters: Vec<_> = case
        .views
        .iter()
        .map(|v| v.counter.map(|n| buffer(b, &n.to_le_bytes(), usage)))
        .collect();
    let entries: Vec<_> = c
        .reflection
        .bindings
        .iter()
        .map(|r| {
            let slot = view_of(case, r);
            let range = match r.kind {
                SlotKind::Storage | SlotKind::Texture => {
                    let (base, _) = start(r);
                    BufferRange {
                        buffer: &buffers[slot],
                        offset: base,
                        size: case.views[slot].bytes.len() as u64 - base,
                    }
                }
                SlotKind::Counter => BufferRange {
                    buffer: counters[slot].as_ref().unwrap(),
                    offset: 0,
                    size: 4,
                },
                _ => panic!("{r:?}"),
            };
            BindingEntry {
                binding: r.binding,
                stages: StageSet::COMPUTE,
                resource: BindingResource::StorageBuffer {
                    range,
                    writable: r.kind != SlotKind::Texture,
                },
            }
        })
        .collect();
    let set = b.create_binding_set(&entries).unwrap();
    let helper_buffer =
        (!helper_bytes.is_empty()).then(|| buffer(b, &helper_bytes, BufferUsage::UNIFORM));
    let helper_set = helper_buffer.as_ref().map(|h| {
        b.create_binding_set(&[BindingEntry {
            binding: helper_binding(ShaderStage::Compute),
            stages: StageSet::COMPUTE,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: h,
                offset: 0,
                size: helper_bytes.len() as u64,
            }),
        }])
        .unwrap()
    });
    let shader = b.create_shader(&c.modules[0]).unwrap();
    let program = b.create_program(&Program::Compute(&shader)).unwrap();
    b.dispatch(&DispatchCall {
        program: &program,
        bindings: [Some(&set), None, None, helper_set.as_ref()],
        immediates: &[],
        groups: DispatchGroups::Direct([case.groups, 1, 1]),
    })
    .unwrap();
    let mut read = |buffer, size: u64| {
        let mut out = vec![0; size as usize];
        b.read_back_now(
            &ReadSource::Buffer {
                buffer,
                offset: 0,
                size,
            },
            &mut out,
        )
        .unwrap();
        out
    };
    let views = buffers
        .iter()
        .zip(&case.views)
        .map(|(buffer, v)| read(buffer, v.bytes.len() as u64))
        .collect();
    let counters = counters
        .iter()
        .map(|c| {
            c.as_ref()
                .map(|c| u32::from_le_bytes(read(c, 4).try_into().unwrap()))
        })
        .collect();
    assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
    Ok(Ran { views, counters })
}

/// DXIL builds refused today, with the reason.
const DXIL_REFUSED: [(&str, &str); 0] = [];

/// Makes what depends on the order invocations ran in comparable: the value the last exchange left
/// (word 648 of `cs_atomics`' raw view) is marked as the values the exchanges returned are, and the
/// appended values are sorted.
fn ordered(case: &Case, ran: &mut Ran) {
    if case.shader != "cs_atomics" {
        return;
    }
    let raw = &mut ran.views[0][4 * case.views[0].first as usize..];
    let last = u32::from_le_bytes(raw[2592..2596].try_into().unwrap()) as usize;
    raw[2592..2596].fill(0);
    if 1024 + last < 2048 {
        raw[4 * (1024 + last)..4 * (1024 + last) + 4].copy_from_slice(&1u32.to_le_bytes());
    }
    let n = ran.counters[3].unwrap_or(0).min(512) as usize;
    let appended_at = 4 * case.views[3].first as usize;
    let mut appended: Vec<u32> = ran.views[3][appended_at..appended_at + 4 * n]
        .chunks(4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
        .collect();
    appended.sort_unstable();
    ran.views[3][appended_at..appended_at + 4 * n].copy_from_slice(&words(&appended));
}

fn lcm(a: u64, b: u64) -> u64 {
    let gcd = |mut a: u64, mut b: u64| {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    };
    a / gcd(a, b) * b
}

#[test]
fn atomics_and_counters_match_direct3d() {
    compare_compute(cases, &mut webgpu());
}

/// The same shaders with every view starting at a seeded element past the start of its buffer,
/// read through `VariantFlags::STORAGE_OFFSET` (Direct3D's views at any first element).
#[test]
fn storage_views_at_any_first_element_match_direct3d() {
    compare_compute(offset_cases, &mut webgpu());
}

/// Float comparisons, minimum, maximum and saturate of NaNs, infinities and signed zeros
/// (`cs_compare`) give Direct3D's results on wgpu's Direct3D 12 and Vulkan paths, whose compilers
/// read an operator's NaN rule differently (SPIR-V names ordered and unordered comparisons).
#[test]
fn nan_rules_match_direct3d() {
    eprintln!("wgpu on direct3d 12:");
    let mut failures = compute_failures(compare_cases, &mut webgpu());
    eprintln!("wgpu on vulkan:");
    let vulkan = compute_failures(compare_cases, &mut webgpu_on(wgpu::Backends::VULKAN));
    failures.extend(vulkan.into_iter().map(|f| format!("vulkan {f}")));
    assert!(failures.is_empty(), "{failures:#?}");
}

/// `cs_compare`'s workgroups and their size.
const COMPARE_GROUPS: u32 = 4;
const COMPARE_GROUP_SIZE: u32 = 64;

/// `cs_compare`'s operand pairs: every pair of the special values first, then seeded pairs of
/// special values and seeded floats of any bits.
fn compare_cases(seed: u64) -> Vec<Case> {
    const SPECIAL: [u32; 11] = [
        f32::NAN.to_bits(),
        0xffc0_0001, // NaN, sign and payload set
        0x7f80_0001, // signalling NaN
        f32::INFINITY.to_bits(),
        f32::NEG_INFINITY.to_bits(),
        0f32.to_bits(),
        (-0f32).to_bits(),
        1f32.to_bits(),
        (-1f32).to_bits(),
        0.5f32.to_bits(),
        1, // the least subnormal
    ];
    let mut next = rng(seed);
    let n = SPECIAL.len();
    let pick = |next: &mut dyn FnMut() -> u32| match next() % 3 {
        0 => SPECIAL[next() as usize % n],
        _ => next(),
    };
    let operands: Vec<u32> = (0..(COMPARE_GROUPS * COMPARE_GROUP_SIZE) as usize)
        .flat_map(|i| {
            if i < n * n {
                [SPECIAL[i % n], SPECIAL[i / n]]
            } else {
                [pick(&mut next), pick(&mut next)]
            }
        })
        .collect();
    vec![Case {
        shader: "cs_compare",
        views: vec![
            View {
                bytes: vec![0; 2 * size_of_val(operands.as_slice())],
                raw: false,
                counter: None,
                append: false,
                read: false,
                first: 0,
            },
            View {
                bytes: words(&operands),
                raw: false,
                counter: None,
                append: false,
                read: true,
                first: 0,
            },
        ],
        groups: COMPARE_GROUPS,
    }]
}

fn compare_compute<B: Backend>(cases: fn(u64) -> Vec<Case>, backend: &mut B) {
    let failures = compute_failures(cases, backend);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The cases run by Direct3D 11 on the GPU and on WARP and by `backend` from both builds, at each
/// seed; the names of those where ours is neither real implementation's answer as a whole, every
/// word and counter of the GPU's or every word and counter of WARP's (they differ where Direct3D
/// leaves a result free, such as a division's last bit).
fn compute_failures<B: Backend>(cases: fn(u64) -> Vec<Case>, backend: &mut B) -> Vec<String> {
    let real = |driver| d3d11::create(driver, false, &[D3D_FEATURE_LEVEL_11_0]).unwrap();
    let gpu = real(wcontainers_graphics_test::Driver::Hardware);
    let warp = real(wcontainers_graphics_test::Driver::Warp);
    let seed: u64 = std::env::var("WCONTAINERS_COMPUTE_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_COMPUTE_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let mut failures = Vec::new();
    let mut compared = 0;
    for seed in seed..seed + seeds {
        for (case, dxil) in cases(seed)
            .into_iter()
            .flat_map(|c| [(c.clone(), false), (c, true)])
        {
            let build = if dxil { "dxil" } else { "dxbc" };
            let name = format!("seed {seed} {} {build}", case.shader);
            let mut reference = direct3d(&gpu, &case);
            let mut software = direct3d(&warp, &case);
            let mut made = match ours(backend, &case, dxil) {
                Ok(made) => made,
                Err(e) => {
                    let refused = DXIL_REFUSED.iter().find(|r| r.0 == case.shader);
                    let why = match &e {
                        ConvertError::Reader(w) => format!("spirv-reader {w}"),
                        e => format!("{e}"),
                    };
                    eprintln!("{name}: refused, {why}");
                    if !dxil || refused.is_none_or(|r| r.1 != why) {
                        failures.push(name);
                    }
                    continue;
                }
            };
            ordered(&case, &mut reference);
            ordered(&case, &mut software);
            ordered(&case, &mut made);
            compared += 1;
            let word = |w: &[u8]| u32::from_le_bytes(w.try_into().unwrap());
            // Words and counters of ours that differ from one real implementation's run.
            let apart = |real: &Ran| {
                let words: usize = (real.views.iter().zip(&made.views))
                    .map(|(a, o)| a.chunks(4).zip(o.chunks(4)).filter(|(x, y)| x != y).count())
                    .sum();
                words + usize::from(real.counters != made.counters)
            };
            let (from_gpu, from_warp) = (apart(&reference), apart(&software));
            let mut shown = 0;
            for (slot, ((a, s), o)) in reference
                .views
                .iter()
                .zip(&software.views)
                .zip(&made.views)
                .enumerate()
            {
                for (w, ((x, z), y)) in a.chunks(4).zip(s.chunks(4)).zip(o.chunks(4)).enumerate() {
                    if (y != x || y != z) && shown < 8 {
                        shown += 1;
                        eprintln!(
                            "  view {slot} word {w}: direct3d {:#010x}, warp {:#010x}, ours {:#010x}",
                            word(x),
                            word(z),
                            word(y)
                        );
                    }
                }
            }
            eprintln!(
                "{name}: counters direct3d {:?}, warp {:?}, ours {:?}; {from_gpu} words and counters apart from the gpu's, {from_warp} from warp's",
                reference.counters, software.counters, made.counters
            );
            if from_gpu > 0 && from_warp > 0 {
                failures.push(name);
            }
        }
    }
    eprintln!("{compared} comparisons, {} failed", failures.len());
    failures
}

/// One of `cs_images`' storage textures: slot, format, its Direct3D format, layers, and whether it
/// starts with seeded texels (the one read and written again).
type Image = (u32, Format, DXGI_FORMAT, u32, bool);

/// `cs_images`' storage textures, each of the kind its declaration reads.
const IMAGES: [Image; 4] = [
    (0, Format::Rgba8Unorm, DXGI_FORMAT_R8G8B8A8_UNORM, 1, false),
    (1, Format::R32Float, DXGI_FORMAT_R32_FLOAT, 1, true),
    (
        2,
        Format::Rgba16Uint,
        DXGI_FORMAT_R16G16B16A16_UINT,
        3,
        false,
    ),
    (3, Format::Rg32Sint, DXGI_FORMAT_R32G32_SINT, 1, false),
];

/// `cs_images`' storage textures each of another kind than its declaration reads, which Direct3D
/// reads and writes by their bits; the values written fit each format.
const OTHER_KIND_IMAGES: [Image; 4] = [
    (
        0,
        Format::Rgba32Uint,
        DXGI_FORMAT_R32G32B32A32_UINT,
        1,
        false,
    ),
    (1, Format::R32Uint, DXGI_FORMAT_R32_UINT, 1, true),
    (
        2,
        Format::Rgba16Sint,
        DXGI_FORMAT_R16G16B16A16_SINT,
        3,
        false,
    ),
    (3, Format::Rg32Uint, DXGI_FORMAT_R32G32_UINT, 1, false),
];
/// `cs_images` with u0 an `A8_UNORM` view: the colour stored there keeps its alpha alone.
const ALPHA_IMAGES: [Image; 4] = [
    (0, Format::A8Unorm, DXGI_FORMAT_A8_UNORM, 1, false),
    IMAGES[1],
    IMAGES[2],
    IMAGES[3],
];
const IMAGE_SIZE: u32 = 16;

/// The format the back ends hold a storage texture of `format` in, and its texels so held: `A8Unorm`
/// as rgba8unorm with the value in alpha (the WebGPU back end's packed.rs).
fn held(format: Format, texels: &[u8]) -> (Format, Vec<u8>) {
    match format {
        Format::A8Unorm => (
            Format::Rgba8Unorm,
            texels.iter().flat_map(|&a| [0, 0, 0, a]).collect(),
        ),
        _ => (format, texels.to_vec()),
    }
}

fn image_texels(seed: u64, format: Format, layers: u32, seeded: bool) -> Vec<u8> {
    let n = (IMAGE_SIZE * IMAGE_SIZE * layers * FORMAT_FACTS[format as usize].block.2) as usize;
    if !seeded {
        return vec![0; n];
    }
    let mut next = rng(seed);
    (0..n / 4)
        .flat_map(|_| ((next() % 2000) as f32 / 16.0 - 60.0).to_le_bytes())
        .collect()
}

/// Each texture's texels after real Direct3D 11 runs `cs_images`, layer after layer.
fn direct3d_images(gpu: &Gpu, seed: u64, images: &[Image]) -> Vec<Vec<u8>> {
    let shader = gpu.cs(&bytecode("cs_images.dxbc")).unwrap();
    let mut textures = Vec::new();
    let mut views = Vec::new();
    for &(_, format, dxgi, layers, seeded) in images {
        let mut desc = d3d11::tex2d(IMAGE_SIZE, IMAGE_SIZE, dxgi, D3D11_BIND_UNORDERED_ACCESS);
        desc.ArraySize = layers;
        let texels = image_texels(seed, format, layers, seeded);
        let row = IMAGE_SIZE * FORMAT_FACTS[format as usize].block.2;
        let data: Vec<_> = texels
            .chunks((row * IMAGE_SIZE) as usize)
            .map(|layer| d3d11::data(layer, row, row * IMAGE_SIZE))
            .collect();
        let texture = gpu.texture2d(&desc, &data).unwrap();
        views.push(Some(gpu.uav(&texture.clone().into(), None).unwrap()));
        textures.push((texture, format, layers));
    }
    unsafe {
        let c = &gpu.context;
        c.CSSetShader(&shader, None);
        c.CSSetUnorderedAccessViews(0, views.len() as u32, Some(views.as_ptr()), None);
        c.Dispatch(IMAGE_SIZE / 8, IMAGE_SIZE / 8, 1);
        let none: Vec<Option<ID3D11UnorderedAccessView>> = vec![None; views.len()];
        c.CSSetUnorderedAccessViews(0, none.len() as u32, Some(none.as_ptr()), None);
    }
    textures
        .iter()
        .map(|(texture, format, layers)| {
            let row = (IMAGE_SIZE * FORMAT_FACTS[*format as usize].block.2) as usize;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            unsafe { texture.GetDesc(&mut desc) };
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            let staging = gpu.texture2d(&desc, &[]).unwrap();
            let mut out = Vec::new();
            unsafe {
                gpu.context.CopyResource(&staging, texture);
                for layer in 0..*layers {
                    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                    gpu.context
                        .Map(&staging, layer, D3D11_MAP_READ, 0, Some(&mut mapped))
                        .unwrap();
                    for y in 0..IMAGE_SIZE as usize {
                        let start = mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize);
                        out.extend_from_slice(std::slice::from_raw_parts(start, row));
                    }
                    gpu.context.Unmap(&staging, layer);
                }
            }
            out
        })
        .collect()
}

/// The same through the WebGPU back end, each view's format given in the variant, from fxc's
/// bytecode (DXBC) or dxc's (DXIL).
fn ours_images<B: Backend>(b: &mut B, seed: u64, dxil: bool, images: &[Image]) -> Vec<Vec<u8>> {
    let key = VariantKey {
        storage_formats: images.iter().map(|&(slot, f, ..)| (slot, f)).collect(),
        ..Default::default()
    };
    let c = if dxil {
        convert(Source::Dxil(&bytecode("cs_images.dxil")), &key, None)
    } else {
        convert(Source::Dxbc(&bytecode("cs_images.dxbc")), &key, None)
    }
    .unwrap();
    assert!(c.reflection.helpers.is_empty());
    let mut textures = Vec::new();
    for &(_, viewed, _, layers, seeded) in images {
        let (format, texels) = held(viewed, &image_texels(seed, viewed, layers, seeded));
        let texture = b
            .create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width: IMAGE_SIZE,
                height: IMAGE_SIZE,
                depth_or_layers: layers,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::STORAGE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
                cube: false,
                reinterpret: false,
            })
            .unwrap();
        let view = b
            .create_view(
                &texture,
                &TextureViewDesc {
                    format,
                    dimension: if layers > 1 {
                        ViewDimension::D2Array
                    } else {
                        ViewDimension::D2
                    },
                    base_mip: 0,
                    mip_count: 1,
                    base_layer: 0,
                    layer_count: layers,
                    stencil: false,
                },
            )
            .unwrap();
        let row = IMAGE_SIZE * FORMAT_FACTS[format as usize].block.2;
        b.write_texture(
            &TextureRegion {
                texture: &texture,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width: IMAGE_SIZE,
                height: IMAGE_SIZE,
                depth_or_layers: layers,
            },
            &texels,
            row,
            IMAGE_SIZE,
            WriteMode::Ordered,
        );
        textures.push((texture, view, format, layers));
    }
    let entries: Vec<_> = c
        .reflection
        .bindings
        .iter()
        .map(|r| {
            assert_eq!(r.kind, SlotKind::Storage, "{r:?}");
            BindingEntry {
                binding: r.binding,
                stages: StageSet::COMPUTE,
                resource: BindingResource::StorageTexture {
                    view: &textures[r.register as usize].1,
                    writable: true,
                },
            }
        })
        .collect();
    let set = b.create_binding_set(&entries).unwrap();
    let shader = b.create_shader(&c.modules[0]).unwrap();
    let program = b.create_program(&Program::Compute(&shader)).unwrap();
    b.dispatch(&DispatchCall {
        program: &program,
        bindings: [Some(&set), None, None, None],
        immediates: &[],
        groups: DispatchGroups::Direct([IMAGE_SIZE / 8, IMAGE_SIZE / 8, 1]),
    })
    .unwrap();
    let out = textures
        .iter()
        .map(|(texture, _, format, layers)| {
            let mut all = Vec::new();
            for layer in 0..*layers {
                let mut out = vec![
                    0;
                    (IMAGE_SIZE * IMAGE_SIZE * FORMAT_FACTS[*format as usize].block.2)
                        as usize
                ];
                b.read_back_now(
                    &ReadSource::Texture {
                        region: TextureRegion {
                            texture,
                            mip: 0,
                            layer,
                            origin: Origin::default(),
                            stencil: false,
                        },
                        extent: Extent {
                            width: IMAGE_SIZE,
                            height: IMAGE_SIZE,
                            depth_or_layers: 1,
                        },
                    },
                    &mut out,
                )
                .unwrap();
                all.extend(out);
            }
            all
        })
        .collect();
    assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
    out
}

#[test]
fn storage_textures_match_direct3d() {
    storage_textures_alike(&IMAGES);
}

#[test]
fn alpha_storage_textures_store_their_alpha_as_direct3d_does() {
    storage_textures_alike(&ALPHA_IMAGES);
}

#[test]
fn storage_textures_of_another_kind_read_and_write_their_bits_as_direct3d_does() {
    storage_textures_alike(&OTHER_KIND_IMAGES);
}

/// `cs_images` on `images` through real Direct3D 11 and through ours, every texel compared.
fn storage_textures_alike(images: &[Image]) {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let seed: u64 = std::env::var("WCONTAINERS_COMPUTE_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_COMPUTE_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let mut failures = Vec::new();
    for seed in seed..seed + seeds {
        let reference = direct3d_images(&gpu, seed, images);
        for (dxil, source) in [(false, "dxbc"), (true, "dxil")] {
            let made = ours_images(&mut backend, seed, dxil, images);
            for ((a, o), &(slot, format, ..)) in reference.iter().zip(&made).zip(images) {
                // Compared as held: Direct3D's texels in the back ends' form.
                let (_, a) = held(format, a);
                let apart = a.chunks(4).zip(o.chunks(4)).filter(|(x, y)| x != y).count();
                eprintln!("seed {seed} {source} u{slot} {format:?}: {apart} words apart");
                if apart > 0 {
                    failures.push(format!("seed {seed} {source} u{slot}"));
                }
            }
        }
    }
    eprintln!("{} seeds, {} failed", seeds, failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// `cs_texels`' typed buffer views: kind, register, format, first element and length, and the
/// bytes of the whole buffer the view lies in.
struct TexelView {
    kind: SlotKind,
    register: u32,
    format: Format,
    first: u32,
    count: u32,
    bytes: Vec<u8>,
}

/// Invocations of `cs_texels` (two groups), and so the elements each view is read and written at.
const TEXEL_INVOCATIONS: u32 = 128;

/// Formats loaded through `Buffer<float4>` and `Buffer<float2>`.
const LOADED_FLOATS: [Format; 22] = [
    Format::R8Unorm,
    Format::R8Snorm,
    Format::R16Unorm,
    Format::R16Snorm,
    Format::R16Float,
    Format::Rg8Unorm,
    Format::Rg8Snorm,
    Format::R32Float,
    Format::Rg16Unorm,
    Format::Rg16Snorm,
    Format::Rg16Float,
    Format::Rgba8Unorm,
    Format::Rgba8Snorm,
    Format::Bgra8Unorm,
    Format::Rgb10a2Unorm,
    Format::Rg11b10Float,
    Format::Rg32Float,
    Format::Rgba16Unorm,
    Format::Rgba16Snorm,
    Format::Rgba16Float,
    Format::Rgb32Float,
    Format::Rgba32Float,
];
const LOADED_UINTS: [Format; 11] = [
    Format::R8Uint,
    Format::R16Uint,
    Format::Rg8Uint,
    Format::R32Uint,
    Format::Rg16Uint,
    Format::Rgba8Uint,
    Format::Rgb10a2Uint,
    Format::Rg32Uint,
    Format::Rgba16Uint,
    Format::Rgb32Uint,
    Format::Rgba32Uint,
];
const LOADED_SINTS: [Format; 10] = [
    Format::R8Sint,
    Format::R16Sint,
    Format::Rg8Sint,
    Format::R32Sint,
    Format::Rg16Sint,
    Format::Rgba8Sint,
    Format::Rg32Sint,
    Format::Rgba16Sint,
    Format::Rgb32Sint,
    Format::Rgba32Sint,
];

/// `cs_image_array`'s elements and the side of its square of invocations, one per texel.
const ARRAY_ELEMENTS: usize = 4;
const ARRAY_SIDE: u32 = 8;

/// What `cs_image_array` leaves in each element, from the seeded texels it starts with, by
/// Direct3D's rules for its loads and stores (each texel's result is determined).
fn image_array_expected(start: &[Vec<u32>]) -> Vec<Vec<u32>> {
    let elements = ARRAY_ELEMENTS as u32;
    let mut out = start.to_vec();
    let at = |x: u32, y: u32| (y * ARRAY_SIDE + x) as usize;
    for y in 0..ARRAY_SIDE {
        for x in 0..ARRAY_SIDE {
            let before = start[((x + 2 * y) % elements) as usize][at(x, y)];
            let i = (x + y) % elements;
            out[i as usize][at(x, y)] = before + x * 16 + y + 1;
            out[(elements - 1 - i) as usize][at(x, y)] = before ^ 30001;
        }
    }
    out
}

/// An array of storage textures (`cs_image_array`, dxc's and fxc's shader model 5.1) through the
/// WebGPU back end: an element read or stored into after a pick at run time is the one named,
/// against the results Direct3D's rules give (shader model 5.1 is not Direct3D 11's).
/// Seeded texels: `WCONTAINERS_COMPUTE_SEED` and `WCONTAINERS_COMPUTE_CASES`.
#[test]
fn storage_texture_arrays_use_the_elements_picked() {
    let mut b = webgpu();
    let seed: u64 = std::env::var("WCONTAINERS_COMPUTE_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_COMPUTE_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let format = Format::R32Uint;
    let key = VariantKey {
        storage_formats: (0..ARRAY_ELEMENTS as u32).map(|s| (s, format)).collect(),
        ..Default::default()
    };
    let dxil = bytecode("cs_image_array.dxil");
    let dxbc = bytecode("cs_image_array.dxbc");
    let (width, height) = (ARRAY_SIDE, ARRAY_SIDE);
    let texel = FORMAT_FACTS[format as usize].block.2;
    let mut failures = Vec::new();
    for (route, source) in [("dxil", Source::Dxil(&dxil)), ("dxbc", Source::Dxbc(&dxbc))] {
        let c = convert(source, &key, None).unwrap();
        let shader = b.create_shader(&c.modules[0]).unwrap();
        let program = b.create_program(&Program::Compute(&shader)).unwrap();
        for seed in seed..seed + seeds {
            let mut next = rng(seed);
            let start: Vec<Vec<u32>> = (0..ARRAY_ELEMENTS)
                .map(|_| (0..width * height).map(|_| next() >> 4).collect())
                .collect();
            let mut textures = Vec::new();
            for texels in &start {
                let texture = b
                    .create_texture(&TextureDesc {
                        dimension: TextureDimension::D2,
                        format,
                        width,
                        height,
                        depth_or_layers: 1,
                        mip_levels: 1,
                        samples: 1,
                        usage: TextureUsage::STORAGE
                            | TextureUsage::COPY_SRC
                            | TextureUsage::COPY_DST,
                        cube: false,
                        reinterpret: false,
                    })
                    .unwrap();
                let view = b
                    .create_view(
                        &texture,
                        &TextureViewDesc {
                            format,
                            dimension: ViewDimension::D2,
                            base_mip: 0,
                            mip_count: 1,
                            base_layer: 0,
                            layer_count: 1,
                            stencil: false,
                        },
                    )
                    .unwrap();
                b.write_texture(
                    &TextureRegion {
                        texture: &texture,
                        mip: 0,
                        layer: 0,
                        origin: Origin::default(),
                        stencil: false,
                    },
                    Extent {
                        width,
                        height,
                        depth_or_layers: 1,
                    },
                    &words(texels),
                    width * texel,
                    height,
                    WriteMode::Ordered,
                );
                textures.push((texture, view));
            }
            let entries: Vec<_> = c
                .reflection
                .bindings
                .iter()
                .map(|r| {
                    assert_eq!(r.kind, SlotKind::Storage, "{r:?}");
                    BindingEntry {
                        binding: r.binding,
                        stages: StageSet::COMPUTE,
                        resource: BindingResource::StorageTexture {
                            view: &textures[r.register as usize].1,
                            writable: true,
                        },
                    }
                })
                .collect();
            assert_eq!(entries.len(), ARRAY_ELEMENTS);
            let set = b.create_binding_set(&entries).unwrap();
            b.dispatch(&DispatchCall {
                program: &program,
                bindings: [Some(&set), None, None, None],
                immediates: &[],
                groups: DispatchGroups::Direct([1, 1, 1]),
            })
            .unwrap();
            let expected = image_array_expected(&start);
            let mut apart = 0;
            for (k, (texture, _)) in textures.iter().enumerate() {
                let mut out = vec![0; (width * height * texel) as usize];
                b.read_back_now(
                    &ReadSource::Texture {
                        region: TextureRegion {
                            texture,
                            mip: 0,
                            layer: 0,
                            origin: Origin::default(),
                            stencil: false,
                        },
                        extent: Extent {
                            width,
                            height,
                            depth_or_layers: 1,
                        },
                    },
                    &mut out,
                )
                .unwrap();
                apart += out
                    .chunks(texel as usize)
                    .zip(&expected[k])
                    .filter(|(o, e)| u32::from_le_bytes((*o).try_into().unwrap()) != **e)
                    .count();
            }
            assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
            eprintln!("seed {seed} cs_image_array ({route}): {apart} texels apart");
            if apart > 0 {
                failures.push((route, seed));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

/// `cs_buffer_array`'s arrays as declared (kind, first register, elements) and its invocations,
/// one per element of each view.
const BUFFER_ARRAYS: [(SlotKind, u32, u32); 5] = [
    (SlotKind::Uniform, 0, 2),
    (SlotKind::Texture, 0, 3),
    (SlotKind::Texture, 3, 2),
    (SlotKind::Storage, 0, 2),
    (SlotKind::Storage, 2, 2),
];
const BUFFER_ARRAY_INVOCATIONS: u32 = 8;

/// `cs_typed_array`'s arrays as declared (kind, first register, elements), the unbounded ones
/// sized as the test's ranges give them.
const TYPED_ARRAYS: [(SlotKind, u32, u32); 4] = [
    (SlotKind::Uniform, 0, 1),
    (SlotKind::Texture, 0, 3),
    (SlotKind::Texture, 3, 2),
    (SlotKind::Storage, 0, 3),
];

/// What `cs_typed_array` leaves in its storage views (u0 to u2), from the seeded constant (one
/// buffer), sources (t0 to t4) and storage words, by Direct3D's rules.
fn typed_array_expected(
    picks: &[[u32; 4]],
    sources: &[Vec<u32>],
    start: &[Vec<u32>],
) -> Vec<Vec<u32>> {
    let mut out = start.to_vec();
    let (rest, last) = (3, 2);
    let p = picks[0];
    for i in 0..BUFFER_ARRAY_INVOCATIONS {
        let at = i as usize;
        let summed = sources[((p[1] + i) % 3) as usize][at]
            .wrapping_add(sources[rest + ((p[2] + i) % 2) as usize][at]);
        out[((p[0] + i) % 2) as usize][at] = summed;
        out[last][at] = sources[last][at] ^ sources[rest + ((p[3] + i) % 2) as usize][at];
    }
    out
}

/// What `cs_buffer_array` leaves in its storage views (u0 to u3), from the seeded constants (four
/// words per constant buffer), sources (t0 to t4) and storage words, by Direct3D's rules.
fn buffer_array_expected(
    picks: &[[u32; 4]],
    sources: &[Vec<u32>],
    start: &[Vec<u32>],
) -> Vec<Vec<u32>> {
    let mut out = start.to_vec();
    let (targets, rest, more) = (0, 3, 2);
    for i in 0..BUFFER_ARRAY_INVOCATIONS {
        let p = picks[(i % 2) as usize];
        let at = i as usize;
        let summed = sources[((p[1] + i) % 3) as usize][at]
            .wrapping_add(sources[rest + ((p[2] + i) % 2) as usize][at]);
        out[targets + ((p[0] + i) % 2) as usize][at] = summed;
        out[more + ((p[3] + i) % 2) as usize][at] =
            sources[2][at] ^ sources[rest + (i % 2) as usize][at];
    }
    out
}

/// Arrays of constant buffers and typed buffers (`cs_buffer_array`, fxc's shader model 5.1 and dxc's
/// 6.0), two declared without a length and sized by the root signature's ranges, through the WebGPU back end:
/// every element read or written after a pick at run time is the one named, against the results
/// Direct3D's rules give. Seeded words: `WCONTAINERS_COMPUTE_SEED` and `WCONTAINERS_COMPUTE_CASES`.
#[test]
fn buffer_arrays_use_the_elements_picked() {
    buffer_arrays_alike(
        "cs_buffer_array",
        &BUFFER_ARRAYS,
        buffer_array_expected,
        &[false, true],
    );
}

/// Arrays of typed buffers alone (`cs_typed_array`, fxc's shader model 5.1 and dxc's 6.0), one of
/// each kind declared without a length and sized by the root signature's ranges, as
/// `buffer_arrays_use_the_elements_picked`.
#[test]
fn typed_buffer_arrays_use_the_elements_picked_on_both_routes() {
    buffer_arrays_alike(
        "cs_typed_array",
        &TYPED_ARRAYS,
        typed_array_expected,
        &[false, true],
    );
}

/// `cs_structured_array`'s arrays as declared (kind, first register, elements), the unbounded one
/// sized as the test's ranges give it.
const STRUCTURED_ARRAYS: [(SlotKind, u32, u32); 4] = [
    (SlotKind::Uniform, 0, 1),
    (SlotKind::Texture, 0, 3),
    (SlotKind::Texture, 3, 2),
    (SlotKind::Storage, 0, 3),
];

/// What `cs_structured_array` leaves in its storage views (u0 to u2), from the seeded constant (one
/// buffer), sources (t0 to t4) and storage words, by Direct3D's rules; every view holds a word per
/// invocation, so the length it takes is that many four-byte elements.
fn structured_array_expected(
    picks: &[[u32; 4]],
    sources: &[Vec<u32>],
    start: &[Vec<u32>],
) -> Vec<Vec<u32>> {
    let mut out = start.to_vec();
    let rest = 3;
    let p = picks[0];
    let length = BUFFER_ARRAY_INVOCATIONS * size_of::<u32>() as u32;
    for i in 0..BUFFER_ARRAY_INVOCATIONS {
        let at = i as usize;
        out[((p[0] + i) % 2) as usize][at] =
            sources[((p[1] + i) % 3) as usize][at].wrapping_add(length);
        let added = ((p[3] + i) % 3) as usize;
        let before = out[added][at];
        out[added][at] = before.wrapping_add(sources[rest + (i % 2) as usize][at]);
        out[2][at] ^= before << 1;
    }
    out
}

/// Arrays of structured buffers (`cs_structured_array`, fxc's shader model 5.1 and dxc's 6.0), one
/// declared without a length and sized by the root signature's ranges, read, stored to, added to
/// atomically and measured through elements picked at run time, as
/// `buffer_arrays_use_the_elements_picked`.
#[test]
fn structured_buffer_arrays_use_the_elements_picked_on_both_routes() {
    buffer_arrays_alike(
        "cs_structured_array",
        &STRUCTURED_ARRAYS,
        structured_array_expected,
        &[false, true],
    );
}

/// What an array test's shader leaves in its storage views, from its seeded constants (four words
/// per constant buffer), sources and storage words.
type ArrayResults = fn(&[[u32; 4]], &[Vec<u32>], &[Vec<u32>]) -> Vec<Vec<u32>>;

/// `shader`, declaring `arrays`, converted from fxc's bytecode and, where `dxil` holds true, dxc's,
/// with every element bound at its register, against `expected`.
fn buffer_arrays_alike(
    shader: &str,
    arrays: &[(SlotKind, u32, u32)],
    expected: ArrayResults,
    dxil: &[bool],
) {
    let mut b = webgpu();
    let seed: u64 = std::env::var("WCONTAINERS_COMPUTE_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_COMPUTE_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let registers = |kind| -> Vec<u32> {
        arrays
            .iter()
            .filter(|a| a.0 == kind)
            .flat_map(|a| a.1..a.1 + a.2)
            .collect()
    };
    let formats = |kind| {
        registers(kind)
            .into_iter()
            .map(|r| (r, Format::R32Uint))
            .collect()
    };
    let ranges = [SlotKind::Texture, SlotKind::Storage]
        .into_iter()
        .map(|kind| (kind, 0, 0..registers(kind).len() as u32))
        .collect();
    let key = VariantKey {
        texel_buffers: TexelBufferRoute::Storage,
        texel_formats: formats(SlotKind::Texture),
        storage_formats: formats(SlotKind::Storage),
        register_ranges: ranges,
        ..Default::default()
    };
    let word = size_of::<u32>();
    let pick_words = size_of::<[u32; 4]>() / word;
    let mut failures = Vec::new();
    for &dxil in dxil {
        let (build, code) = match dxil {
            true => ("dxil", bytecode(&format!("{shader}.dxil"))),
            false => ("dxbc", bytecode(&format!("{shader}.dxbc"))),
        };
        let source = match dxil {
            true => Source::Dxil(&code),
            false => Source::Dxbc(&code),
        };
        let c = convert(source, &key, None).unwrap();
        let module = b.create_shader(&c.modules[0]).unwrap();
        let program = b.create_program(&Program::Compute(&module)).unwrap();
        for seed in seed..seed + seeds {
            let mut next = rng(seed);
            let picks: Vec<[u32; 4]> = registers(SlotKind::Uniform)
                .iter()
                .map(|_| [(); 4].map(|_| next() >> 4))
                .collect();
            let mut seeded = |kind| -> Vec<Vec<u32>> {
                registers(kind)
                    .iter()
                    .map(|_| (0..BUFFER_ARRAY_INVOCATIONS).map(|_| next() >> 4).collect())
                    .collect()
            };
            let sources = seeded(SlotKind::Texture);
            let start = seeded(SlotKind::Storage);
            let usage = BufferUsage::STORAGE | BufferUsage::COPY_SRC;
            let constants: Vec<_> = picks
                .iter()
                .map(|p| buffer(&mut b, &words(p), BufferUsage::UNIFORM))
                .collect();
            let read_only: Vec<_> = sources
                .iter()
                .map(|w| buffer(&mut b, &words(w), usage))
                .collect();
            let written: Vec<_> = start
                .iter()
                .map(|w| buffer(&mut b, &words(w), usage))
                .collect();
            let bytes = u64::from(BUFFER_ARRAY_INVOCATIONS) * word as u64;
            let entries: Vec<_> = c
                .reflection
                .bindings
                .iter()
                .map(|r| {
                    let at = r.register as usize;
                    let resource = match r.kind {
                        SlotKind::Uniform => BindingResource::UniformBuffer(BufferRange {
                            buffer: &constants[at],
                            offset: 0,
                            size: (pick_words * word) as u64,
                        }),
                        SlotKind::Texture | SlotKind::Storage => {
                            let storage = r.kind == SlotKind::Storage;
                            let buffer = if storage {
                                &written[at]
                            } else {
                                &read_only[at]
                            };
                            BindingResource::StorageBuffer {
                                range: BufferRange {
                                    buffer,
                                    offset: 0,
                                    size: bytes,
                                },
                                writable: storage,
                            }
                        }
                        other => panic!("{other:?}"),
                    };
                    BindingEntry {
                        binding: r.binding,
                        stages: StageSet::COMPUTE,
                        resource,
                    }
                })
                .collect();
            let set = b.create_binding_set(&entries).unwrap();
            let mut helpers = vec![0u8; c.reflection.helper_size as usize];
            for h in &c.reflection.helpers {
                let HelperValue::TexelBufferView { .. } = h.value else {
                    panic!("{h:?}");
                };
                // Every view starts at its buffer's first word and holds a word per invocation.
                let at = h.offset as usize + word;
                helpers[at..at + word].copy_from_slice(&BUFFER_ARRAY_INVOCATIONS.to_le_bytes());
            }
            let helper_buffer = buffer(&mut b, &helpers, BufferUsage::UNIFORM);
            let helper_set = b
                .create_binding_set(&[BindingEntry {
                    binding: helper_binding(ShaderStage::Compute),
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &helper_buffer,
                        offset: 0,
                        size: helpers.len() as u64,
                    }),
                }])
                .unwrap();
            let mut bindings = [Some(&set), None, None, None];
            bindings[HELPER_SET as usize] = Some(&helper_set);
            b.dispatch(&DispatchCall {
                program: &program,
                bindings,
                immediates: &[],
                groups: DispatchGroups::Direct([1, 1, 1]),
            })
            .unwrap();
            let expected = expected(&picks, &sources, &start);
            let mut apart = 0;
            for (k, buffer) in written.iter().enumerate() {
                let mut out = vec![0; bytes as usize];
                b.read_back_now(
                    &ReadSource::Buffer {
                        buffer,
                        offset: 0,
                        size: bytes,
                    },
                    &mut out,
                )
                .unwrap();
                apart += out
                    .chunks(word)
                    .zip(&expected[k])
                    .filter(|(o, e)| u32::from_le_bytes((*o).try_into().unwrap()) != **e)
                    .count();
            }
            assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
            eprintln!("seed {seed} {build} {shader}: {apart} words apart");
            if apart > 0 {
                failures.push(format!("{build} seed {seed}"));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

/// Whether the device has `format` for a typed buffer view (`uav`: one written by the program).
fn texel_supported(gpu: &Gpu, format: Format, uav: bool) -> bool {
    let need = if uav {
        D3D11_FORMAT_SUPPORT_TYPED_UNORDERED_ACCESS_VIEW.0 as u32
    } else {
        D3D11_FORMAT_SUPPORT_BUFFER.0 as u32
    };
    unsafe { gpu.device.CheckFormatSupport(dxgi(format)) }.is_ok_and(|s| s & need == need)
}

/// One seeded case: each view's format picked from the formats the device has for it, its first
/// element and length (some invocations read and write past it), and seeded bytes everywhere.
fn texel_views(gpu: &Gpu, seed: u64, kinds: Kinds) -> Vec<TexelView> {
    let mut next = rng(seed);
    let pick = |from: &[Format], uav: bool, next: &mut dyn FnMut() -> u32| {
        let have: Vec<Format> = from
            .iter()
            .copied()
            .filter(|&f| texel_supported(gpu, f, uav))
            .collect();
        have[next() as usize % have.len()]
    };
    // Each declaration's own kind, or the formats of the other two kinds.
    let floats = [LOADED_UINTS.as_slice(), &LOADED_SINTS].concat();
    let uints = [LOADED_FLOATS.as_slice(), &LOADED_SINTS].concat();
    let sints = [LOADED_FLOATS.as_slice(), &LOADED_UINTS].concat();
    let read = match kinds {
        Kinds::Read(f) => {
            assert!(texel_supported(gpu, f, false), "{f:?} typed buffers");
            vec![f]
        }
        _ => Vec::new(),
    };
    let (floats, uints, sints, counts) = match kinds {
        Kinds::Declared => (
            LOADED_FLOATS.as_slice(),
            LOADED_UINTS.as_slice(),
            LOADED_SINTS.as_slice(),
            Format::R32Uint,
        ),
        Kinds::Other => (floats.as_slice(), &uints[..], &sints[..], Format::R32Sint),
        Kinds::Read(_) => (
            read.as_slice(),
            LOADED_UINTS.as_slice(),
            LOADED_SINTS.as_slice(),
            Format::R32Uint,
        ),
    };
    let stored = match kinds {
        Kinds::Read(_) => LOADED_FLOATS.as_slice(),
        _ => floats,
    };
    let slots = [
        (SlotKind::Texture, 0, pick(floats, false, &mut next)),
        (SlotKind::Texture, 1, pick(uints, false, &mut next)),
        (SlotKind::Texture, 2, pick(sints, false, &mut next)),
        (SlotKind::Texture, 3, pick(floats, false, &mut next)),
        (SlotKind::Storage, 0, pick(stored, true, &mut next)),
        (SlotKind::Storage, 1, counts),
    ];
    slots
        .into_iter()
        .map(|(kind, register, format)| {
            let size = FORMAT_FACTS[format as usize].block.2;
            let first = next() % 8;
            let count = 100 + next() % 41;
            let len = ((first + count + 3) * size).next_multiple_of(16);
            let bytes = (0..len).map(|_| next() as u8).collect();
            TexelView {
                kind,
                register,
                format,
                first,
                count,
                bytes,
            }
        })
        .collect()
}

/// The bytes of every view's buffer after the dispatch, then the results.
fn direct3d_texels(gpu: &Gpu, views: &[TexelView]) -> Vec<Vec<u8>> {
    let shader = gpu.cs(&bytecode("cs_texels.dxbc")).unwrap();
    let mut buffers = Vec::new();
    let mut srvs = Vec::new();
    let mut uavs = Vec::new();
    for v in views {
        let storage = v.kind == SlotKind::Storage;
        let buffer = gpu
            .buffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: v.bytes.len() as u32,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: if storage {
                        D3D11_BIND_UNORDERED_ACCESS.0 as u32
                    } else {
                        D3D11_BIND_SHADER_RESOURCE.0 as u32
                    },
                    ..Default::default()
                },
                Some(&v.bytes),
            )
            .unwrap();
        let resource: ID3D11Resource = buffer.clone().into();
        if storage {
            let desc = D3D11_UNORDERED_ACCESS_VIEW_DESC {
                Format: dxgi(v.format),
                ViewDimension: D3D11_UAV_DIMENSION_BUFFER,
                Anonymous: D3D11_UNORDERED_ACCESS_VIEW_DESC_0 {
                    Buffer: D3D11_BUFFER_UAV {
                        FirstElement: v.first,
                        NumElements: v.count,
                        Flags: 0,
                    },
                },
            };
            uavs.push(Some(gpu.uav(&resource, Some(&desc)).unwrap()));
        } else {
            let desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
                Format: dxgi(v.format),
                ViewDimension: D3D_SRV_DIMENSION_BUFFER,
                Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                    Buffer: D3D11_BUFFER_SRV {
                        Anonymous1: D3D11_BUFFER_SRV_0 {
                            FirstElement: v.first,
                        },
                        Anonymous2: D3D11_BUFFER_SRV_1 {
                            NumElements: v.count,
                        },
                    },
                },
            };
            srvs.push(Some(gpu.srv(&resource, Some(&desc)).unwrap()));
        }
        buffers.push(buffer);
    }
    let results_len = TEXEL_INVOCATIONS * 6 * 16;
    let results = gpu
        .buffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: results_len,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_UNORDERED_ACCESS.0 as u32,
                MiscFlags: D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0 as u32,
                StructureByteStride: 16,
                ..Default::default()
            },
            Some(&vec![0; results_len as usize]),
        )
        .unwrap();
    uavs.push(Some(gpu.uav(&results.clone().into(), None).unwrap()));
    buffers.push(results);
    unsafe {
        let c = &gpu.context;
        c.CSSetShader(&shader, None);
        c.CSSetShaderResources(0, Some(&srvs));
        c.CSSetUnorderedAccessViews(0, uavs.len() as u32, Some(uavs.as_ptr()), None);
        c.Dispatch(TEXEL_INVOCATIONS / 64, 1, 1);
        let none: Vec<Option<ID3D11UnorderedAccessView>> = vec![None; uavs.len()];
        c.CSSetUnorderedAccessViews(0, none.len() as u32, Some(none.as_ptr()), None);
        let none: Vec<Option<ID3D11ShaderResourceView>> = vec![None; srvs.len()];
        c.CSSetShaderResources(0, Some(&none));
    }
    let mut out: Vec<Vec<u8>> = Vec::new();
    for (i, b) in buffers.iter().enumerate() {
        let storage = i >= views.len() || views[i].kind == SlotKind::Storage;
        if storage {
            out.push(gpu.read_buffer(b).unwrap());
        }
    }
    out
}

/// The same through the WebGPU back end, from the fxc or the dxc build: each typed buffer bound as
/// storage from its view's first byte rounded down to the storage offset alignment, its place and
/// length in the helpers. The views' registers start at 0 and have no gaps, so Direct3D 12's slots
/// (the DXIL route's) are the registers.
/// Where `cs_texels` comes from: fxc's or dxc's build through `convert`, or `typed_texels.wgsl`
/// through `typed::buffers` and `rewrite`.
#[derive(Clone, Copy)]
enum TexelBuild {
    Dxbc,
    Dxil,
    Module,
}

fn ours_texels<B: Backend>(b: &mut B, views: &[TexelView], build: TexelBuild) -> Vec<Vec<u8>> {
    let format_of = |kind| {
        views
            .iter()
            .filter(|v| v.kind == kind)
            .map(|v| (v.register, v.format))
            .collect()
    };
    let key = VariantKey {
        texel_buffers: TexelBufferRoute::Storage,
        texel_formats: format_of(SlotKind::Texture),
        storage_formats: format_of(SlotKind::Storage),
        ..Default::default()
    };
    let c = match build {
        TexelBuild::Dxil => convert(Source::Dxil(&bytecode("cs_texels.dxil")), &key, None).unwrap(),
        TexelBuild::Dxbc => convert(Source::Dxbc(&bytecode("cs_texels.dxbc")), &key, None).unwrap(),
        TexelBuild::Module => forms::typed_texels(&key, &forms::TYPED_TEXELS).unwrap(),
    };
    let usage = BufferUsage::STORAGE | BufferUsage::COPY_SRC;
    let buffers: Vec<_> = views.iter().map(|v| buffer(b, &v.bytes, usage)).collect();
    let results_len = (TEXEL_INVOCATIONS * 6 * 16) as u64;
    let results = buffer(b, &vec![0; results_len as usize], usage);
    let align = 256;
    let start = |v: &TexelView| (v.first * FORMAT_FACTS[v.format as usize].block.2) / align * align;
    let view_of = |set: u32, binding: u32| {
        c.reflection
            .bindings
            .iter()
            .find(|r| r.set == set && r.binding == binding)
            .and_then(|r| {
                views
                    .iter()
                    .position(|v| v.kind == r.kind && v.register == r.register)
            })
    };
    let entries: Vec<_> = c
        .reflection
        .bindings
        .iter()
        .map(|r| {
            let range = match view_of(r.set, r.binding) {
                Some(i) => {
                    let offset = start(&views[i]) as u64;
                    BufferRange {
                        buffer: &buffers[i],
                        offset,
                        size: views[i].bytes.len() as u64 - offset,
                    }
                }
                None => BufferRange {
                    buffer: &results,
                    offset: 0,
                    size: results_len,
                },
            };
            BindingEntry {
                binding: r.binding,
                stages: StageSet::COMPUTE,
                resource: BindingResource::StorageBuffer {
                    range,
                    writable: r.kind == SlotKind::Storage,
                },
            }
        })
        .collect();
    let set = b.create_binding_set(&entries).unwrap();
    let mut helpers = vec![0u8; c.reflection.helper_size as usize];
    for h in &c.reflection.helpers {
        let HelperValue::TexelBufferView { set, binding } = h.value else {
            panic!("{h:?}");
        };
        let v = &views[view_of(set, binding).unwrap()];
        let at = h.offset as usize;
        let offset = v.first * FORMAT_FACTS[v.format as usize].block.2 - start(v);
        helpers[at..at + 4].copy_from_slice(&offset.to_le_bytes());
        helpers[at + 4..at + 8].copy_from_slice(&v.count.to_le_bytes());
    }
    let helper_buffer = buffer(b, &helpers, BufferUsage::UNIFORM);
    let helper_set = b
        .create_binding_set(&[BindingEntry {
            binding: helper_binding(ShaderStage::Compute),
            stages: StageSet::COMPUTE,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &helper_buffer,
                offset: 0,
                size: helpers.len() as u64,
            }),
        }])
        .unwrap();
    let shader = b.create_shader(&c.modules[0]).unwrap();
    let program = b.create_program(&Program::Compute(&shader)).unwrap();
    let mut bindings = [Some(&set), None, None, None];
    bindings[HELPER_SET as usize] = Some(&helper_set);
    b.dispatch(&DispatchCall {
        program: &program,
        bindings,
        immediates: &[],
        groups: DispatchGroups::Direct([TEXEL_INVOCATIONS / 64, 1, 1]),
    })
    .unwrap();
    let mut read = |buffer, size: usize| {
        let mut out = vec![0; size];
        b.read_back_now(
            &ReadSource::Buffer {
                buffer,
                offset: 0,
                size: size as u64,
            },
            &mut out,
        )
        .unwrap();
        out
    };
    let mut out = Vec::new();
    for (v, buffer) in views.iter().zip(&buffers) {
        if v.kind == SlotKind::Storage {
            out.push(read(buffer, v.bytes.len()));
        }
    }
    out.push(read(&results, results_len as usize));
    assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
    out
}

/// A component's byte, bit shift and width.
type Component = (u32, u32, u32);

/// A normalised format's components, and whether they are signed.
fn normalised(format: Format) -> Option<(Vec<Component>, bool)> {
    let even = |bits: u32, n: u32| (0..n).map(|i| (i * bits / 8, 0, bits)).collect();
    Some(match format {
        Format::R8Unorm | Format::R8Snorm => (even(8, 1), format == Format::R8Snorm),
        Format::Rg8Unorm | Format::Rg8Snorm => (even(8, 2), format == Format::Rg8Snorm),
        Format::Rgba8Unorm | Format::Rgba8Snorm => (even(8, 4), format == Format::Rgba8Snorm),
        Format::R16Unorm | Format::R16Snorm => (even(16, 1), format == Format::R16Snorm),
        Format::Rg16Unorm | Format::Rg16Snorm => (even(16, 2), format == Format::Rg16Snorm),
        Format::Rgba16Unorm | Format::Rgba16Snorm => (even(16, 4), format == Format::Rgba16Snorm),
        Format::Rgb10a2Unorm => (
            vec![(0, 0, 10), (0, 10, 10), (0, 20, 10), (0, 30, 2)],
            false,
        ),
        Format::Bgra8Unorm => (vec![(2, 0, 8), (1, 0, 8), (0, 0, 8), (3, 0, 8)], false),
        _ => return None,
    })
}

/// Elements of the stored view `v` that differ beyond Direct3D's conversion tolerance, and the
/// normalised components apart within it. Direct3D permits a float to normalised conversion to
/// give a code 0.6 from the exact value (functional specification 3.2.3.4 and 3.2.3.6); the value
/// stored is known from the results, so a component passes where both codes are that close.
fn stored_apart(v: &TexelView, results: &[u8], a: &[u8], o: &[u8]) -> (usize, usize) {
    let size = FORMAT_FACTS[v.format as usize].block.2 as usize;
    let word = |b: &[u8], at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let (mut apart, mut tolerated) = (0, 0);
    for (e, (x, y)) in a.chunks(size).zip(o.chunks(size)).enumerate() {
        if x == y {
            continue;
        }
        let i = e as u32 - v.first.min(e as u32);
        let inside = e as u32 >= v.first && i < v.count && i < TEXEL_INVOCATIONS;
        let Some((components, signed)) = normalised(v.format).filter(|_| inside) else {
            apart += 1;
            eprintln!("  u0 element {e}: direct3d {x:02x?}, ours {y:02x?}");
            continue;
        };
        let mut wrong = false;
        for (k, &(byte, shift, bits)) in components.iter().enumerate() {
            let code = |b: &[u8]| {
                let mut w = [0u8; 4];
                let n = (size - byte as usize).min(4);
                w[..n].copy_from_slice(&b[byte as usize..byte as usize + n]);
                let raw = (u32::from_le_bytes(w) >> shift) & ((1u64 << bits) - 1) as u32;
                match signed {
                    true => ((raw << (32 - bits)) as i32 >> (32 - bits)) as f64,
                    false => raw as f64,
                }
            };
            let (ca, co) = (code(x), code(y));
            if ca == co {
                continue;
            }
            let value = f32::from_bits(word(results, (i as usize * 6 + 5) * 16 + 4 * k));
            let (low, n) = if signed {
                (-1.0, bits - 1)
            } else {
                (0.0, bits)
            };
            let c = if value.is_nan() {
                0.0
            } else {
                (value as f64).clamp(low, 1.0)
            };
            let exact = c * ((1u64 << n) - 1) as f64;
            if (ca - exact).abs() <= 0.6 && (co - exact).abs() <= 0.6 {
                tolerated += 1;
            } else {
                wrong = true;
                eprintln!(
                    "  u0 element {e} component {k}: {value:e} is {exact}, direct3d {ca}, ours {co}"
                );
            }
        }
        apart += wrong as usize;
    }
    (apart, tolerated)
}

/// Whether `cs_texels`' views are of the kinds its declarations read or of others.
#[derive(Clone, Copy)]
enum Kinds {
    Declared,
    Other,
    /// The float views (t0, t3) of this format, the others as `Declared`.
    Read(Format),
}

#[test]
fn typed_buffers_match_direct3d() {
    typed_buffers_alike(Kinds::Declared);
}

/// `B8G8R8X8_UNORM` views read as Direct3D 11 reads them: blue, green and red from their bytes, alpha
/// one whatever the fourth byte holds.
#[test]
fn b8g8r8x8_typed_buffers_read_as_direct3d_reads_them() {
    typed_buffers_alike(Kinds::Read(Format::Bgrx8Unorm));
}

#[test]
fn typed_buffers_of_another_kind_read_and_write_their_bits_as_direct3d_does() {
    typed_buffers_alike(Kinds::Other);
}

/// `cs_texels` on seeded views of `kinds` through real Direct3D 11 and through ours, every word
/// compared.
fn typed_buffers_alike(kinds: Kinds) {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let seed: u64 = std::env::var("WCONTAINERS_COMPUTE_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_COMPUTE_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let mut failures = Vec::new();
    for seed in seed..seed + seeds {
        let views = texel_views(&gpu, seed, kinds);
        let formats: Vec<_> = views.iter().map(|v| v.format).collect();
        let reference = direct3d_texels(&gpu, &views);
        let builds = [
            (TexelBuild::Dxbc, "dxbc"),
            (TexelBuild::Dxil, "dxil"),
            (TexelBuild::Module, "module"),
        ];
        for (from, build) in builds {
            let made = ours_texels(&mut backend, &views, from);
            let results = &reference[2];
            for (n, (a, o)) in reference.iter().zip(&made).enumerate() {
                let name = ["u0", "u1", "results"][n];
                let name = format!("{build} {name}");
                if n == 0 {
                    let (apart, tolerated) = stored_apart(&views[4], results, a, o);
                    eprintln!(
                        "seed {seed} {name} ({formats:?}): {apart} elements apart, {tolerated} \
                     components within Direct3D's tolerance apart"
                    );
                    if apart > 0 {
                        failures.push(format!("seed {seed} {name}"));
                    }
                    continue;
                }
                let mut apart = 0;
                for (w, (x, y)) in a.chunks(4).zip(o.chunks(4)).enumerate() {
                    if x != y {
                        apart += 1;
                        if apart <= 8 {
                            eprintln!(
                                "  {name} word {w}: direct3d {:#010x}, ours {:#010x}",
                                u32::from_le_bytes(x.try_into().unwrap()),
                                u32::from_le_bytes(y.try_into().unwrap())
                            );
                        }
                    }
                }
                eprintln!("seed {seed} {name} ({formats:?}): {apart} words apart");
                if apart > 0 {
                    failures.push(format!("seed {seed} {name}"));
                }
            }
        }
    }
    eprintln!("{seeds} seeds, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// One of `cs_through`'s storage textures: slot, whether 3D (else 2D, arrayed when it has layers),
/// width, height and layers or depth, format, and its texels before the dispatch, layer by layer.
struct Through {
    slot: u32,
    volume: bool,
    size: [u32; 3],
    format: Format,
    texels: Vec<u8>,
}

/// Formats `cs_through` writes into its float array and its unsigned volume.
const THROUGH_FLOATS: [Format; 7] = [
    Format::Rgba16Float,
    Format::Rgba32Float,
    Format::Rg11b10Float,
    Format::R32Float,
    Format::Rg16Float,
    Format::R16Float,
    Format::Rg32Float,
];
const THROUGH_UINTS: [Format; 9] = [
    Format::Rgba16Uint,
    Format::Rgba8Uint,
    Format::Rgba32Uint,
    Format::R32Uint,
    Format::Rg32Uint,
    Format::Rg16Uint,
    Format::R8Uint,
    Format::R16Uint,
    Format::Rg8Uint,
];

fn through_views(gpu: &Gpu, seed: u64) -> Vec<Through> {
    let mut next = rng(seed);
    let can = |f: Format| {
        let need = D3D11_FORMAT_SUPPORT_TYPED_UNORDERED_ACCESS_VIEW.0 as u32;
        unsafe { gpu.device.CheckFormatSupport(dxgi(f)) }.is_ok_and(|s| s & need == need)
    };
    let floats: Vec<Format> = THROUGH_FLOATS.into_iter().filter(|&f| can(f)).collect();
    let uints: Vec<Format> = THROUGH_UINTS.into_iter().filter(|&f| can(f)).collect();
    let layered = floats[next() as usize % floats.len()];
    let volume = uints[next() as usize % uints.len()];
    [
        (3, false, [8, 8, 1], Format::R32Uint),
        (4, false, [12, 12, 1], Format::R32Float),
        (5, false, [11, 9, 3], layered),
        (6, true, [10, 10, 2], volume),
    ]
    .into_iter()
    .map(|(slot, volume, size, format)| {
        let n = size[0] * size[1] * size[2] * FORMAT_FACTS[format as usize].block.2;
        let texels = if format == Format::R32Float {
            (0..n / 4)
                .flat_map(|_| ((next() % 2000) as f32 / 16.0 - 60.0).to_le_bytes())
                .collect()
        } else {
            (0..n).map(|_| next() as u8).collect()
        };
        Through {
            slot,
            volume,
            size,
            format,
            texels,
        }
    })
    .collect()
}

/// Each texture's texels after real Direct3D 11 runs `shader` (`cs_through` or `cs_alpha`), layer
/// after layer.
fn direct3d_through(gpu: &Gpu, views: &[Through], shader: &str) -> Vec<Vec<u8>> {
    let shader = gpu.cs(&bytecode(&format!("{shader}.dxbc"))).unwrap();
    let mut resources: Vec<ID3D11Resource> = Vec::new();
    let mut uavs = vec![None; views[0].slot as usize];
    for t in views {
        let [w, h, l] = t.size;
        let row = w * FORMAT_FACTS[t.format as usize].block.2;
        let resource: ID3D11Resource = if t.volume {
            let desc = D3D11_TEXTURE3D_DESC {
                Width: w,
                Height: h,
                Depth: l,
                MipLevels: 1,
                Format: dxgi(t.format),
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_UNORDERED_ACCESS.0 as u32,
                ..Default::default()
            };
            let data = d3d11::data(&t.texels, row, row * h);
            d3d11::made(|o| unsafe { gpu.device.CreateTexture3D(&desc, Some(&data), Some(o)) })
                .unwrap()
                .cast()
                .unwrap()
        } else {
            let mut desc = d3d11::tex2d(w, h, dxgi(t.format), D3D11_BIND_UNORDERED_ACCESS);
            desc.ArraySize = l;
            let data: Vec<_> = t
                .texels
                .chunks((row * h) as usize)
                .map(|layer| d3d11::data(layer, row, row * h))
                .collect();
            gpu.texture2d(&desc, &data).unwrap().cast().unwrap()
        };
        uavs.push(Some(gpu.uav(&resource, None).unwrap()));
        resources.push(resource);
    }
    unsafe {
        let c = &gpu.context;
        c.CSSetShader(&shader, None);
        c.CSSetUnorderedAccessViews(0, uavs.len() as u32, Some(uavs.as_ptr()), None);
        c.Dispatch(2, 2, 1);
        let none: Vec<Option<ID3D11UnorderedAccessView>> = vec![None; uavs.len()];
        c.CSSetUnorderedAccessViews(0, none.len() as u32, Some(none.as_ptr()), None);
    }
    views
        .iter()
        .zip(&resources)
        .map(|(t, resource)| {
            let [w, h, l] = t.size;
            let row = (w * FORMAT_FACTS[t.format as usize].block.2) as usize;
            let staging: ID3D11Resource = if t.volume {
                let desc = D3D11_TEXTURE3D_DESC {
                    Width: w,
                    Height: h,
                    Depth: l,
                    MipLevels: 1,
                    Format: dxgi(t.format),
                    Usage: D3D11_USAGE_STAGING,
                    CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                    ..Default::default()
                };
                d3d11::made(|o| unsafe { gpu.device.CreateTexture3D(&desc, None, Some(o)) })
                    .unwrap()
                    .cast()
                    .unwrap()
            } else {
                let mut desc = d3d11::tex2d(w, h, dxgi(t.format), D3D11_BIND_FLAG(0));
                desc.ArraySize = l;
                desc.Usage = D3D11_USAGE_STAGING;
                desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                gpu.texture2d(&desc, &[]).unwrap().cast().unwrap()
            };
            let mut out = Vec::new();
            unsafe {
                gpu.context.CopyResource(&staging, resource);
                let subresources = if t.volume { 1 } else { l };
                for s in 0..subresources {
                    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                    gpu.context
                        .Map(&staging, s, D3D11_MAP_READ, 0, Some(&mut mapped))
                        .unwrap();
                    let slices = if t.volume { l } else { 1 };
                    for z in 0..slices as usize {
                        for y in 0..h as usize {
                            let start = mapped
                                .pData
                                .cast::<u8>()
                                .add(z * mapped.DepthPitch as usize + y * mapped.RowPitch as usize);
                            out.extend_from_slice(std::slice::from_raw_parts(start, row));
                        }
                    }
                    gpu.context.Unmap(&staging, s);
                }
            }
            out
        })
        .collect()
}

/// The same through the WebGPU back end, every texture read and written through a buffer whose rows
/// are 3 texels longer and layers one row taller than the view, the padding seeded too; from fxc's
/// DXBC or dxc's DXIL, whose slots are the views' places among the program's (Direct3D 12's
/// numbering).
fn ours_through<B: Backend>(
    b: &mut B,
    views: &[Through],
    shader: &str,
    seed: u64,
    dxil: bool,
) -> Vec<Vec<u8>> {
    let slot = |t: &Through| match dxil {
        true => views.iter().filter(|v| v.slot < t.slot).count() as u32,
        false => t.slot,
    };
    let key = VariantKey {
        texels_through_buffer: views.iter().map(|t| 1u64 << slot(t)).sum(),
        storage_formats: views.iter().map(|t| (slot(t), t.format)).collect(),
        ..Default::default()
    };
    let c = match dxil {
        true => convert(
            Source::Dxil(&bytecode(&format!("{shader}.dxil"))),
            &key,
            None,
        ),
        false => convert(
            Source::Dxbc(&bytecode(&format!("{shader}.dxbc"))),
            &key,
            None,
        ),
    }
    .unwrap();
    let mut next = rng(seed ^ 0x5eed);
    let layout = |t: &Through| (t.size[0] + 3, t.size[1] + 1);
    let usage = BufferUsage::STORAGE | BufferUsage::COPY_SRC;
    let buffers: Vec<_> = views
        .iter()
        .map(|t| {
            let size = FORMAT_FACTS[t.format as usize].block.2 as usize;
            let (pitch, rows) = layout(t);
            let len = (pitch * rows * t.size[2]) as usize * size;
            let mut bytes: Vec<u8> = (0..len.next_multiple_of(4)).map(|_| next() as u8).collect();
            for (n, texel) in t.texels.chunks(size).enumerate() {
                let n = n as u32;
                let (x, y, z) = (
                    n % t.size[0],
                    n / t.size[0] % t.size[1],
                    n / t.size[0] / t.size[1],
                );
                let at = ((z * rows + y) * pitch + x) as usize * size;
                bytes[at..at + size].copy_from_slice(texel);
            }
            (buffer(b, &bytes, usage), bytes.len())
        })
        .collect();
    let entries: Vec<_> = c
        .reflection
        .bindings
        .iter()
        .map(|r| {
            let i = views.iter().position(|t| t.slot == r.register).unwrap();
            BindingEntry {
                binding: r.binding,
                stages: StageSet::COMPUTE,
                resource: BindingResource::StorageBuffer {
                    range: BufferRange {
                        buffer: &buffers[i].0,
                        offset: 0,
                        size: buffers[i].1 as u64,
                    },
                    writable: true,
                },
            }
        })
        .collect();
    let set = b.create_binding_set(&entries).unwrap();
    let mut helpers = vec![0u8; c.reflection.helper_size as usize];
    for h in &c.reflection.helpers {
        let HelperValue::TexelLayout { slot: s } = h.value else {
            panic!("{h:?}");
        };
        let t = views.iter().find(|t| slot(t) == s).unwrap();
        let (pitch, rows) = layout(t);
        let words = [pitch, rows, t.size[0], t.size[1], t.size[2]];
        for (k, w) in words.iter().enumerate() {
            let at = h.offset as usize + 4 * k;
            helpers[at..at + 4].copy_from_slice(&w.to_le_bytes());
        }
    }
    let helper_buffer = buffer(b, &helpers, BufferUsage::UNIFORM);
    let helper_set = b
        .create_binding_set(&[BindingEntry {
            binding: helper_binding(ShaderStage::Compute),
            stages: StageSet::COMPUTE,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &helper_buffer,
                offset: 0,
                size: helpers.len() as u64,
            }),
        }])
        .unwrap();
    let shader = b.create_shader(&c.modules[0]).unwrap();
    let program = b.create_program(&Program::Compute(&shader)).unwrap();
    let mut bindings = [Some(&set), None, None, None];
    bindings[HELPER_SET as usize] = Some(&helper_set);
    b.dispatch(&DispatchCall {
        program: &program,
        bindings,
        immediates: &[],
        groups: DispatchGroups::Direct([2, 2, 1]),
    })
    .unwrap();
    let out = views
        .iter()
        .zip(&buffers)
        .map(|(t, (buffer, len))| {
            let mut bytes = vec![0; *len];
            b.read_back_now(
                &ReadSource::Buffer {
                    buffer,
                    offset: 0,
                    size: *len as u64,
                },
                &mut bytes,
            )
            .unwrap();
            let size = FORMAT_FACTS[t.format as usize].block.2 as usize;
            let (pitch, rows) = layout(t);
            let mut texels = Vec::new();
            for z in 0..t.size[2] {
                for y in 0..t.size[1] {
                    let at = ((z * rows + y) * pitch) as usize * size;
                    texels.extend_from_slice(&bytes[at..at + t.size[0] as usize * size]);
                }
            }
            texels
        })
        .collect();
    assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
    out
}

#[test]
fn storage_textures_through_buffers_match_direct3d() {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let seed: u64 = std::env::var("WCONTAINERS_COMPUTE_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_COMPUTE_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let mut failures = Vec::new();
    for seed in seed..seed + seeds {
        let views = through_views(&gpu, seed);
        let reference = direct3d_through(&gpu, &views, "cs_through");
        through_alike(
            &mut backend,
            seed,
            &views,
            &reference,
            "cs_through",
            &mut failures,
        );
    }
    eprintln!("{seeds} seeds, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// `shader` on `views` through ours, both builds, each texture's texels compared with `reference`.
fn through_alike<B: Backend>(
    backend: &mut B,
    seed: u64,
    views: &[Through],
    reference: &[Vec<u8>],
    shader: &str,
    failures: &mut Vec<String>,
) {
    for dxil in [false, true] {
        let made = ours_through(backend, views, shader, seed, dxil);
        let build = if dxil { "dxil" } else { "dxbc" };
        for ((a, o), t) in reference.iter().zip(&made).zip(views) {
            let size = FORMAT_FACTS[t.format as usize].block.2 as usize;
            let mut apart = 0;
            for (n, (x, y)) in a.chunks(size).zip(o.chunks(size)).enumerate() {
                if x != y {
                    apart += 1;
                    if apart <= 6 {
                        eprintln!("  u{} texel {n}: direct3d {x:02x?}, ours {y:02x?}", t.slot);
                    }
                }
            }
            eprintln!(
                "seed {seed} {build} {shader} u{} {:?}: {apart} texels apart",
                t.slot, t.format
            );
            if apart > 0 {
                failures.push(format!("seed {seed} {build} {shader} u{}", t.slot));
            }
        }
    }
}

/// `cs_alpha` loading and storing an `A8_UNORM` storage texture read through a buffer of its
/// one-byte texels, against Direct3D 11's typed loads and stores of the format: loads give
/// (0, 0, 0, a), stores keep the value's alpha.
#[test]
fn a8_storage_textures_load_and_store_through_buffers_as_direct3d_does() {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let seed: u64 = std::env::var("WCONTAINERS_COMPUTE_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_COMPUTE_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let mut failures = Vec::new();
    for seed in seed..seed + seeds {
        let mut next = rng(seed);
        let size = [ALPHA_SIDE, ALPHA_SIDE, 1];
        let views = [Through {
            slot: 0,
            volume: false,
            size,
            format: Format::A8Unorm,
            texels: (0..ALPHA_SIDE * ALPHA_SIDE).map(|_| next() as u8).collect(),
        }];
        let reference = direct3d_through(&gpu, &views, "cs_alpha");
        through_alike(
            &mut backend,
            seed,
            &views,
            &reference,
            "cs_alpha",
            &mut failures,
        );
    }
    eprintln!("{seeds} seeds, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The side of `cs_alpha`'s texture, which its dispatch of two by two groups covers.
const ALPHA_SIDE: u32 = 16;

/// `cs_one_d`'s textures: register, whether an unordered access view, format, layers (none for a
/// texture that is not an array), and whether seeded.
const ONE_D: [(u32, bool, Format, Option<u32>, bool); 4] = [
    (0, false, Format::R32Float, None, true),
    (1, false, Format::R32Float, Some(3), true),
    (0, true, Format::R32Float, None, false),
    (1, true, Format::R32Uint, Some(2), false),
];
/// Their width, one group of `cs_one_d`.
const ONE_D_WIDTH: u32 = 64;

/// A 1D texture's texels, layer after layer: seeded floats, or zeros.
fn one_d_texels(seed: u64, i: usize) -> Vec<u8> {
    let (_, _, format, layers, seeded) = ONE_D[i];
    let n = (ONE_D_WIDTH * layers.unwrap_or(1) * FORMAT_FACTS[format as usize].block.2) as usize;
    let mut next = rng(seed + i as u64);
    match seeded {
        true => (0..n / 4)
            .flat_map(|_| ((next() % 3000) as f32 / 64.0).to_le_bytes())
            .collect(),
        false => vec![0; n],
    }
}

/// Each output texture's texels after real Direct3D 11 runs `cs_one_d` on 1D textures.
fn direct3d_one_d(gpu: &Gpu, seed: u64) -> Vec<Vec<u8>> {
    let shader = gpu.cs(&bytecode("cs_one_d.dxbc")).unwrap();
    let mut srvs = Vec::new();
    let mut uavs = Vec::new();
    let mut outputs = Vec::new();
    for (i, &(_, uav, format, layers, _)) in ONE_D.iter().enumerate() {
        let desc = D3D11_TEXTURE1D_DESC {
            Width: ONE_D_WIDTH,
            MipLevels: 1,
            ArraySize: layers.unwrap_or(1),
            Format: dxgi(format),
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: match uav {
                true => D3D11_BIND_UNORDERED_ACCESS.0 as u32,
                false => D3D11_BIND_SHADER_RESOURCE.0 as u32,
            },
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let texels = one_d_texels(seed, i);
        let row = ONE_D_WIDTH * FORMAT_FACTS[format as usize].block.2;
        let data: Vec<_> = texels
            .chunks(row as usize)
            .map(|layer| d3d11::data(layer, row, row))
            .collect();
        let texture: ID3D11Texture1D = d3d11::made(|o| unsafe {
            gpu.device
                .CreateTexture1D(&desc, Some(data.as_ptr()), Some(o))
        })
        .unwrap();
        let resource: ID3D11Resource = texture.cast().unwrap();
        match uav {
            true => {
                uavs.push(Some(gpu.uav(&resource, None).unwrap()));
                outputs.push((texture, desc, row));
            }
            false => srvs.push(Some(gpu.srv(&resource, None).unwrap())),
        }
    }
    let sampler = gpu
        .sampler(&D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            MaxLOD: f32::MAX,
            ..Default::default()
        })
        .unwrap();
    unsafe {
        let c = &gpu.context;
        c.CSSetShader(&shader, None);
        c.CSSetShaderResources(0, Some(&srvs));
        c.CSSetSamplers(0, Some(&[Some(sampler)]));
        c.CSSetUnorderedAccessViews(0, uavs.len() as u32, Some(uavs.as_ptr()), None);
        c.Dispatch(1, 1, 1);
        let none: Vec<Option<ID3D11UnorderedAccessView>> = vec![None; uavs.len()];
        c.CSSetUnorderedAccessViews(0, none.len() as u32, Some(none.as_ptr()), None);
    }
    outputs
        .iter()
        .map(|(texture, desc, row)| {
            let mut staging_desc = *desc;
            staging_desc.Usage = D3D11_USAGE_STAGING;
            staging_desc.BindFlags = 0;
            staging_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            let staging: ID3D11Texture1D = d3d11::made(|o| unsafe {
                gpu.device.CreateTexture1D(&staging_desc, None, Some(o))
            })
            .unwrap();
            let mut out = Vec::new();
            unsafe {
                gpu.context.CopyResource(&staging, texture);
                for layer in 0..desc.ArraySize {
                    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                    gpu.context
                        .Map(&staging, layer, D3D11_MAP_READ, 0, Some(&mut mapped))
                        .unwrap();
                    out.extend_from_slice(std::slice::from_raw_parts(
                        mapped.pData.cast::<u8>(),
                        *row as usize,
                    ));
                    gpu.context.Unmap(&staging, layer);
                }
            }
            out
        })
        .collect()
}

/// The same through the WebGPU back end on 2D textures one texel high, as the fronts make 1D ones,
/// from fxc's bytecode (DXBC) or dxc's (DXIL).
fn ours_one_d<B: Backend>(b: &mut B, seed: u64, dxil: bool) -> Vec<Vec<u8>> {
    let key = VariantKey {
        storage_formats: ONE_D.iter().filter(|t| t.1).map(|t| (t.0, t.2)).collect(),
        ..Default::default()
    };
    let c = match dxil {
        true => convert(Source::Dxil(&bytecode("cs_one_d.dxil")), &key, None),
        false => convert(Source::Dxbc(&bytecode("cs_one_d.dxbc")), &key, None),
    }
    .unwrap();
    assert!(c.reflection.helpers.is_empty());
    let shader = b.create_shader(&c.modules[0]).unwrap();
    let program = b.create_program(&Program::Compute(&shader)).unwrap();
    let mut textures = Vec::new();
    for (i, &(_, uav, format, layers, _)) in ONE_D.iter().enumerate() {
        let count = layers.unwrap_or(1);
        let texture = b
            .create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width: ONE_D_WIDTH,
                height: 1,
                depth_or_layers: count,
                mip_levels: 1,
                samples: 1,
                usage: match uav {
                    true => TextureUsage::STORAGE,
                    false => TextureUsage::SAMPLE,
                } | TextureUsage::COPY_SRC
                    | TextureUsage::COPY_DST,
                cube: false,
                reinterpret: false,
            })
            .unwrap();
        let view = b
            .create_view(
                &texture,
                &TextureViewDesc {
                    format,
                    dimension: match layers {
                        Some(_) => ViewDimension::D2Array,
                        None => ViewDimension::D2,
                    },
                    base_mip: 0,
                    mip_count: 1,
                    base_layer: 0,
                    layer_count: count,
                    stencil: false,
                },
            )
            .unwrap();
        let extent = Extent {
            width: ONE_D_WIDTH,
            height: 1,
            depth_or_layers: count,
        };
        b.write_texture(
            &TextureRegion {
                texture: &texture,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent,
            &one_d_texels(seed, i),
            ONE_D_WIDTH * FORMAT_FACTS[format as usize].block.2,
            1,
            WriteMode::Ordered,
        );
        textures.push((texture, view, extent));
    }
    let sampler = b
        .create_sampler(&SamplerDesc {
            min: Filter::Linear,
            mag: Filter::Linear,
            mip: Filter::Linear,
            address: [AddressMode::Clamp; 3],
            lod_range: [0, f32::MAX.to_bits()],
            anisotropy: 1,
            compare: None,
            border: BorderColor::TransparentBlack,
        })
        .unwrap();
    let entries: Vec<_> = c
        .reflection
        .bindings
        .iter()
        .map(|r| {
            let at = |uav: bool| {
                ONE_D
                    .iter()
                    .position(|t| (t.0, t.1) == (r.register, uav))
                    .unwrap()
            };
            BindingEntry {
                binding: r.binding,
                stages: StageSet::COMPUTE,
                resource: match r.kind {
                    SlotKind::Texture => BindingResource::Texture(&textures[at(false)].1),
                    SlotKind::Storage => BindingResource::StorageTexture {
                        view: &textures[at(true)].1,
                        writable: true,
                    },
                    SlotKind::Sampler => BindingResource::Sampler(&sampler),
                    k => panic!("{k:?}"),
                },
            }
        })
        .collect();
    let set = b.create_binding_set(&entries).unwrap();
    b.dispatch(&DispatchCall {
        program: &program,
        bindings: [Some(&set), None, None, None],
        immediates: &[],
        groups: DispatchGroups::Direct([1, 1, 1]),
    })
    .unwrap();
    ONE_D
        .iter()
        .zip(&textures)
        .filter(|(t, _)| t.1)
        .map(|(t, (texture, _, extent))| {
            let mut out = vec![
                0;
                (extent.width * extent.depth_or_layers * FORMAT_FACTS[t.2 as usize].block.2)
                    as usize
            ];
            b.read_back_now(
                &ReadSource::Texture {
                    region: TextureRegion {
                        texture,
                        mip: 0,
                        layer: 0,
                        origin: Origin::default(),
                        stencil: false,
                    },
                    extent: *extent,
                },
                &mut out,
            )
            .unwrap();
            out
        })
        .collect()
}

/// 1D textures and arrays, loaded, sampled, sized and stored, through both routes as 2D textures one
/// texel high, against real Direct3D 11's 1D textures.
#[test]
fn one_d_textures_match_direct3d() {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let seed: u64 = std::env::var("WCONTAINERS_COMPUTE_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_COMPUTE_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let mut failures = Vec::new();
    for seed in seed..seed + seeds {
        let reference = direct3d_one_d(&gpu, seed);
        for (dxil, source) in [(false, "dxbc"), (true, "dxil")] {
            let made = ours_one_d(&mut backend, seed, dxil);
            for (k, (a, o)) in reference.iter().zip(&made).enumerate() {
                let apart = a.chunks(4).zip(o.chunks(4)).filter(|(x, y)| x != y).count();
                eprintln!("seed {seed} {source} u{k}: {apart} words apart");
                if apart > 0 {
                    failures.push(format!("seed {seed} {source} u{k}"));
                }
            }
        }
    }
    eprintln!("{} seeds, {} failed", seeds, failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Views `cs_view_kinds` reads its three textures (float, unsigned and signed declarations) through,
/// of every component kind.
const VIEW_KIND_FORMATS: [Format; 11] = [
    Format::Rgba8Unorm,
    Format::Rgba8Snorm,
    Format::Rgba16Float,
    Format::Rgba8Uint,
    Format::Rgba16Uint,
    Format::Rgba32Uint,
    Format::R32Uint,
    Format::Rgba8Sint,
    Format::Rgba16Sint,
    Format::Rgba32Sint,
    Format::R32Sint,
];
/// The side of `cs_view_kinds`'s textures, one group of it.
const VIEW_KIND_SIDE: u32 = 8;
/// Its textures, one per declared kind; its output holds each one's loads side by side.
const VIEW_KIND_TEXTURES: u32 = 3;
const VIEW_KIND_OUTPUT: Format = Format::Rgba32Uint;

/// A seeded view format for each texture slot, and its texels: seeded bytes, half floats kept finite.
fn view_kind_views(seed: u64) -> Vec<(Format, Vec<u8>)> {
    let mut next = rng(seed);
    (0..VIEW_KIND_TEXTURES)
        .map(|_| {
            let format = VIEW_KIND_FORMATS[next() as usize % VIEW_KIND_FORMATS.len()];
            let n =
                (VIEW_KIND_SIDE * VIEW_KIND_SIDE * FORMAT_FACTS[format as usize].block.2) as usize;
            let mut texels: Vec<u8> = (0..n).map(|_| next() as u8).collect();
            if format == Format::Rgba16Float {
                // The top exponent bit of each half cleared: no infinities or NaNs.
                let top_exponent_bit = 0x40;
                for high in texels
                    .iter_mut()
                    .skip(1)
                    .step_by(std::mem::size_of::<u16>())
                {
                    *high &= !top_exponent_bit;
                }
            }
            (format, texels)
        })
        .collect()
}

/// The output's texels after real Direct3D 11 runs `cs_view_kinds` with `views` bound.
fn direct3d_view_kinds(gpu: &Gpu, views: &[(Format, Vec<u8>)]) -> Vec<u8> {
    let shader = gpu.cs(&bytecode("cs_view_kinds.dxbc")).unwrap();
    let srvs: Vec<_> = views
        .iter()
        .map(|(format, texels)| {
            let row = VIEW_KIND_SIDE * FORMAT_FACTS[*format as usize].block.2;
            let desc = d3d11::tex2d(
                VIEW_KIND_SIDE,
                VIEW_KIND_SIDE,
                dxgi(*format),
                D3D11_BIND_SHADER_RESOURCE,
            );
            let texture = gpu
                .texture2d(&desc, &[d3d11::data(texels, row, 0)])
                .unwrap();
            Some(gpu.srv(&texture.cast().unwrap(), None).unwrap())
        })
        .collect();
    let width = VIEW_KIND_SIDE * VIEW_KIND_TEXTURES;
    let desc = d3d11::tex2d(
        width,
        VIEW_KIND_SIDE,
        dxgi(VIEW_KIND_OUTPUT),
        D3D11_BIND_UNORDERED_ACCESS,
    );
    let output = gpu.texture2d(&desc, &[]).unwrap();
    let uav = Some(gpu.uav(&output.cast().unwrap(), None).unwrap());
    unsafe {
        let c = &gpu.context;
        c.CSSetShader(&shader, None);
        c.CSSetShaderResources(0, Some(&srvs));
        c.CSSetUnorderedAccessViews(0, 1, Some(&uav), None);
        c.Dispatch(1, 1, 1);
        c.CSSetUnorderedAccessViews(0, 1, Some(&None), None);
    }
    let mut staging_desc = desc;
    staging_desc.Usage = D3D11_USAGE_STAGING;
    staging_desc.BindFlags = 0;
    staging_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
    let staging = gpu.texture2d(&staging_desc, &[]).unwrap();
    let row = (width * FORMAT_FACTS[VIEW_KIND_OUTPUT as usize].block.2) as usize;
    let mut out = Vec::new();
    unsafe {
        gpu.context.CopyResource(&staging, &output);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        gpu.context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .unwrap();
        for y in 0..VIEW_KIND_SIDE as usize {
            let at = mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize);
            out.extend_from_slice(std::slice::from_raw_parts(at, row));
        }
        gpu.context.Unmap(&staging, 0);
    }
    out
}

/// The same through the WebGPU back end, each texture of its view's format, from fxc's bytecode
/// (DXBC) or dxc's (DXIL), with the views' formats in `texel_formats`.
fn ours_view_kinds<B: Backend>(b: &mut B, views: &[(Format, Vec<u8>)], dxil: bool) -> Vec<u8> {
    let key = VariantKey {
        texel_formats: (0..).zip(views.iter().map(|v| v.0)).collect(),
        storage_formats: vec![(0, VIEW_KIND_OUTPUT)],
        ..Default::default()
    };
    let c = match dxil {
        true => convert(Source::Dxil(&bytecode("cs_view_kinds.dxil")), &key, None),
        false => convert(Source::Dxbc(&bytecode("cs_view_kinds.dxbc")), &key, None),
    }
    .unwrap();
    let shader = b.create_shader(&c.modules[0]).unwrap();
    let program = b.create_program(&Program::Compute(&shader)).unwrap();
    let made = |b: &mut B, format: Format, width: u32, usage: TextureUsage| {
        let texture = b
            .create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width,
                height: VIEW_KIND_SIDE,
                depth_or_layers: 1,
                mip_levels: 1,
                samples: 1,
                usage: usage | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
                cube: false,
                reinterpret: false,
            })
            .unwrap();
        let view = b
            .create_view(
                &texture,
                &TextureViewDesc {
                    format,
                    dimension: ViewDimension::D2,
                    base_mip: 0,
                    mip_count: 1,
                    base_layer: 0,
                    layer_count: 1,
                    stencil: false,
                },
            )
            .unwrap();
        (texture, view)
    };
    let mut textures = Vec::new();
    for (format, texels) in views {
        let (texture, view) = made(b, *format, VIEW_KIND_SIDE, TextureUsage::SAMPLE);
        b.write_texture(
            &TextureRegion {
                texture: &texture,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width: VIEW_KIND_SIDE,
                height: VIEW_KIND_SIDE,
                depth_or_layers: 1,
            },
            texels,
            VIEW_KIND_SIDE * FORMAT_FACTS[*format as usize].block.2,
            VIEW_KIND_SIDE,
            WriteMode::Ordered,
        );
        textures.push((texture, view));
    }
    let width = VIEW_KIND_SIDE * VIEW_KIND_TEXTURES;
    let (output, output_view) = made(b, VIEW_KIND_OUTPUT, width, TextureUsage::STORAGE);
    let entries: Vec<_> = c
        .reflection
        .bindings
        .iter()
        .map(|r| BindingEntry {
            binding: r.binding,
            stages: StageSet::COMPUTE,
            resource: match r.kind {
                SlotKind::Texture => BindingResource::Texture(&textures[r.register as usize].1),
                SlotKind::Storage => BindingResource::StorageTexture {
                    view: &output_view,
                    writable: true,
                },
                k => panic!("{k:?}"),
            },
        })
        .collect();
    let set = b.create_binding_set(&entries).unwrap();
    b.dispatch(&DispatchCall {
        program: &program,
        bindings: [Some(&set), None, None, None],
        immediates: &[],
        groups: DispatchGroups::Direct([1, 1, 1]),
    })
    .unwrap();
    let mut out = vec![
        0;
        (width * VIEW_KIND_SIDE * FORMAT_FACTS[VIEW_KIND_OUTPUT as usize].block.2)
            as usize
    ];
    b.read_back_now(
        &ReadSource::Texture {
            region: TextureRegion {
                texture: &output,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width,
                height: VIEW_KIND_SIDE,
                depth_or_layers: 1,
            },
        },
        &mut out,
    )
    .unwrap();
    out
}

/// Textures bound through views of another component kind than declared (and of the declared kind),
/// loaded by both routes, against real Direct3D 11: every word of every load equal, the view's value
/// read by its bits, as Direct3D 12's texture-load test expects too.
#[test]
fn textures_of_another_kind_load_their_bits_as_direct3d_does() {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let seed: u64 = std::env::var("WCONTAINERS_COMPUTE_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_COMPUTE_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(8);
    let mut failures = Vec::new();
    let mut other_kind = 0;
    for seed in seed..seed + seeds {
        let views = view_kind_views(seed);
        let formats: Vec<Format> = views.iter().map(|v| v.0).collect();
        other_kind += formats
            .iter()
            .zip([FormatKindOf::Float, FormatKindOf::Uint, FormatKindOf::Sint])
            .filter(|(f, declared)| FormatKindOf::of(**f) != *declared)
            .count();
        let reference = direct3d_view_kinds(&gpu, &views);
        for (dxil, source) in [(false, "dxbc"), (true, "dxil")] {
            let made = ours_view_kinds(&mut backend, &views, dxil);
            let apart = reference
                .chunks(std::mem::size_of::<u32>())
                .zip(made.chunks(std::mem::size_of::<u32>()))
                .filter(|(x, y)| x != y)
                .count();
            eprintln!("seed {seed} {source} {formats:?}: {apart} words apart");
            if apart > 0 {
                failures.push(format!("seed {seed} {source} {formats:?}"));
            }
        }
    }
    eprintln!(
        "{seeds} seeds, {other_kind} views of another kind, {} failed",
        failures.len()
    );
    assert!(other_kind > 0);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A view format's component kind as a texture declaration names it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FormatKindOf {
    Float,
    Uint,
    Sint,
}

impl FormatKindOf {
    fn of(format: Format) -> FormatKindOf {
        match FORMAT_FACTS[format as usize].kind {
            wcontainers_gpu::FormatKind::Uint => FormatKindOf::Uint,
            wcontainers_gpu::FormatKind::Sint => FormatKindOf::Sint,
            _ => FormatKindOf::Float,
        }
    }
}
