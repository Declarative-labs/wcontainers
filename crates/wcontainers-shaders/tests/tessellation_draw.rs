//! The stage helper's tessellator on the WebGPU back end against real Direct3D 11's on the same GPU:
//! patches of seeded factors for every domain, partitioning and output topology, Direct3D's domain
//! shader output streamed (where each point lies and which patch made it), against our tessellator's
//! points and indices read back, word for word and in order; and the same streamed through the whole
//! stage helper (the domain stage as compute, with and without a geometry shader after it).
//! `WCONTAINERS_TESSELLATION_SEED` (default 1) and `WCONTAINERS_TESSELLATION_CASES` seeds from it
//! (default 4); every topology runs per seed. The tests drawing converted programs draw each as fxc
//! builds it and as dxc does.

#![cfg(windows)]

mod common;

use common::*;
use wcontainers_gpu::{Backend, BufferRange, BufferUsage, ReadSource, RingKind};
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

/// Real Direct3D 11's streamed domain shader output (or the output of geometry shader `gs` after it)
/// for one draw of the patches, of `points` control points each, and how many primitives it wrote.
fn direct3d(
    gpu: &Gpu,
    hs: &str,
    ds: &str,
    gs: Option<&str>,
    points: u32,
    bound: usize,
    factors: &[[f32; 8]],
) -> (Vec<Vertex>, u64) {
    let vs = gpu.vs(&bytecode("vs_patch.dxbc")).unwrap();
    let hull = gpu.hs(&bytecode(&format!("{hs}.dxbc"))).unwrap();
    let domain_code = bytecode(&format!("{ds}.dxbc"));
    let domain = gpu.ds(&domain_code).unwrap();
    let streamed_code = match gs {
        Some(gs) => bytecode(&format!("{gs}.dxbc")),
        None => domain_code.clone(),
    };
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
            &streamed_code,
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
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY(
            D3D_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST.0 + points as i32 - 1,
        ));
        c.VSSetShader(&vs, None);
        c.HSSetShader(&hull, None);
        c.HSSetConstantBuffers(0, Some(&[Some(constants)]));
        c.DSSetShader(&domain, None);
        c.GSSetShader(&streamed, None);
        c.PSSetShader(None, None);
        let targets = [Some(target.clone())];
        c.SOSetTargets(1, Some(targets.as_ptr()), Some([0u32].as_ptr()));
        c.Begin(&query);
        c.Draw(factors.len() as u32 * points, 0);
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
    let header = tess::INDEX_HEADER as usize;
    let indices = read(
        b,
        tessellator.indices().unwrap(),
        header + counts[0] as usize,
    );
    assert_eq!(indices[0], counts[0], "the index header holds the total");
    assert_eq!(
        counts[2] as usize, header,
        "the draw starts past the header"
    );
    let indices = &indices[header..];
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
    let mut backend = webgpu();
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
            let (reference, written) = direct3d(&gpu, hs, ds, None, 1, bound, &factors);
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

/// The rig's Direct3D 11 tessellation scene's patches (its data, which the rig keeps private): hull
/// and domain shader names, control points, and whether the patch is drawn solid (the others as
/// lines, so every new vertex shows).
fn scene_patches() -> Vec<(&'static str, &'static str, Vec<[f32; 7]>, bool)> {
    let point = |x: f32, y: f32, k: u32| {
        let [r, g, b] = wcontainers_graphics_test::d3d11::scenes::hue(k, 4);
        [x, y, 0.5, r, g, b, 1.0]
    };
    let tri = vec![
        point(0.0, 0.85, 0),
        point(0.85, -0.75, 1),
        point(-0.85, -0.75, 2),
    ];
    let quad = vec![
        point(-0.85, 0.85, 0),
        point(0.85, 0.85, 1),
        point(0.85, -0.85, 2),
        point(-0.85, -0.85, 3),
    ];
    let line = vec![point(-0.9, 0.0, 0), point(0.9, 0.0, 2)];
    vec![
        ("hs_tri", "ds_tri", tri.clone(), false),
        ("hs_quad_odd", "ds_quad", quad.clone(), false),
        ("hs_quad_even", "ds_quad", quad.clone(), false),
        ("hs_quad_pow2", "ds_quad", quad.clone(), false),
        ("hs_quad_integer", "ds_quad", quad, false),
        ("hs_line", "ds_line", line, false),
        ("hs_tri", "ds_tri", tri, true),
    ]
}

