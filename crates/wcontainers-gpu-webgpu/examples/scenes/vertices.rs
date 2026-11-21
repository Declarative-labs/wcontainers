//! Vertex formats WebGPU lacks, read through shadow streams (plan 5.7, `VertexFormatConverted`):
//! every one of them, with a float beside it in the same stream, read by points at a stream offset
//! and compared with the values the format means; the shadow is kept while its source is unwritten
//! and made again after a write. Shared by tests/vertices.rs and the browser page.

use crate::draw;
use half::f16;
use wcontainers_gpu::{
    Backend, BufferUsage, ColorAttachment, DeviceStatus, DrawCall, DrawRange, Extent, Format,
    LoadOp, Origin, PassDesc, ReadSource, SlowPath, StepMode, StoreOp, TextureRegion, TextureUsage,
    Topology, VertexBinding, VertexFormat, WriteMode,
};
use wcontainers_gpu_webgpu::WebGpu;

type Tex = <WebGpu as Backend>::Texture;

/// Points, one per pixel, carrying the stream's two attributes into two targets.
const POINTS_WGSL: &str = r#"
struct O {
    @builtin(position) p: vec4f,
    @location(0) @interpolate(flat) v: vec4<T>,
    @location(1) @interpolate(flat) f: f32,
}
struct Out { @location(0) v: vec4<T>, @location(1) f: f32 }
@vertex fn vs(@builtin(vertex_index) i: u32, @location(0) f: f32, @location(1) v: vec4<T>) -> O {
    var o: O;
    o.p = vec4f((f32(i) + 0.5) / f32(N) * 2.0 - 1.0, 0.0, 0.0, 1.0);
    o.v = v;
    o.f = f;
    return o;
}
@fragment fn ps(i: O) -> Out { return Out(i.v, i.f); }
"#;

/// Bytes from one element to the next; the float at 0, the converted attribute at 4.
const STRIDE: usize = 12;
/// Elements before the bound offset.
const SKIPPED: usize = 2;

/// The shader's component type and the first target's format.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Float,
    Uint,
    Sint,
}

fn kind(f: VertexFormat) -> Kind {
    use VertexFormat::*;
    match f {
        Uint8x3 | Uint16x3 | Uint10_10_10_2 => Kind::Uint,
        Sint8x3 | Sint16x3 => Kind::Sint,
        _ => Kind::Float,
    }
}

/// Seeded source bytes of one attribute: 3, 6 or 4 bytes, floats kept finite.
fn attribute(rng: &mut fastrand::Rng, f: VertexFormat) -> Vec<u8> {
    use VertexFormat::*;
    match f {
        Uint8x3 | Sint8x3 | Unorm8x3 | Snorm8x3 => (0..3).map(|_| rng.u8(..)).collect(),
        Float16x3 => (0..3)
            .flat_map(|_| {
                let h = rng.u16(..) & !(1 << 14) | (u16::from(rng.bool()) << 14);
                // An exponent of 31 is infinity or NaN; clear one of its bits.
                let h = if (h >> 10) & 0x1f == 0x1f {
                    h & !(1 << 10)
                } else {
                    h
                };
                h.to_le_bytes()
            })
            .collect(),
        Float11_11_10 => {
            let field =
                |rng: &mut fastrand::Rng, m: u32| (rng.u32(0..31) << m) | rng.u32(0..1 << m);
            let v = field(rng, 6) | (field(rng, 6) << 11) | (field(rng, 5) << 22);
            v.to_le_bytes().to_vec()
        }
        Uint10_10_10_2 => rng.u32(..).to_le_bytes().to_vec(),
        _ => (0..6).map(|_| rng.u8(..)).collect(),
    }
}

/// An unsigned float of `m` mantissa bits over a 5-bit exponent.
fn small_float(v: u32, m: u32) -> f32 {
    let (e, f) = ((v >> m) & 0x1f, v & ((1 << m) - 1));
    let f = f as f32 / (1u32 << m) as f32;
    if e == 0 {
        f * 2f32.powi(-14)
    } else {
        (1.0 + f) * 2f32.powi(e as i32 - 15)
    }
}

