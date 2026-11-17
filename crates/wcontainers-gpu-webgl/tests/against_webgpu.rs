//! The same drawings through this back end on ANGLE, with and without EXT_clip_control (both
//! window-origin routes), and through the WebGPU back end on wgpu's Direct3D 12 on the same GPU: the
//! texels read back must be equal. Windows only; ANGLE's directory as in tests/suite.rs.
#![cfg(windows)]

#[path = "../examples/common/kit.rs"]
#[allow(dead_code)]
mod kit;

use kit::Kit;
use std::path::PathBuf;
use wcontainers_gpu::*;
use wcontainers_gpu_webgl::WebGl;
use wcontainers_gpu_webgpu::WebGpu;

fn webgl(off: &[&str]) -> WebGl {
    let dir = std::env::var_os("WEBGL_ANGLE_DIR").map_or_else(
        || PathBuf::from(r"C:\wc\tools\angle-mozangle-0.7.1"),
        PathBuf::from,
    );
    let b = WebGl::on_angle(&dir, off).expect("ANGLE's OpenGL ES 3.0 context");
    assert!(b.last_log().is_empty(), "{}", b.last_log());
    b
}

fn webgpu() -> WebGpu {
    webgpu_without(wgpu::Features::empty())
}

/// The WebGPU back end on a device made without `drop`.
fn webgpu_without(drop: wgpu::Features) -> WebGpu {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::DX12;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("a Direct3D 12 adapter");
    let mut device_desc = WebGpu::device_descriptor(&adapter);
    device_desc.required_features.remove(drop);
    let (device, queue) =
        pollster::block_on(adapter.request_device(&device_desc)).expect("a device");
    WebGpu::new(&adapter, device, queue)
}

/// What one drawing gives: named parts read back.
type Parts = Vec<(String, Vec<u8>)>;

fn check(name: &str, reference: &Parts, got: &Parts, route: &str) {
    assert_eq!(reference.len(), got.len());
    for ((part, want), (_, got)) in reference.iter().zip(got) {
        let differ = want.iter().zip(got).filter(|(a, b)| a != b).count();
        assert!(
            differ == 0 && want.len() == got.len(),
            "{name}, {part}, WebGL {route}: {differ} of {} bytes differ from WebGPU's\n  webgpu {want:?}\n  webgl  {got:?}",
            want.len()
        );
    }
}

/// Draws `scene` on every back end and compares.
fn on_all(name: &str, scene: impl Fn(&mut dyn Draw) -> Parts) {
    let reference = scene(&mut webgpu());
    for (route, off) in [
        ("with EXT_clip_control", &[][..]),
        ("without EXT_clip_control", &["EXT_clip_control"][..]),
    ] {
        let got = scene(&mut webgl(off));
        check(name, &reference, &got, route);
    }
}

/// A back end whose drawing a scene runs, so one scene body serves both.
trait Draw {
    fn origin(&mut self) -> Parts;
    fn shapes(&mut self) -> Parts;
    fn mips(&mut self, format: Format, values: Format, bytes: &[u8]) -> Parts;
}

impl<B: Backend> Draw for B {
    fn origin(&mut self) -> Parts {
        origin(self)
    }
    fn shapes(&mut self) -> Parts {
        shapes(self)
    }
    fn mips(&mut self, format: Format, values: Format, bytes: &[u8]) -> Parts {
        mips(self, format, values, bytes)
    }
}