/// The scene's factors, its constant buffer at b0 of the hull stage.
const SCENE_FACTORS: [f32; 8] = [3.3, 4.7, 2.2, 5.9, 3.6, 2.4, 0.0, 0.0];
const SCENE_WIDTH: u32 = 256;
const SCENE_HEIGHT: u32 = 128;

/// One set per number from `bindings`' entries.
fn sets<B: Backend>(
    b: &mut B,
    entries: Vec<(u32, wcontainers_gpu::BindingEntry<'_, B>)>,
) -> [Option<B::BindingSet>; 4] {
    let mut by_set: [Vec<_>; 4] = Default::default();
    for (set, e) in entries {
        by_set[set as usize].push(e);
    }
    by_set.map(|e| (!e.is_empty()).then(|| b.create_binding_set(&e).unwrap()))
}

/// The scene drawn through the stage helper: the vertex and hull stages as compute, the tessellator,
/// then the domain program's indexed indirect draw of its points.
fn scene_ours<B: Backend>(b: &mut B, all_solid: bool, route: Route) -> Vec<u8> {
    use wcontainers_gpu::*;
    use wcontainers_shaders::convert;
    use wcontainers_shaders::stage::{InputTopology, StageDraw, Stages};
    let ext = route.extension();
    let rig = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../wcontainers-graphics-test/shaders")
        .join(ext);
    let read = |name: &str| std::fs::read(rig.join(format!("tessellation.{name}.{ext}"))).unwrap();
    let plain = VariantKey::default();
    let ps = convert(route.source(&read("ps")), &plain, None).unwrap();
    let ds_key = VariantKey {
        output_interpolation: ps
            .reflection
            .inputs
            .iter()
            .filter(|v| v.system.is_none() && v.interpolation != Interpolation::Perspective)
            .map(|v| (v.location, v.interpolation))
            .collect(),
        ..Default::default()
    };
    let vs_key = VariantKey {
        pulled: vec![(0, VertexFormat::Float32x3), (1, VertexFormat::Float32x4)],
        ..Default::default()
    };
    let vs = convert(route.source(&read("vs")), &vs_key, None).unwrap();
    let fragment = b.create_shader(&ps.modules[0]).unwrap();
    let constants: Vec<u8> = SCENE_FACTORS.iter().flat_map(|f| f.to_le_bytes()).collect();
    let constant_buffer = buffer(b, &constants, BufferUsage::UNIFORM);
    let mut helpers = vec![0u8; vs.reflection.helper_size as usize];
    for h in &vs.reflection.helpers {
        let words = match h.value {
            HelperValue::PulledStream { input } => [if input == 0 { 0u32 } else { 12 }, 28, 0, 0],
            _ => panic!("{h:?}"),
        };
        let at = h.offset as usize;
        for (i, w) in words.iter().enumerate() {
            helpers[at + 4 * i..at + 4 * i + 4].copy_from_slice(&w.to_le_bytes());
        }
    }
    let helper_buffer = buffer(b, &helpers, BufferUsage::UNIFORM);
    let blend = b
        .create_blend(&BlendDesc {
            targets: [RenderTargetBlend {
                blend: None,
                writes: ColorWrites::all(),
            }; 8],
            same_for_all: true,
            alpha_to_coverage: false,
            logic_op: None,
        })
        .unwrap();
    let depth_stencil = b
        .create_depth_stencil(&DepthStencilDesc {
            depth_compare: None,
            depth_write: false,
            stencil: None,
            stencil_read_mask: [0xff; 2],
            stencil_write_mask: [0xff; 2],
            depth_bounds: None,
        })
        .unwrap();
    let raster = |b: &mut B, fill| {
        b.create_raster(&RasterDesc {
            fill,
            cull: CullMode::None,
            front: FrontFace::Clockwise,
            depth_bias: 0,
            depth_bias_slope: 0,
            depth_bias_clamp: 0,
            depth_clip: true,
            scissor: false,
            line_width: 1.0f32.to_bits(),
            provoking: ProvokingVertex::First,
            depth_minus_one: false,
            sample_mask: !0,
        })
        .unwrap()
    };
    let (solid, lines) = (raster(b, FillMode::Solid), raster(b, FillMode::Line));
    let no_streams = b
        .create_vertex_layout(&VertexLayoutDesc {
            streams: [None; MAX_VERTEX_STREAMS],
            attributes: [None; MAX_VERTEX_STREAMS],
        })
        .unwrap();
    let target = b
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format: Format::Rgba8Unorm,
            width: SCENE_WIDTH,
            height: SCENE_HEIGHT,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::COPY_SRC,
            cube: false,
            reinterpret: false,
        })
        .unwrap();
    let target_view = b
        .create_view(
            &target,
            &TextureViewDesc {
                format: Format::Rgba8Unorm,
                dimension: ViewDimension::D2,
                base_mip: 0,
                mip_count: 1,
                base_layer: 0,
                layer_count: 1,
                stencil: false,
            },
        )
        .unwrap();

    for (i, (hs, ds, points, solid_fill)) in scene_patches().into_iter().enumerate() {
        let hs = convert(route.source(&read(hs)), &plain, None).unwrap();
        let ds = convert(route.source(&read(ds)), &ds_key, None).unwrap();
        let mut stages = Stages::tessellated(b, &vs, &hs, &ds).unwrap();
        let flat: Vec<f32> = points.iter().flatten().copied().collect();
        let vertex_buffer = buffer(b, bytes_of(&flat), BufferUsage::STORAGE);
        let size = (flat.len() * 4) as u64;
        let whole = |buffer| BufferRange {
            buffer,
            offset: 0,
            size,
        };
        let mut vertex_entries = Vec::new();
        for r in &vs.reflection.bindings {
            assert!(
                matches!(r.added, Some(AddedBinding::PulledStream(_))),
                "{r:?}"
            );
            vertex_entries.push((
                r.set,
                BindingEntry {
                    binding: r.binding,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::StorageBuffer {
                        range: whole(&vertex_buffer),
                        writable: false,
                    },
                },
            ));
        }
        vertex_entries.push((
            HELPER_SET,
            BindingEntry {
                binding: helper_binding(ShaderStage::Compute),
                stages: StageSet::COMPUTE,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &helper_buffer,
                    offset: 0,
                    size: helpers.len() as u64,
                }),
            },
        ));
        let vertex_sets = sets(b, vertex_entries);
        assert!(
            hs.reflection.helpers.is_empty(),
            "{:?}",
            hs.reflection.helpers
        );
        let hull_entries = hs
            .reflection
            .bindings
            .iter()
            .map(|r| {
                assert_eq!((r.kind, r.register), (SlotKind::Uniform, 0), "{r:?}");
                (
                    r.set,
                    BindingEntry {
                        binding: r.binding,
                        stages: StageSet::COMPUTE,
                        resource: BindingResource::UniformBuffer(BufferRange {
                            buffer: &constant_buffer,
                            offset: 0,
                            size: constants.len() as u64,
                        }),
                    },
                )
            })
            .collect();
        let hull_sets = sets(b, hull_entries);
        let n = points.len() as u32;
        let last = stages
            .run(
                b,
                &StageDraw {
                    vertex_sets: vertex_sets.each_ref().map(Option::as_ref),
                    geometry_sets: hull_sets.each_ref().map(Option::as_ref),
                    index: None,
                    topology: InputTopology::PatchList(n as u8),
                    range: DrawRange::Vertices {
                        first: 0,
                        count: n,
                        first_instance: 0,
                        instances: 1,
                    },
                    stream_output: None,
                },
            )
            .unwrap()
            .expect("patches");
        let t = last.tessellated.as_ref().expect("a tessellated draw");

        // The domain program reads the patches and points in its helper set.
        assert!(
            ds.reflection.bindings.is_empty(),
            "{:?}",
            ds.reflection.bindings
        );
        assert!(
            ds.reflection.helpers.is_empty(),
            "{:?}",
            ds.reflection.helpers
        );
        fn read_only<B: Backend>(
            binding: u32,
            range: &BufferRange<B::Buffer>,
        ) -> BindingEntry<'_, B> {
            BindingEntry {
                binding,
                stages: StageSet::VERTEX,
                resource: BindingResource::StorageBuffer {
                    range: BufferRange {
                        buffer: &range.buffer,
                        offset: range.offset,
                        size: range.size,
                    },
                    writable: false,
                },
            }
        }
        let domain_set = b
            .create_binding_set(&[
                read_only(DOMAIN_PATCHES_BINDING, &t.patches),
                read_only(DOMAIN_POINTS_BINDING, &t.points),
            ])
            .unwrap();
        let vertex = b.create_shader(&ds.modules[0]).unwrap();
        let program = b
            .create_program(&Program::Render {
                vertex: &vertex,
                fragment: Some(&fragment),
            })
            .unwrap();
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view: &target_view,
            depth_slice: 0,
            resolve: None,
            load: match i {
                0 => LoadOp::Clear(wcontainers_graphics_test::scene::CLEAR.map(f32::to_bits)),
                _ => LoadOp::Load,
            },
            store: StoreOp::Store,
        });
        let mut bindings = [None; 4];
        bindings[HELPER_SET as usize] = Some(&domain_set);
        let cell = i as u32;
        b.begin_pass(&pass);
        b.draw(&DrawCall {
            program: &program,
            blend: &blend,
            depth_stencil: &depth_stencil,
            raster: if solid_fill || all_solid {
                &solid
            } else {
                &lines
            },
            vertex_layout: &no_streams,
            bindings,
            vertex_buffers: &[],
            index: Some(IndexBinding {
                buffer: &last.buffer,
                offset: 0,
                format: IndexFormat::U32,
            }),
            topology: last.topology,
            restart: false,
            viewports: &[Viewport {
                x: ((cell % 4) as f32 * 64.0).to_bits(),
                y: ((cell / 4) as f32 * 64.0).to_bits(),
                width: 64.0f32.to_bits(),
                height: 64.0f32.to_bits(),
                min_depth: 0.0f32.to_bits(),
                max_depth: 1.0f32.to_bits(),
            }],
            scissors: &[],
            blend_constant: [0; 4],
            stencil_reference: [0; 2],
            immediates: &[],
            range: DrawRange::IndexedIndirect {
                args: &t.arguments,
                offset: 0,
            },
        });
        b.end_pass();
        stages.destroy(b);
    }
    let mut out = vec![0; (SCENE_WIDTH * SCENE_HEIGHT * 4) as usize];
    b.read_back_now(
        &ReadSource::Texture {
            region: TextureRegion {
                texture: &target,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width: SCENE_WIDTH,
                height: SCENE_HEIGHT,
                depth_or_layers: 1,
            },
        },
        &mut out,
    )
    .unwrap();
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert!(
        b.slow_paths().count(SlowPath::TessellationAsCompute) > 0,
        "the slow path is counted"
    );
    out
}

