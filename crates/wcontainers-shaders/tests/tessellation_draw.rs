//! The stage helper's tessellator on the WebGPU back end against real Direct3D 11's on the same GPU:
//! patches of seeded factors for every domain, partitioning and output topology, Direct3D's domain
//! shader output streamed (where each point lies and which patch made it), against our tessellator's
//! points and indices read back, word for word and in order. `WCONTAINERS_TESSELLATION_SEED` (default
//! 1) and `WCONTAINERS_TESSELLATION_CASES` seeds from it (default 4); every topology runs per seed.

#![cfg(windows)]

mod common;

use common::*;
use wcontainers_gpu::{Backend, BufferRange, BufferUsage, ReadSource};
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::stage::{Tessellation, Tessellator, tess};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::core::s;

/// Patches per draw; the constant buffer holds 32.
const PATCHES: usize = 8;

/// One streamed vertex: the domain location's three components (f32 bits) and the primitive number.
type Vertex = [u32; 4];

/// A factor drawn from ranges that reach every rule: ordinary values past the largest, whole numbers,
/// values a step either side of whole and half numbers, and values that cull a patch or clamp.
fn factor(rng: &mut fastrand::Rng) -> f32 {
    match rng.u32(0..100) {
        0..45 => rng.f32() * 66.0 + 0.5,
        45..60 => rng.u32(1..=64) as f32,
        60..75 => {
            let k = rng.u32(1..64) as f32 + [0.0, 0.5][rng.usize(0..2)];
            let step = f32::EPSILON * k;
            [k - step, k, k + step][rng.usize(0..3)]
        }
        75..80 => [f32::NAN, 0.0, -1.0, f32::INFINITY, 1e-40, 100.0, -0.0][rng.usize(0..7)],
        _ => rng.f32() * 3.0 + 1.0,
    }
}

/// Each patch's outer factors, then its inner ones; an edge factor that culls is rarer than in
/// [`factor`], so most patches make points.
fn factors(rng: &mut fastrand::Rng) -> Vec<[f32; 8]> {
    (0..PATCHES)
        .map(|_| {
            let mut f = [0.0; 8];
            for v in &mut f[..6] {
                *v = factor(rng);
                if rng.u32(0..4) != 0 && (v.is_nan() || *v <= 0.0) {
                    *v = rng.f32() * 8.0 + 1.0;
                }
            }
            f
        })
        .collect()
}

/// Real Direct3D 11's streamed domain shader output for one draw of the patches, and how many
/// primitives it wrote.
fn direct3d(
    gpu: &Gpu,
    hs: &str,
    ds: &str,
    bound: usize,
    factors: &[[f32; 8]],
) -> (Vec<Vertex>, u64) {
    let vs = gpu.vs(&bytecode("vs_patch.dxbc")).unwrap();
    let hull = gpu.hs(&bytecode(&format!("{hs}.dxbc"))).unwrap();
    let domain_code = bytecode(&format!("{ds}.dxbc"));
    let domain = gpu.ds(&domain_code).unwrap();
    let entries = [
        D3D11_SO_DECLARATION_ENTRY {
            Stream: 0,
            SemanticName: s!("DOMAIN"),
            SemanticIndex: 0,
            StartComponent: 0,
            ComponentCount: 3,
            OutputSlot: 0,
        },
        D3D11_SO_DECLARATION_ENTRY {
            Stream: 0,
            SemanticName: s!("PRIMITIVE"),
            SemanticIndex: 0,
            StartComponent: 0,
            ComponentCount: 1,
            OutputSlot: 0,
        },
    ];
    let streamed: ID3D11GeometryShader = d3d11::made(|o| unsafe {
        gpu.device.CreateGeometryShaderWithStreamOutput(
            &domain_code,
            Some(&entries),
            Some(&[16]),
            D3D11_SO_NO_RASTERIZED_STREAM,
            None::<&ID3D11ClassLinkage>,
            Some(o),
        )
    })
    .unwrap();
    let mut constants = vec![0u8; 1024];
    for (p, f) in factors.iter().enumerate() {
        for (i, v) in f.iter().enumerate() {
            constants[p * 32 + i * 4..][..4].copy_from_slice(&v.to_le_bytes());
        }
    }
    let constants = gpu
        .immutable(&constants, D3D11_BIND_CONSTANT_BUFFER)
        .unwrap();
    let size = (bound * 16) as u32;
    let target = gpu
        .buffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: size,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_STREAM_OUTPUT.0 as u32,
                ..Default::default()
            },
            None,
        )
        .unwrap();
    let query: ID3D11Query = d3d11::made(|q| unsafe {
        gpu.device.CreateQuery(
            &D3D11_QUERY_DESC {
                Query: D3D11_QUERY_SO_STATISTICS_STREAM0,
                MiscFlags: 0,
            },
            Some(q),
        )
    })
    .unwrap();
    let mut statistics = D3D11_QUERY_DATA_SO_STATISTICS::default();
    unsafe {
        let c = &gpu.context;
        c.IASetInputLayout(None);
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST);
        c.VSSetShader(&vs, None);
        c.HSSetShader(&hull, None);
        c.HSSetConstantBuffers(0, Some(&[Some(constants)]));
        c.DSSetShader(&domain, None);
        c.GSSetShader(&streamed, None);
        c.PSSetShader(None, None);
        let targets = [Some(target.clone())];
        c.SOSetTargets(1, Some(targets.as_ptr()), Some([0u32].as_ptr()));
        c.Begin(&query);
        c.Draw(factors.len() as u32, 0);
        c.End(&query);
        loop {
            let hr = (windows::core::Interface::vtable(c).GetData)(
                windows::core::Interface::as_raw(c),
                windows::core::Interface::as_raw(&query),
                (&mut statistics as *mut D3D11_QUERY_DATA_SO_STATISTICS).cast(),
                std::mem::size_of::<D3D11_QUERY_DATA_SO_STATISTICS>() as u32,
                0,
            );
            if hr == windows::Win32::Foundation::S_OK {
                break;
            }
            hr.ok().unwrap();
        }
        c.SOSetTargets(0, None, None);
        c.GSSetShader(None, None);
        c.HSSetShader(None, None);
        c.DSSetShader(None, None);
    }
    assert!(
        statistics.NumPrimitivesWritten == statistics.PrimitivesStorageNeeded,
        "the stream output buffer was too small"
    );
    let bytes = gpu.read_buffer(&target).unwrap();
    let words: Vec<u32> = bytes
        .chunks(4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
        .collect();
    let vertices = words.chunks(4).map(|v| [v[0], v[1], v[2], v[3]]).collect();
    (vertices, statistics.NumPrimitivesWritten)
}