/// A triangle inside its viewport, coloured by values interpolated across it. `place` maps clip
/// position `c` to where the drawing's viewport puts it: `c` itself under the viewport, or the
/// in-program viewport adjustment of `ViewportInShader` under a whole-target one.
fn fraction_program(place: &str) -> String {
    format!(
        "
struct V {{
    @builtin(position) p: vec4<f32>,
    @location(0) c: vec4<f32>,
}}
@vertex fn vs(@builtin(vertex_index) i: u32) -> V {{
    var p = array<vec2<f32>, 3>(vec2(-0.8, -0.7), vec2(0.9, -0.3), vec2(-0.2, 0.85));
    var c = array<vec4<f32>, 3>(vec4(1.0, 0.0, 0.2, 1.0), vec4(0.0, 1.0, 0.4, 1.0), vec4(0.1, 0.3, 1.0, 1.0));
    let c4 = vec4<f32>(p[i], 0.5, 1.0);
    return V({place}, c[i]);
}}
@fragment fn fs(v: V) -> @location(0) vec4<f32> {{
    return v.c;
}}
"
    )
}

/// A viewport at x 2.5, y 1.25, 11 by 12.5 pixels of a 16 by 16 target: natively where the back end
/// has `FRACTIONAL_VIEWPORT`, otherwise through the program variant over the whole target
/// (`ViewportInShader`, the adjustment G6's variant makes).
fn fraction<B: Backend>(b: &mut B) -> Parts {
    let k = Kit::new(b);
    let (x, y, w, h) = (2.5f32, 1.25f32, 11.0f32, 12.5f32);
    let n = 16.0f32;
    let native = b.caps().has(Features::FRACTIONAL_VIEWPORT);
    let (program, vp) = if native {
        (fraction_program("c4"), kit::viewport(x, y, w, h))
    } else {
        b.slow_paths().hit(SlowPath::ViewportInShader);
        // Window x = x + (cx + 1) / 2 * w, written as a clip x over the whole target; y likewise
        // from the top.
        let place = format!(
            "vec4<f32>(c4.x * {sx:.9} + c4.w * {ox:.9}, c4.y * {sy:.9} + c4.w * {oy:.9}, c4.z, c4.w)",
            sx = w / n,
            ox = (2.0 * x + w) / n - 1.0,
            sy = h / n,
            oy = 1.0 - (2.0 * y + h) / n,
        );
        (fraction_program(&place), kit::viewport(0.0, 0.0, n, n))
    };
    let program = kit::program(b, &program, "vs", "fs");
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let f = Format::Rgba8Unorm;
    let t = kit::texture(b, TextureDimension::D2, f, RGBA8, 1, usage, false);
    let v = kit::target(b, &t, f, 0, 0);
    let vps = [vp];
    b.begin_pass(&kit::pass(&v, [0; 4], None));
    b.draw(&kit::call(
        &k,
        &program,
        None,
        &vps,
        Topology::TriangleList,
        3,
    ));
    b.end_pass();
    let parts = vec![(
        format!("native {native}"),
        kit::read(b, &t, 0, 0, RGBA8, 4, false),
    )];
    assert_eq!(b.status(), DeviceStatus::Ok);
    parts
}

const ORIGIN: &str = "
struct V {
    @builtin(position) p: vec4<f32>,
}
// Two triangles in the top half: the left one clockwise on the screen, the right one
// counter-clockwise; with clockwise front faces and back faces culled, only the left one is drawn.
@vertex fn tris(@builtin(vertex_index) i: u32) -> V {
    var p = array<vec2<f32>, 6>(
        vec2(-1.0, 1.0), vec2(-0.1, 1.0), vec2(-1.0, 0.1),
        vec2(0.1, 1.0), vec2(1.0, 0.1), vec2(1.0, 1.0));
    return V(vec4<f32>(p[i], 0.5, 1.0));
}
@fragment fn red() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
// The bottom half, coloured by the fragment's window position.
@vertex fn bottom(@builtin(vertex_index) i: u32) -> V {
    var p = array<vec2<f32>, 4>(vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(-1.0, -1.0), vec2(1.0, -1.0));
    return V(vec4<f32>(p[i], 0.5, 1.0));
}
@vertex fn full(@builtin(vertex_index) i: u32) -> V {
    return V(vec4<f32>(f32((i & 1u) << 2u) - 1.0, f32((i & 2u) << 1u) - 1.0, 0.5, 1.0));
}
@fragment fn gradient(v: V) -> @location(0) vec4<f32> {
    return vec4<f32>(v.p.x / 16.0, v.p.y / 16.0, 0.0, 1.0);
}
";