/// The rig's tessellation scene with every patch drawn solid, by real Direct3D 11.
fn direct3d_solid_scene(gpu: &Gpu) -> Vec<u8> {
    let rig = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../wcontainers-graphics-test/shaders/dxbc");
    let read = |name: &str| std::fs::read(rig.join(format!("tessellation.{name}.dxbc"))).unwrap();
    let (target, rtv) = gpu
        .target(
            SCENE_WIDTH,
            SCENE_HEIGHT,
            windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM,
        )
        .unwrap();
    gpu.begin(&rtv, None, SCENE_WIDTH, SCENE_HEIGHT);
    gpu.program(&read("vs"), &read("ps"), &d3d11::scenes::colored_layout())
        .unwrap();
    let factors: Vec<u8> = SCENE_FACTORS.iter().flat_map(|f| f.to_le_bytes()).collect();
    let constants = gpu.immutable(&factors, D3D11_BIND_CONSTANT_BUFFER).unwrap();
    let solid = gpu.rasterizer(&d3d11::solid()).unwrap();
    let c = &gpu.context;
    unsafe {
        c.HSSetConstantBuffers(0, Some(&[Some(constants)]));
        c.RSSetState(&solid);
    }
    for (i, (hs, ds, points, _)) in scene_patches().into_iter().enumerate() {
        let flat: Vec<f32> = points.iter().flatten().copied().collect();
        let _vertices = gpu.vertices(0, bytes_of(&flat), 28).unwrap();
        let topology = match points.len() {
            2 => D3D_PRIMITIVE_TOPOLOGY_2_CONTROL_POINT_PATCHLIST,
            3 => D3D_PRIMITIVE_TOPOLOGY_3_CONTROL_POINT_PATCHLIST,
            _ => D3D_PRIMITIVE_TOPOLOGY_4_CONTROL_POINT_PATCHLIST,
        };
        unsafe {
            c.RSSetViewports(Some(&[d3d11::cell(i as u32, 4, 64.0)]));
            c.HSSetShader(&gpu.hs(&read(hs)).unwrap(), None);
            c.DSSetShader(&gpu.ds(&read(ds)).unwrap(), None);
            c.IASetPrimitiveTopology(topology);
            c.Draw(points.len() as u32, 0);
        }
    }
    unsafe {
        c.HSSetShader(None, None);
        c.DSSetShader(None, None);
    }
    gpu.read(&target, 0, 0).unwrap().to_rgba8().into_raw()
}

