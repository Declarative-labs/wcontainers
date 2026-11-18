//! Compute shaders with atomic operations and append, consume and counter views (`compute.hlsl`) on the
//! WebGPU back end, against real Direct3D 11 running the same shaders on the same GPU. The values
//! consumed are seeded: `WCONTAINERS_COMPUTE_SEED` (default 1) and `WCONTAINERS_COMPUTE_CASES` seeds
//! from it (default 4).

#![cfg(windows)]

mod common;

use common::*;
use wcontainers_gpu::{
    Backend, BindingEntry, BindingResource, BufferRange, BufferUsage, DispatchCall, DispatchGroups,
    Program, ReadSource, SlotKind, StageSet, VariantKey,
};
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::{Source, convert};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

/// One storage view: its bytes before the dispatch, whether it is a raw buffer, the count its
/// counter starts at (none: no counter), and whether it is an append or consume view.
struct View {
    bytes: Vec<u8>,
    raw: bool,
    counter: Option<u32>,
    append: bool,
}

/// One case: a shader, its views by slot and its workgroups.
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
    let consumed: Vec<u32> = (0..256).map(|_| next() >> 4).collect();
    let returned: Vec<u32> = (0..32).map(|_| next() >> 4).collect();
    vec![
        Case {
            shader: "cs_atomics",
            views: vec![
                View {
                    bytes: words(&raw),
                    raw: true,
                    counter: None,
                    append: false,
                },
                View {
                    bytes: words(&[i32::MAX as u32, i32::MIN as u32, 5, 0]),
                    raw: false,
                    counter: None,
                    append: false,
                },
                View {
                    bytes: vec![0; 2048],
                    raw: false,
                    counter: Some(0),
                    append: false,
                },
                View {
                    bytes: vec![0; 2048],
                    raw: false,
                    counter: Some(0),
                    append: true,
                },
                View {
                    bytes: words(&consumed),
                    raw: false,
                    counter: Some(256),
                    append: true,
                },
            ],
            groups: 4,
        },
        Case {
            shader: "cs_decrement",
            views: vec![View {
                bytes: words(&returned),
                raw: false,
                counter: Some(32),
                append: false,
            }],
            groups: 1,
        },
    ]
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
    for v in &case.views {
        let buffer = gpu
            .buffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: v.bytes.len() as u32,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_UNORDERED_ACCESS.0 as u32,
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
                    FirstElement: 0,
                    NumElements: v.bytes.len() as u32 / 4,
                    Flags: flags,
                },
            },
        };
        views.push(Some(gpu.uav(&buffer.clone().into(), Some(&desc)).unwrap()));
        buffers.push(buffer);
    }
    let initial: Vec<u32> = case.views.iter().map(|v| v.counter.unwrap_or(!0)).collect();
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
        c.CSSetUnorderedAccessViews(
            0,
            views.len() as u32,
            Some(views.as_ptr()),
            Some(initial.as_ptr()),
        );
        c.Dispatch(case.groups, 1, 1);
        for (i, (v, view)) in case.views.iter().zip(&views).enumerate() {
            if v.counter.is_some() {
                c.CopyStructureCount(&counts, 4 * i as u32, view.as_ref().unwrap());
            }
        }
        let none: Vec<Option<ID3D11UnorderedAccessView>> = vec![None; views.len()];
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

fn ours<B: Backend>(b: &mut B, case: &Case) -> Ran {
    let c = convert(
        Source::Dxbc(&bytecode(&format!("{}.dxbc", case.shader))),
        &VariantKey::default(),
        None,
    )
    .unwrap();
    assert!(c.reflection.helpers.is_empty());
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
            let slot = r.register as usize;
            let range = match r.kind {
                SlotKind::Storage => BufferRange {
                    buffer: &buffers[slot],
                    offset: 0,
                    size: case.views[slot].bytes.len() as u64,
                },
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
    Ran { views, counters }
}

/// Makes what depends on the order invocations ran in comparable: the value the last exchange left
/// (word 648 of `cs_atomics`' raw view) is marked as the values the exchanges returned are, and the
/// appended values are sorted.
fn ordered(case: &Case, ran: &mut Ran) {
    if case.shader != "cs_atomics" {
        return;
    }
    let raw = &mut ran.views[0];
    let last = u32::from_le_bytes(raw[2592..2596].try_into().unwrap()) as usize;
    raw[2592..2596].fill(0);
    if 1024 + last < 2048 {
        raw[4 * (1024 + last)..4 * (1024 + last) + 4].copy_from_slice(&1u32.to_le_bytes());
    }
    let n = ran.counters[3].unwrap_or(0).min(512) as usize;
    let mut appended: Vec<u32> = ran.views[3][..4 * n]
        .chunks(4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
        .collect();
    appended.sort_unstable();
    ran.views[3][..4 * n].copy_from_slice(&words(&appended));
}

#[test]
fn atomics_and_counters_match_direct3d() {
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
    let mut compared = 0;
    for seed in seed..seed + seeds {
        for case in cases(seed) {
            let name = format!("seed {seed} {}", case.shader);
            let mut reference = direct3d(&gpu, &case);
            let mut made = ours(&mut backend, &case);
            ordered(&case, &mut reference);
            ordered(&case, &mut made);
            compared += 1;
            let mut apart = 0;
            for (slot, (a, o)) in reference.views.iter().zip(&made.views).enumerate() {
                for (w, (x, y)) in a.chunks(4).zip(o.chunks(4)).enumerate() {
                    if x != y {
                        apart += 1;
                        if apart <= 8 {
                            eprintln!(
                                "  view {slot} word {w}: direct3d {:#010x}, ours {:#010x}",
                                u32::from_le_bytes(x.try_into().unwrap()),
                                u32::from_le_bytes(y.try_into().unwrap())
                            );
                        }
                    }
                }
            }
            eprintln!(
                "{name}: counters direct3d {:?}, ours {:?}; {apart} words apart",
                reference.counters, made.counters
            );
            if apart > 0 || reference.counters != made.counters {
                failures.push(name);
            }
        }
    }
    eprintln!("{compared} comparisons, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}