/// Our tessellator's points and indices for the same patches, as the vertices a draw of them makes.
fn ours<B: Backend>(
    b: &mut B,
    tessellator: &mut Tessellator<B>,
    t: &Tessellation,
    factors: &[[f32; 8]],
) -> Vec<Vertex> {
    let mut words = vec![tess::NO_SLOT; tess::HEADER_WORDS as usize];
    words[tess::PATCH_WORDS as usize] = 8;
    words[tess::POINT_WORDS as usize] = 0;
    words[tess::CONSTANT_BASE as usize] = 0;
    words[tess::FACTOR_BASE as usize] = 0;
    for f in factors {
        words.extend(f.iter().map(|v| v.to_bits()));
    }
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let hull = buffer(b, &bytes, BufferUsage::STORAGE);
    let n = factors.len() as u32;
    tessellator
        .run(
            b,
            t,
            BufferRange {
                buffer: &hull,
                offset: 0,
                size: bytes.len() as u64,
            },
            n,
            n,
        )
        .unwrap();
    let read = |b: &mut B, buffer: &B::Buffer, words: usize| -> Vec<u32> {
        let mut out = vec![0u8; words * 4];
        if words > 0 {
            b.read_back_now(
                &ReadSource::Buffer {
                    buffer,
                    offset: 0,
                    size: out.len() as u64,
                },
                &mut out,
            )
            .unwrap();
        }
        out.chunks(4)
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
            .collect()
    };
    let counts = read(b, tessellator.counts().unwrap(), 8);
    let indices = read(b, tessellator.indices().unwrap(), counts[0] as usize);
    let points = read(b, tessellator.points().unwrap(), counts[5] as usize * 4);
    assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
    indices
        .iter()
        .map(|&i| {
            let p = &points[4 * i as usize..][..4];
            let (u, v) = (f32::from_bits(p[0]), f32::from_bits(p[1]));
            let w = if t.domain == 0 { 1.0 - u - v } else { 0.0 };
            [p[0], p[1], w.to_bits(), p[3]]
        })
        .collect()
}

/// A primitive turned to its least rotation (so a degenerate one, with a vertex twice, has one form),
/// winding kept.
fn turned(p: &[Vertex]) -> Vec<Vertex> {
    (0..p.len())
        .map(|first| (0..p.len()).map(|i| p[(first + i) % p.len()]).collect())
        .min()
        .unwrap()
}