/// Per 64-texel cell of the scene: texels that differ, texels Direct3D drew (not the clear colour)
/// that ours did not, texels only ours drew, and texels both drew more than 2 steps apart.
fn by_cell(reference: &[u8], ours: &[u8]) -> Vec<[usize; 4]> {
    let clear = &reference[..4];
    let mut cells = vec![[0; 4]; 8];
    for (i, (p, q)) in reference.chunks(4).zip(ours.chunks(4)).enumerate() {
        let (x, y) = (i as u32 % SCENE_WIDTH, i as u32 / SCENE_WIDTH);
        let c = &mut cells[(y / 64 * 4 + x / 64) as usize];
        let d = p.iter().zip(q).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
        if d == 0 {
            continue;
        }
        c[0] += 1;
        match (p == clear, q == clear) {
            (false, true) => c[1] += 1,
            (true, false) => c[2] += 1,
            _ if d > 2 => c[3] += 1,
            _ => {}
        }
    }
    cells
}

/// The rig's tessellation scene (triangle, quad and isoline patches, each partitioning) with every
/// patch drawn solid, through the stage helper, against real Direct3D 11 drawing the same: every
/// texel of the surfaces equal; the isoline patch, being lines, as the scene's lines below.
#[test]
fn tessellated_patches_draw_as_direct3d_11_does() {
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let reference = direct3d_solid_scene(&gpu);
    keep(
        "tessellation-solid-direct3d",
        &reference,
        wcontainers_gpu::Format::Rgba8Unorm,
    );
    let mut backend = webgpu();
    for route in Route::BOTH {
        let ours = scene_ours(&mut backend, true, route);
        keep(
            &format!("tessellation-solid-ours-{}", route.extension()),
            &ours,
            wcontainers_gpu::Format::Rgba8Unorm,
        );
        let cells = by_cell(&reference, &ours);
        eprintln!(
            "tessellated patches, solid, {route:?}, per cell (differ, missed, only ours, apart): {cells:?}"
        );
        for (i, c) in cells.iter().enumerate() {
            match i {
                // The isoline patch: lines, drawn by each API's line rule.
                5 => assert_eq!((c[1], c[3]), (0, 0), "{route:?} cell {i}"),
                _ => assert_eq!(c[0], 0, "{route:?} cell {i}"),
            }
        }
    }
}