const RGBA8: [u32; 3] = [16, 16, 1];

/// Both window-origin routes against WebGPU: a culled one-sided pair of triangles and a
/// `gl_FragCoord` gradient in one target, and the gradient drawn into each level of a mip chain, all
/// read back.
fn origin<B: Backend>(b: &mut B) -> Parts {
    let k = Kit::new(b);
    let cull = kit::raster(b, CullMode::Back, FrontFace::Clockwise, 0.0, 0.0);
    let tris = kit::program(b, ORIGIN, "tris", "red");
    let bottom = kit::program(b, ORIGIN, "bottom", "gradient");
    let full = kit::program(b, ORIGIN, "full", "gradient");
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let f = Format::Rgba8Unorm;
    let t = kit::texture(b, TextureDimension::D2, f, RGBA8, 1, usage, false);
    let v = kit::target(b, &t, f, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, 16.0, 16.0)];
    b.begin_pass(&kit::pass(&v, [0; 4], None));
    b.draw(&DrawCall {
        raster: &cull,
        ..kit::call(&k, &tris, None, &vp, Topology::TriangleList, 6)
    });
    b.draw(&kit::call(
        &k,
        &bottom,
        None,
        &vp,
        Topology::TriangleStrip,
        4,
    ));
    b.end_pass();
    let mut parts = vec![(
        "target".to_string(),
        kit::read(b, &t, 0, 0, RGBA8, 4, false),
    )];
    let chain = kit::texture(b, TextureDimension::D2, f, RGBA8, 4, usage, false);
    for mip in 0..4 {
        let n = 16 >> mip;
        let v = kit::target(b, &chain, f, mip, 0);
        let vp = [kit::viewport(0.0, 0.0, n as f32, n as f32)];
        b.begin_pass(&kit::pass(&v, [0; 4], None));
        b.draw(&kit::call(&k, &full, None, &vp, Topology::TriangleList, 3));
        b.end_pass();
        parts.push((
            format!("mip {mip}"),
            kit::read(b, &chain, mip, 0, [n, n, 1], 4, false),
        ));
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
    parts
}

const SHAPES: &str = "
@group(0) @binding(16) var s: sampler;
@group(0) @binding(32) var cube: texture_cube<f32>;
@group(0) @binding(33) var layers: texture_2d_array<f32>;
@group(0) @binding(34) var volume: texture_3d<f32>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((i & 1u) << 2u) - 1.0, f32((i & 2u) << 1u) - 1.0, 0.5, 1.0);
}
// Row 0: the cube sampled towards 16 directions; row 1: the array's texels; row 2: the volume's.
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let x = u32(p.x);
    let y = u32(p.y);
    if (y == 0u) {
        let a = f32(x) * 0.7;
        let d = vec3<f32>(cos(a), f32(x % 3u) - 1.0, sin(a) * 0.8);
        return textureSampleLevel(cube, s, d, 0.0);
    }
    if (y == 1u) {
        return textureLoad(layers, vec2<u32>(x % 4u, (x / 4u) % 4u), x % 3u, 0);
    }
    return textureLoad(volume, vec3<u32>(x % 4u, (x / 4u) % 4u, (x + y) % 4u), 0);
}
";