/// The four components the attribute's bytes mean, as the first target holds them.
fn meaning(f: VertexFormat, a: &[u8]) -> [u32; 4] {
    use VertexFormat::*;
    let h = |i: usize| u16::from_le_bytes([a[2 * i], a[2 * i + 1]]);
    let w = u32::from_le_bytes([a[0], a[1], a[2], *a.get(3).unwrap_or(&0)]);
    // Three components and the fourth one a three-component format reads as.
    let ints = |c: [i64; 3]| [c[0] as u32, c[1] as u32, c[2] as u32, 1];
    let floats = |c: [f32; 3]| [c[0], c[1], c[2], 1.0].map(f32::to_bits);
    match f {
        Uint8x3 => ints([0, 1, 2].map(|i| i64::from(a[i]))),
        Sint8x3 => ints([0, 1, 2].map(|i| i64::from(a[i] as i8))),
        Unorm8x3 => floats([0, 1, 2].map(|i| f32::from(a[i]) / 255.0)),
        Snorm8x3 => floats([0, 1, 2].map(|i| (f32::from(a[i] as i8) / 127.0).max(-1.0))),
        Uint16x3 => ints([0, 1, 2].map(|i| i64::from(h(i)))),
        Sint16x3 => ints([0, 1, 2].map(|i| i64::from(h(i) as i16))),
        Unorm16x3 => floats([0, 1, 2].map(|i| f32::from(h(i)) / 65535.0)),
        Snorm16x3 => floats([0, 1, 2].map(|i| (f32::from(h(i) as i16) / 32767.0).max(-1.0))),
        Float16x3 => floats([0, 1, 2].map(|i| f16::from_bits(h(i)).to_f32())),
        Uint10_10_10_2 => [w & 0x3ff, (w >> 10) & 0x3ff, (w >> 20) & 0x3ff, w >> 30],
        Float11_11_10 => floats([
            small_float(w, 6),
            small_float(w >> 11, 6),
            small_float(w >> 22, 5),
        ]),
        _ => unreachable!(),
    }
}

/// The stream's bytes: `SKIPPED` elements of noise, then one element per vertex, then a partial
/// element's noise.
fn stream(rng: &mut fastrand::Rng, f: VertexFormat, n: usize) -> (Vec<u8>, Vec<([u32; 4], f32)>) {
    let mut bytes: Vec<u8> = (0..SKIPPED * STRIDE).map(|_| rng.u8(..)).collect();
    let mut want = Vec::new();
    for i in 0..n {
        let value = i as f32 * 0.25 - 3.0;
        let a = attribute(rng, f);
        let mut element = vec![0; STRIDE];
        element[..4].copy_from_slice(&value.to_le_bytes());
        element[4..4 + a.len()].copy_from_slice(&a);
        for b in &mut element[4 + a.len()..] {
            *b = rng.u8(..);
        }
        bytes.extend(&element);
        want.push((meaning(f, &a), value));
    }
    bytes.extend((0..5).map(|_| rng.u8(..)));
    (bytes, want)
}

fn read(b: &mut WebGpu, texture: &Tex, width: u32, texel: usize) -> Vec<u8> {
    let rb = b
        .read_back(&ReadSource::Texture {
            region: TextureRegion {
                texture,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width,
                height: 1,
                depth_or_layers: 1,
            },
        })
        .unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut out = vec![0; width as usize * texel];
    b.finish_read(rb, &mut out);
    out
}

/// Whether the first target's texel is what the format means: exact for integers and floats made
/// from 16 or fewer bits, within a rounding of the division for normalised ones.
fn close(f: VertexFormat, got: [u32; 4], want: [u32; 4]) -> bool {
    use VertexFormat::*;
    match f {
        Unorm8x3 | Snorm8x3 | Unorm16x3 | Snorm16x3 => got
            .iter()
            .zip(want)
            .all(|(g, w)| (f32::from_bits(*g) - f32::from_bits(w)).abs() <= 1e-6),
        _ => got == want,
    }
}