/// Direct3D leaves the order of a patch's primitives to the device, so each patch's primitives are
/// compared as a set, each turned to its least rotation with its winding kept; the patches
/// must come in draw order in both. Says how many primitives also start at the same vertex, and
/// whether the whole order is the same.
fn compare(reference: &[Vertex], ours: &[Vertex], per: usize) -> Result<String, String> {
    let patches = |v: &[Vertex]| -> Result<Vec<Vec<Vec<Vertex>>>, String> {
        let mut out: Vec<Vec<Vec<Vertex>>> = vec![Vec::new(); PATCHES];
        let mut last = 0;
        for p in v.chunks(per) {
            let patch = p[0][3] as usize;
            if p.iter().any(|v| v[3] as usize != patch) || patch >= PATCHES || patch < last {
                return Err(format!("a primitive out of patch order at patch {patch}"));
            }
            last = patch;
            out[patch].push(p.to_vec());
        }
        Ok(out)
    };
    let (a, b) = (
        patches(reference)?,
        patches(ours).map_err(|e| format!("ours: {e}"))?,
    );
    let mut leading = 0;
    for (p, (x, y)) in a.iter().zip(&b).enumerate() {
        let mut xs: Vec<Vec<Vertex>> = x.iter().map(|q| turned(q)).collect();
        let mut ys: Vec<Vec<Vertex>> = y.iter().map(|q| turned(q)).collect();
        xs.sort();
        ys.sort();
        if xs != ys {
            let show = |q: &Vec<Vertex>| -> String {
                q.iter()
                    .map(|v| {
                        format!(
                            "({:?}, {:?}, {:?})",
                            f32::from_bits(v[0]),
                            f32::from_bits(v[1]),
                            f32::from_bits(v[2])
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let missing: Vec<String> = xs
                .iter()
                .filter(|q| ys.binary_search(q).is_err())
                .map(show)
                .collect();
            let extra: Vec<String> = ys
                .iter()
                .filter(|q| xs.binary_search(q).is_err())
                .map(show)
                .collect();
            return Err(format!(
                "patch {p}: direct3d {} primitives, ours {}; direct3d's not ours {missing:?}, ours not direct3d's {extra:?}",
                x.len(),
                y.len()
            ));
        }
        let mut exact: Vec<&Vec<Vertex>> = y.iter().collect();
        exact.sort();
        leading += x.iter().filter(|q| exact.binary_search(q).is_ok()).count();
    }
    Ok(format!(
        "every patch's primitives equal, {leading} of {} starting at the same vertex, order {}",
        reference.len() / per,
        if reference == ours {
            "the same"
        } else {
            "another"
        }
    ))
}

#[test]
fn tessellator_matches_direct3d() {
    // On Vulkan: fxc, which wgpu's Direct3D 12 path compiles with here, fails on this program with
    // an internal error ("argument pulled into unrelated predicate").
    let mut backend = webgpu_on(wgpu::Backends::VULKAN);
    let mut tessellator = Tessellator::new(&mut backend).unwrap();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let seed: u64 = std::env::var("WCONTAINERS_TESSELLATION_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_TESSELLATION_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let partitionings = ["integer", "pow2", "odd", "even"];
    let mut topologies = Vec::new();
    for (domain, name) in [(0u8, "tri"), (1, "quad"), (2, "line")] {
        let outputs: &[(u8, &str)] = match domain {
            2 => &[(0, "point"), (1, "line")],
            _ => &[(0, "point"), (2, "cw"), (3, "ccw")],
        };
        for (partitioning, mode) in partitionings.iter().enumerate() {
            for &(output, out) in outputs {
                topologies.push((
                    Tessellation {
                        domain,
                        partitioning: partitioning as u8,
                        output,
                        max_factor: 64.0,
                    },
                    format!("hs_{name}_{mode}_{out}"),
                    format!("ds_{name}"),
                ));
            }
        }
    }
    let mut failures = Vec::new();
    let mut compared = 0;
    for seed in seed..seed + seeds {
        for (i, (t, hs, ds)) in topologies.iter().enumerate() {
            let mut rng = fastrand::Rng::with_seed(seed * 1000 + i as u64);
            let factors = factors(&mut rng);
            let per = match t.output {
                0 => 1,
                1 => 2,
                _ => 3,
            };
            let bound = PATCHES * tess::patch_bounds(t.max_factor).1 as usize;
            let (reference, written) = direct3d(&gpu, hs, ds, bound, &factors);
            let reference = &reference[..written as usize * per];
            let streamed = ours(&mut backend, &mut tessellator, t, &factors);
            compared += 1;
            let name = format!("seed {seed} {hs}");
            match compare(reference, &streamed, per) {
                Ok(note) => eprintln!("{name}: {} vertices; {note}", reference.len()),
                Err(why) => {
                    eprintln!("{name}: {why}; factors {factors:?}");
                    failures.push(name);
                }
            }
        }
    }
    eprintln!("{compared} comparisons, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}