/// Cube, array and 3D textures, each made with its own GL target, sampled and loaded.
fn shapes<B: Backend>(b: &mut B) -> Parts {
    let k = Kit::new(b);
    let program = kit::program(b, SHAPES, "vs", "fs");
    let f = Format::Rgba8Unorm;
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_DST;
    let cube = kit::texture(b, TextureDimension::D2, f, [4, 4, 6], 1, usage, true);
    let layers = kit::texture(b, TextureDimension::D2, f, [4, 4, 3], 1, usage, false);
    let volume = kit::texture(b, TextureDimension::D3, f, [4, 4, 4], 1, usage, false);
    for (i, (t, n)) in [(&cube, 6u32), (&layers, 3), (&volume, 4)]
        .into_iter()
        .enumerate()
    {
        let bytes: Vec<u8> = (0..64 * n)
            .map(|j| (j * 29 + i as u32 * 71 + 7) as u8)
            .collect();
        b.write_texture(
            &TextureRegion {
                texture: t,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width: 4,
                height: 4,
                depth_or_layers: n,
            },
            &bytes,
            16,
            4,
            WriteMode::Ordered,
        );
    }
    let views = [
        kit::view(b, &cube, f, ViewDimension::Cube, 0, 1, 0, 6),
        kit::view(b, &layers, f, ViewDimension::D2Array, 0, 1, 0, 3),
        kit::view(b, &volume, f, ViewDimension::D3, 0, 1, 0, 1),
    ];
    let sampler = b
        .create_sampler(&SamplerDesc {
            min: Filter::Nearest,
            mag: Filter::Nearest,
            mip: Filter::Nearest,
            address: [AddressMode::Clamp; 3],
            lod_range: [0f32.to_bits(), 0f32.to_bits()],
            compare: None,
            anisotropy: 1,
            border: BorderColor::TransparentBlack,
        })
        .expect("create_sampler");
    let mut entries = vec![BindingEntry {
        binding: 16,
        stages: StageSet::FRAGMENT,
        resource: BindingResource::Sampler(&sampler),
    }];
    for (i, v) in views.iter().enumerate() {
        entries.push(BindingEntry {
            binding: 32 + i as u32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(v),
        });
    }
    let set = b.create_binding_set(&entries).expect("create_binding_set");
    let size = [16, 3, 1];
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let t = kit::texture(b, TextureDimension::D2, f, size, 1, usage, false);
    let v = kit::target(b, &t, f, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, 16.0, 3.0)];
    b.begin_pass(&kit::pass(&v, [0; 4], None));
    b.draw(&kit::call(
        &k,
        &program,
        Some(&set),
        &vp,
        Topology::TriangleList,
        3,
    ));
    b.end_pass();
    let parts = vec![(
        "samples".to_string(),
        kit::read(b, &t, 0, 0, size, 4, false),
    )];
    assert_eq!(b.status(), DeviceStatus::Ok);
    parts
}

/// `generate_mips` of an 8 by 8 texture of `format` holding `values`, written with `bytes`; every
/// level read back.
fn mips<B: Backend>(b: &mut B, format: Format, values: Format, bytes: &[u8]) -> Parts {
    let usage = TextureUsage::SAMPLE
        | TextureUsage::RENDER
        | TextureUsage::COPY_SRC
        | TextureUsage::COPY_DST;
    let desc = TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width: 8,
        height: 8,
        depth_or_layers: 1,
        mip_levels: 4,
        samples: 1,
        usage,
        cube: false,
        reinterpret: false,
    };
    let t = b.create_texture(&desc).expect("create_texture");
    let texel = format.block().2;
    b.write_texture(
        &TextureRegion {
            texture: &t,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        },
        Extent {
            width: 8,
            height: 8,
            depth_or_layers: 1,
        },
        bytes,
        8 * texel,
        8,
        WriteMode::Ordered,
    );
    b.generate_mips(&t, &desc, values, 1..4, 0..1)
        .expect("generate_mips");
    let parts = (1..4)
        .map(|mip| {
            let n = 8 >> mip;
            (
                format!("mip {mip}"),
                kit::read(b, &t, mip, 0, [n, n, 1], texel as usize, false),
            )
        })
        .collect();
    assert_eq!(b.status(), DeviceStatus::Ok);
    parts
}