/// Every vertex format WebGPU lacks, `n` seeded vertices each (`seed`), drawn for three frames:
/// panics at the first value read wrong or shadow made other than once and again after a write.
pub fn lacking_vertex_formats(b: &mut WebGpu, seed: u64, n: u32) {
    use VertexFormat::*;
    let mut rng = fastrand::Rng::with_seed(seed);
    for f in [
        Uint8x3,
        Sint8x3,
        Unorm8x3,
        Snorm8x3,
        Uint16x3,
        Sint16x3,
        Unorm16x3,
        Snorm16x3,
        Float16x3,
        Uint10_10_10_2,
        Float11_11_10,
    ] {
        let (ty, target) = match kind(f) {
            Kind::Float => ("f32", Format::Rgba32Float),
            Kind::Uint => ("u32", Format::Rgba32Uint),
            Kind::Sint => ("i32", Format::Rgba32Sint),
        };
        let source = POINTS_WGSL
            .replace("vec4<T>", &format!("vec4<{ty}>"))
            .replace("f32(N)", &format!("{n}.0"));
        let program = draw::program(b, &source).unwrap();
        let blend = b.create_blend(&draw::opaque()).unwrap();
        let depth_stencil = b.create_depth_stencil(&draw::no_depth()).unwrap();
        let raster = b.create_raster(&draw::solid()).unwrap();
        let layout = b
            .create_vertex_layout(&draw::layout(
                &[(STRIDE as u32, StepMode::Vertex)],
                &[(0, 0, Float32x1), (0, 4, f)],
            ))
            .unwrap();
        let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
        let (values, values_view) = draw::texture_2d(b, target, n, 1, usage).unwrap();
        let (floats, floats_view) = draw::texture_2d(b, Format::R32Float, n, 1, usage).unwrap();
        let before = b.slow_paths().count(SlowPath::VertexFormatConverted);
        let (bytes, mut want) = stream(&mut rng, f, n as usize);
        let vb = draw::buffer(b, &bytes, BufferUsage::VERTEX).unwrap();
        // Three frames: the first makes the shadow, the second keeps it, the third follows a write.
        for frame in 0..3 {
            if frame == 2 {
                let (bytes, again) = stream(&mut rng, f, n as usize);
                b.write_buffer(&vb, 0, &bytes, WriteMode::Ordered);
                want = again;
            }
            let mut pass = PassDesc {
                colors: Default::default(),
                depth: None,
                empty: None,
                occlusion: None,
            };
            for (i, view) in [&values_view, &floats_view].into_iter().enumerate() {
                pass.colors[i] = Some(ColorAttachment {
                    view,
                    depth_slice: 0,
                    resolve: None,
                    load: LoadOp::Clear([0; 4]),
                    store: StoreOp::Store,
                });
            }
            b.begin_pass(&pass);
            let viewport = [wcontainers_gpu::Viewport {
                x: 0,
                y: 0,
                width: (n as f32).to_bits(),
                height: 1.0f32.to_bits(),
                min_depth: 0,
                max_depth: 1.0f32.to_bits(),
            }];
            let streams = [Some(VertexBinding {
                buffer: &vb,
                offset: (SKIPPED * STRIDE) as u64,
            })];
            b.draw(&DrawCall {
                program: &program,
                blend: &blend,
                depth_stencil: &depth_stencil,
                raster: &raster,
                vertex_layout: &layout,
                bindings: [None; 4],
                vertex_buffers: &streams,
                index: None,
                topology: Topology::PointList,
                restart: false,
                viewports: &viewport,
                scissors: &[],
                blend_constant: [0; 4],
                stencil_reference: [0; 2],
                immediates: &[],
                range: DrawRange::Vertices {
                    first: 0,
                    count: n,
                    first_instance: 0,
                    instances: 1,
                },
            });
            b.end_pass();
            let got = read(b, &values, n, 16);
            let got_floats = read(b, &floats, n, 4);
            assert_eq!(b.status(), DeviceStatus::Ok, "{f:?}");
            for (i, (w, wf)) in want.iter().enumerate() {
                let g: [u32; 4] = std::array::from_fn(|c| {
                    u32::from_le_bytes(got[i * 16 + c * 4..][..4].try_into().unwrap())
                });
                let gf = f32::from_le_bytes(got_floats[i * 4..][..4].try_into().unwrap());
                assert!(
                    close(f, g, *w),
                    "{f:?} frame {frame} vertex {i}: read {g:x?}, means {w:x?}"
                );
                assert_eq!(
                    gf, *wf,
                    "{f:?} frame {frame} vertex {i}: the float beside it"
                );
            }
            let made = b.slow_paths().count(SlowPath::VertexFormatConverted) - before;
            // One shadow made (splitting the pass) at the first draw and one after the write.
            assert_eq!(made, [1, 1, 2][frame], "{f:?} frame {frame}");
        }
    }
}