/// The rig's tessellation scene as it is (the patches drawn as lines but the last) through the stage
/// helper, against real Direct3D 11 running the scene itself. Lines drawn from filled polygons follow
/// each API's line rule (the back end's), so this one asks only that every texel
/// Direct3D drew is drawn, and that the solid patch is equal; the rest is printed.
#[test]
fn rig_tessellation_scene_draws_as_direct3d_11_does() {
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let (_, scene) = d3d11::scenes::find("tessellation").unwrap();
    let outputs = scene(&gpu).unwrap();
    let wcontainers_graphics_test::output::Value::Picture(picture) = &outputs[0].value else {
        panic!("the scene draws a picture");
    };
    let reference = picture.to_rgba8().into_raw();
    keep(
        "tessellation-scene-direct3d",
        &reference,
        wcontainers_gpu::Format::Rgba8Unorm,
    );
    let mut backend = webgpu();
    for route in Route::BOTH {
        let ours = scene_ours(&mut backend, false, route);
        keep(
            &format!("tessellation-scene-ours-{}", route.extension()),
            &ours,
            wcontainers_gpu::Format::Rgba8Unorm,
        );
        let cells = by_cell(&reference, &ours);
        eprintln!(
            "tessellation scene, {route:?}, per cell (differ, missed, only ours, apart): {cells:?}"
        );
        for (i, c) in cells.iter().enumerate() {
            assert_eq!(c[1], 0, "{route:?} cell {i}");
        }
        // The last patch is drawn solid.
        assert_eq!(cells[6][0], 0, "{route:?}");
    }
}

/// A case's hull, domain and (if any) geometry shader, and the build they are read from.
#[derive(Clone, Copy)]
struct Staged<'a> {
    hs: &'a str,
    ds: &'a str,
    gs: Option<&'a str>,
    route: Route,
}