#[test]
fn both_window_origins_equal_webgpu() {
    on_all("origin", |b| b.origin());
}

/// WebGPU's native fractional viewport against `ViewportInShader` on this back end.
#[test]
fn a_fractional_viewport_in_the_program_equals_webgpu_native() {
    assert!(webgpu().caps().has(Features::FRACTIONAL_VIEWPORT));
    let reference = fraction(&mut webgpu());
    for (route, off) in [
        ("with EXT_clip_control", &[][..]),
        ("without EXT_clip_control", &["EXT_clip_control"][..]),
    ] {
        let mut gl = webgl(off);
        assert!(!gl.caps().has(Features::FRACTIONAL_VIEWPORT));
        let got = fraction(&mut gl);
        let same = |p: &Parts| p.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>();
        assert_eq!(
            same(&reference),
            same(&got),
            "WebGL {route} with the variant against WebGPU's native viewport"
        );
    }
}

#[test]
fn cube_array_and_3d_textures_equal_webgpu() {
    on_all("shapes", |b| b.shapes());
}

/// `r32float` filtered in the program where OES_texture_float_linear is switched off, against
/// WebGPU's filtering sampler.
#[test]
fn generate_mips_of_r32float_without_float_filtering_equals_webgpu() {
    let bytes: Vec<u8> = (0..64u32)
        .flat_map(|i| ((i * 37 % 64) as f32 / 7.0).to_le_bytes())
        .collect();
    let reference = webgpu().mips(Format::R32Float, Format::R32Float, &bytes);
    let mut failed = Vec::new();
    for off in [&["OES_texture_float_linear"][..], &[][..]] {
        let got = webgl(off).mips(Format::R32Float, Format::R32Float, &bytes);
        let route = format!("off {off:?}");
        let r = std::panic::catch_unwind(|| check("r32float mips", &reference, &got, &route));
        if r.is_err() {
            failed.push(route);
        }
    }
    assert!(failed.is_empty(), "failed with extensions {failed:?}");
}

/// `r32float` filtered in the program on both: WebGPU on a device made without float32-filterable
/// (as the WebGPU back end's own comparison with Direct3D makes it), this back end without
/// OES_texture_float_linear.
#[test]
fn generate_mips_of_r32float_filtered_in_the_program_on_both_equals_webgpu() {
    let bytes: Vec<u8> = (0..64u32)
        .flat_map(|i| ((i * 37 % 64) as f32 / 7.0).to_le_bytes())
        .collect();
    let mut gpu = webgpu_without(
        wgpu::Features::FLOAT32_FILTERABLE
            | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
    );
    assert!(
        !gpu.caps()
            .formats
            .get(Format::R32Float)
            .contains(FormatUse::FILTER)
    );
    let reference = gpu.mips(Format::R32Float, Format::R32Float, &bytes);
    assert!(gpu.slow_paths().count(SlowPath::FilterInShader) > 0);
    let mut gl = webgl(&["OES_texture_float_linear"]);
    let got = gl.mips(Format::R32Float, Format::R32Float, &bytes);
    assert!(gl.slow_paths().count(SlowPath::FilterInShader) > 0);
    check(
        "r32float mips",
        &reference,
        &got,
        "off OES_texture_float_linear",
    );
}

/// A 16-bit normalised format stored as its integer twin, filtered in the program on both.
#[test]
fn generate_mips_of_an_integer_twin_equals_webgpu() {
    let bytes: Vec<u8> = (0..128u32)
        .flat_map(|i| ((i * 2741 + 13) as u16).to_le_bytes())
        .collect();
    let reference = webgpu().mips(Format::Rg16Uint, Format::Rg16Unorm, &bytes);
    let got = webgl(&[]).mips(Format::Rg16Uint, Format::Rg16Unorm, &bytes);
    check("rg16unorm as rg16uint mips", &reference, &got, "");
}