/// Points carrying one four-component float attribute into an `rgba32float` target.
const ATTRIBUTE_WGSL: &str = r#"
struct O { @builtin(position) p: vec4f, @location(0) @interpolate(flat) v: vec4f }
@vertex fn vs(@builtin(vertex_index) i: u32, @location(0) v: vec4f) -> O {
    var o: O;
    o.p = vec4f((f32(i) + 0.5) / f32(N) * 2.0 - 1.0, 0.0, 0.0, 1.0);
    o.v = v;
    return o;
}
@fragment fn ps(i: O) -> @location(0) vec4f { return i.v; }
"#;

/// The attribute each 4-byte element of `bytes` gives a program as `VertexFormat::Bgra8Unorm`
/// (Direct3D's `B8G8R8A8_UNORM`, and `B8G8R8X8_UNORM` with X read as written).
pub fn bgra_attributes(b: &mut WebGpu, bytes: &[u8]) -> Vec<[f32; 4]> {
    let n = (bytes.len() / 4) as u32;
    let source = ATTRIBUTE_WGSL.replace("f32(N)", &format!("{n}.0"));
    let program = draw::program(b, &source).unwrap();
    let blend = b.create_blend(&draw::opaque()).unwrap();
    let depth_stencil = b.create_depth_stencil(&draw::no_depth()).unwrap();
    let raster = b.create_raster(&draw::solid()).unwrap();
    let layout = b
        .create_vertex_layout(&draw::layout(
            &[(4, StepMode::Vertex)],
            &[(0, 0, VertexFormat::Bgra8Unorm)],
        ))
        .unwrap();
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let (values, view) = draw::texture_2d(b, Format::Rgba32Float, n, 1, usage).unwrap();
    let vb = draw::buffer(b, bytes, BufferUsage::VERTEX).unwrap();
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear([0; 4]),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    b.draw(&DrawCall {
        program: &program,
        blend: &blend,
        depth_stencil: &depth_stencil,
        raster: &raster,
        vertex_layout: &layout,
        bindings: [None; 4],
        vertex_buffers: &[Some(VertexBinding {
            buffer: &vb,
            offset: 0,
        })],
        index: None,
        topology: Topology::PointList,
        restart: false,
        viewports: &[wcontainers_gpu::Viewport {
            x: 0,
            y: 0,
            width: (n as f32).to_bits(),
            height: 1.0f32.to_bits(),
            min_depth: 0,
            max_depth: 1.0f32.to_bits(),
        }],
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: n,
            first_instance: 0,
            instances: 1,
        },
    });
    b.end_pass();
    let got = read(b, &values, n, 16);
    assert_eq!(b.status(), DeviceStatus::Ok, "bgra vertices");
    got.as_chunks::<16>()
        .0
        .iter()
        .map(|t| std::array::from_fn(|c| f32::from_le_bytes(t[c * 4..][..4].try_into().unwrap())))
        .collect()
}

/// `n` seeded elements (`seed`) read as `Bgra8Unorm`: each component the byte it names over 255,
/// the fourth the X byte as written. One line, or the first value read wrong.
pub fn bgra_vertices(b: &mut WebGpu, seed: u64, n: u32) -> Result<String, String> {
    let mut rng = fastrand::Rng::with_seed(seed);
    let bytes: Vec<u8> = (0..n * 4).map(|_| rng.u8(..)).collect();
    let got = bgra_attributes(b, &bytes);
    for (i, (g, e)) in got.iter().zip(bytes.as_chunks::<4>().0).enumerate() {
        let want = [e[2], e[1], e[0], e[3]].map(|v| f32::from(v) / 255.0);
        if g.iter().zip(want).any(|(g, w)| (g - w).abs() > 1e-6) {
            return Err(format!("bgra vertex {i}: read {g:?}, means {want:?}"));
        }
    }
    Ok(format!(
        "bgra vertices: seed {seed}, {n} vertices, all as they mean"
    ))
}