/// The same patches streamed through the whole stage helper: the vertex and hull stages, the
/// tessellator, the domain stage as compute and geometry shader `gs` if given, then stream output of
/// the location and the patch's number; the vertices written and the primitives the helper counted.
/// With `ring_room`, the data ring is placed first to wrap after that many parts (`place_data_ring`).
fn ours_staged<B: Backend>(
    b: &mut B,
    programs: &Staged<'_>,
    points: u32,
    bound: usize,
    factors: &[[f32; 8]],
    ring_room: Option<u64>,
) -> (Vec<Vertex>, [u32; 2]) {
    use wcontainers_gpu::*;
    use wcontainers_shaders::convert;
    use wcontainers_shaders::stage::{
        DomainSets, InputTopology, StageDraw, Stages, StreamOutput, StreamOutputEntry,
        StreamTargets,
    };
    let Staged { hs, ds, gs, route } = *programs;
    let plain = VariantKey::default();
    let read = |name: &str| bytecode(&format!("{name}.{}", route.extension()));
    let vs = convert(route.source(&read("vs_patch")), &plain, None).unwrap();
    let hull = convert(route.source(&read(hs)), &plain, None).unwrap();
    let domain = convert(route.source(&read(ds)), &plain, None).unwrap();
    let geometry = gs.map(|gs| convert(route.source(&read(gs)), &plain, None).unwrap());
    let entry = |semantic: &str, count| StreamOutputEntry {
        stream: 0,
        semantic: Some(semantic.into()),
        semantic_index: 0,
        start_component: 0,
        component_count: count,
        slot: 0,
    };
    let so = StreamOutput {
        entries: vec![entry("DOMAIN", 3), entry("PRIMITIVE", 1)],
        strides: [16, 0, 0, 0],
    };
    let mut stages =
        Stages::tessellated_with(b, &vs, &hull, &domain, geometry.as_ref(), Some(&so)).unwrap();
    for (name, c) in [("vertex", &vs), ("domain", &domain)] {
        assert!(
            c.reflection.bindings.is_empty(),
            "{name}: {:?}",
            c.reflection.bindings
        );
        assert!(
            c.reflection.helpers.is_empty(),
            "{name}: {:?}",
            c.reflection.helpers
        );
    }
    let mut constants = vec![0u8; 1024];
    for (p, f) in factors.iter().enumerate() {
        for (i, v) in f.iter().enumerate() {
            constants[p * 32 + i * 4..][..4].copy_from_slice(&v.to_le_bytes());
        }
    }
    let constant_buffer = buffer(b, &constants, BufferUsage::UNIFORM);
    let hull_entries: Vec<_> = hull
        .reflection
        .bindings
        .iter()
        .map(|r| {
            assert_eq!((r.kind, r.register), (SlotKind::Uniform, 0), "{r:?}");
            (
                r.set,
                BindingEntry {
                    binding: r.binding,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &constant_buffer,
                        offset: 0,
                        size: constants.len() as u64,
                    }),
                },
            )
        })
        .collect();
    let hull_sets = sets(b, hull_entries);
    let target_bytes = (bound * 16) as u64;
    let target = b
        .create_buffer(&BufferDesc {
            size: target_bytes,
            usage: BufferUsage::STORAGE | BufferUsage::COPY_SRC,
            intent: MapIntent::Rarely,
        })
        .unwrap();
    let filled = buffer(b, &[0u8; 32], BufferUsage::STORAGE | BufferUsage::COPY_SRC);
    let n = factors.len() as u32;
    let none = [None; 4];
    if let Some(parts) = ring_room {
        place_data_ring(b, parts);
    }
    stages
        .run_after_domain(
            b,
            &StageDraw {
                vertex_sets: none,
                geometry_sets: hull_sets.each_ref().map(Option::as_ref),
                index: None,
                topology: InputTopology::PatchList(points as u8),
                range: DrawRange::Vertices {
                    first: 0,
                    count: n * points,
                    first_instance: 0,
                    instances: 1,
                },
                stream_output: Some(StreamTargets {
                    buffers: [
                        Some(BufferRange {
                            buffer: &target,
                            offset: 0,
                            size: target_bytes,
                        }),
                        None,
                        None,
                        None,
                    ],
                    filled: BufferRange {
                        buffer: &filled,
                        offset: 0,
                        size: 32,
                    },
                }),
            },
            &DomainSets {
                domain_sets: none,
                geometry_sets: none,
            },
        )
        .unwrap();
    let read_words = |b: &mut B, buffer: &B::Buffer, bytes: u64| -> Vec<u32> {
        let mut out = vec![0u8; bytes as usize];
        b.read_back_now(
            &ReadSource::Buffer {
                buffer,
                offset: 0,
                size: bytes,
            },
            &mut out,
        )
        .unwrap();
        out.chunks(4)
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
            .collect()
    };
    let counts = read_words(b, &filled, 32);
    let bytes = u64::from(counts[0]);
    let words = if bytes == 0 {
        Vec::new()
    } else {
        read_words(b, &target, bytes)
    };
    assert_eq!(b.status(), DeviceStatus::Ok);
    for path in [
        SlowPath::TessellationAsCompute,
        SlowPath::StreamOutAsCompute,
    ] {
        assert!(b.slow_paths().count(path) > 0, "{path:?} is counted");
    }
    stages.destroy(b);
    let vertices = words.chunks(4).map(|v| [v[0], v[1], v[2], v[3]]).collect();
    (vertices, [counts[4], counts[5]])
}

/// Stream output after the domain stage, directly and through a geometry shader, through the whole
/// stage helper on the WebGPU back end, against real Direct3D 11 streaming the same draw: each
/// patch's primitives equal (as [`compare`] has it), and the primitives written and made as Direct3D
/// counts them. One case draws patches of three control points to a hull shader that reads one.
#[test]
fn stream_output_after_the_domain_stage_matches_direct3d() {
    let mut backend = webgpu();
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
    let mut cases = Vec::new();
    for (name, partitionings, outputs) in [
        ("tri", &["integer", "odd"][..], &["point", "cw", "ccw"][..]),
        ("quad", &["pow2", "even"], &["point", "cw"]),
        ("line", &["integer", "odd", "even"], &["point", "line"]),
    ] {
        for mode in partitionings {
            for out in outputs {
                let (per, gs) = match *out {
                    "point" => (1, "gs_tess_point"),
                    "line" => (2, "gs_tess_line"),
                    _ => (3, "gs_tess_tri"),
                };
                for gs in [None, Some(gs)] {
                    cases.push((
                        format!("hs_{name}_{mode}_{out}"),
                        format!("ds_{name}"),
                        gs,
                        per,
                        1,
                    ));
                }
            }
        }
    }
    cases.push(("hs_tri_odd_cw".into(), "ds_tri".into(), None, 3, 3));
    // Three control points in and four out, each holding the position, numbers and rows, and patch
    // constants read from both patches: the domain shader moves its location where one is not
    // what the hull gave.
    for (hs, gs, per) in [
        ("hs_values_odd_ccw", None, 3),
        ("hs_values_odd_ccw", Some("gs_tess_tri"), 3),
        ("hs_values_even_cw", None, 3),
        ("hs_values_integer_point", None, 1),
        ("hs_values_integer_point", Some("gs_tess_point"), 1),
    ] {
        cases.push((hs.into(), "ds_values".into(), gs, per, 3));
    }
    let mut failures = Vec::new();
    let mut known_cases = Vec::new();
    let mut compared = 0;
    for seed in seed..seed + seeds {
        for (i, (hs, ds, gs, per, points)) in cases.iter().enumerate() {
            let mut rng = fastrand::Rng::with_seed(seed * 1000 + i as u64);
            let factors = factors(&mut rng);
            // A geometry shader makes at most twice the primitives.
            let bound = 2 * PATCHES * tess::patch_bounds(64.0).1 as usize;
            let (reference, written) = direct3d(&gpu, hs, ds, *gs, *points, bound, &factors);
            let reference = &reference[..written as usize * per];
            for route in Route::BOTH {
                let (streamed, counted) = ours_staged(
                    &mut backend,
                    &Staged {
                        hs,
                        ds,
                        gs: *gs,
                        route,
                    },
                    *points,
                    bound,
                    &factors,
                    None,
                );
                compared += 1;
                let name = format!(
                    "seed {seed} {hs} {} {points} points {route:?}",
                    gs.unwrap_or("streamed")
                );
                let counts = if counted == [written as u32; 2] {
                    Ok(())
                } else {
                    Err(format!(
                        "primitives written and made {counted:?}, direct3d {written}"
                    ))
                };
                match (counts, compare(reference, &streamed, *per)) {
                    (Ok(()), Ok(note)) => eprintln!("{name}: {} vertices; {note}", reference.len()),
                    (counts, shape) => {
                        let why = [counts.err(), shape.err()].into_iter().flatten();
                        let why: Vec<String> = why.collect();
                        eprintln!("{name}: {}; factors {factors:?}", why.join("; "));
                        // A subnormal factor moved through the hull shader: Direct3D keeps its bits, the
                        // back end's compiled program flushes it to zero. Known when ours equals
                        // Direct3D drawing the factors flushed.
                        let flushed: Vec<[f32; 8]> = factors
                            .iter()
                            .map(|f| f.map(|v| if v.is_subnormal() { 0.0 } else { v }))
                            .collect();
                        let known = factors.iter().flatten().any(|v| v.is_subnormal()) && {
                            let (r, w) = direct3d(&gpu, hs, ds, *gs, *points, bound, &flushed);
                            counted == [w as u32; 2]
                                && compare(&r[..w as usize * per], &streamed, *per).is_ok()
                        };
                        if known {
                            eprintln!(
                                "{name}: known, equal to direct3d with subnormal factors flushed"
                            );
                            known_cases.push(name);
                        } else {
                            failures.push(name);
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "{compared} comparisons, {} failed, {} known (subnormal factors)",
        failures.len(),
        known_cases.len()
    );
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The data ring's size [`place_data_ring`] makes: larger than the ring the back end starts with,
/// so that its first write makes a ring of exactly this size.
const PLACED_RING_BYTES: u64 = 1 << 23;

/// More parts than the stage helper writes into the data ring for one draw after a domain stage
/// (each stage's values, the tessellator's three, the stream's), so the ring wraps at each in turn.
const RING_PLACES: u64 = 12;

/// Leaves the data ring `parts` parts short of its end, so that the next writes into it wrap at
/// the `parts`th part: one write fills a ring of [`PLACED_RING_BYTES`], the next refills it from
/// the start.
fn place_data_ring<B: Backend>(b: &mut B, parts: u64) {
    let align = b.caps().limits.min_uniform_offset_alignment;
    let mut offsets = [0u64];
    let full = vec![0u8; PLACED_RING_BYTES as usize];
    b.write_buffer_ring(RingKind::Data, &[&full], align, &mut offsets)
        .unwrap();
    assert_eq!(offsets, [0], "the data ring starts at the filling write");
    let short = vec![0u8; (PLACED_RING_BYTES - parts * u64::from(align)) as usize];
    b.write_buffer_ring(RingKind::Data, &[&short], align, &mut offsets)
        .unwrap();
    assert_eq!(offsets, [0], "the data ring is {PLACED_RING_BYTES} bytes");
}

/// The stage helper's values hold wherever the data ring wraps among its writes for one draw: a
/// draw streamed after the domain stage, directly and through a geometry shader, with the ring
/// placed to wrap at each of its next [`RING_PLACES`] parts in turn, against real Direct3D 11.
/// A wrap gives the ring fresh storage, so a stage whose values were written before it and bound
/// after it would read none.
#[test]
fn stage_values_hold_wherever_the_data_ring_wraps() {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let seed: u64 = std::env::var("WCONTAINERS_TESSELLATION_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let bound = 2 * PATCHES * tess::patch_bounds(64.0).1 as usize;
    let mut failures = Vec::new();
    let mut compared = 0;
    for (i, gs) in [None, Some("gs_tess_tri")].into_iter().enumerate() {
        let (hs, ds, per, points) = ("hs_tri_odd_cw", "ds_tri", 3, 1);
        // Subnormal factors are another matter (the stream test's known cases): flushed here.
        let mut rng = fastrand::Rng::with_seed(seed * 1000 + i as u64);
        let factors: Vec<[f32; 8]> = factors(&mut rng)
            .iter()
            .map(|f| f.map(|v| if v.is_subnormal() { 0.0 } else { v }))
            .collect();
        let (reference, written) = direct3d(&gpu, hs, ds, gs, points, bound, &factors);
        let reference = &reference[..written as usize * per];
        for parts in 0..=RING_PLACES {
            for route in Route::BOTH {
                let staged = Staged { hs, ds, gs, route };
                let (streamed, counted) =
                    ours_staged(&mut backend, &staged, points, bound, &factors, Some(parts));
                compared += 1;
                let name = format!(
                    "seed {seed} {hs} {} ring wrapping after {parts} parts {route:?}",
                    gs.unwrap_or("streamed")
                );
                let counts = match counted == [written as u32; 2] {
                    true => Ok(()),
                    false => Err(format!(
                        "primitives written and made {counted:?}, direct3d {written}"
                    )),
                };
                match (counts, compare(reference, &streamed, per)) {
                    (Ok(()), Ok(note)) => eprintln!("{name}: {} vertices; {note}", reference.len()),
                    (counts, shape) => {
                        let why: Vec<String> =
                            [counts.err(), shape.err()].into_iter().flatten().collect();
                        eprintln!("{name}: {}", why.join("; "));
                        failures.push(name);
                    }
                }
            }
        }
    }
    eprintln!("{compared} comparisons, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}
