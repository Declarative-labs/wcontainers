//! Drawings made the same way on the WebGL 2 and the WebGPU back end, whose texels read back must be
//! equal, generic over the back end: run natively on ANGLE against wgpu's Direct3D 12
//! (tests/against_webgpu.rs) and in Chrome against its WebGPU (examples/browser.rs, against.html).

use crate::kit::{self, Kit};
use wcontainers_gpu::*;

/// What one drawing gives: named parts read back.
pub type Parts = Vec<(String, Vec<u8>)>;

/// The WebGPU device a comparison draws its reference on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Reference {
    /// With every feature the back end asks for.
    Full,
    /// Without `float32-filterable`, so `r32float` is filtered in the program there too.
    WithoutFloatFiltering,
}

/// What a comparison draws.
#[derive(Clone, Copy)]
pub enum Scene {
    Origin,
    Fraction,
    Shapes,
    /// Gathers of a cube whose footprints cross each edge of each face.
    CubeGathers,
    /// Gathers through samplers whose minimum level of detail is 0 and 1, alone and beside a sample
    /// through the same pair.
    LevelZeroGathers,
    /// `generate_mips` of a texture of `format` holding `values`, from `bytes()`.
    Mips {
        format: Format,
        values: Format,
        bytes: fn() -> Vec<u8>,
    },
}

/// One comparison: the reference, the WebGL routes (extensions switched off for each), the
/// drawing, and what each side's caps and counters must show beside its texels (`webgpu` says
/// which side).
pub struct Comparison {
    pub name: &'static str,
    pub reference: Reference,
    pub routes: &'static [&'static [&'static str]],
    pub scene: Scene,
    pub expect: fn(webgpu: bool, caps: &Caps, slow: &SlowPaths) -> Result<(), String>,
}

const BOTH_ORIGINS: &[&[&str]] = &[&[], &["EXT_clip_control"]];

fn nothing(_: bool, _: &Caps, _: &SlowPaths) -> Result<(), String> {
    Ok(())
}

fn r32float_bytes() -> Vec<u8> {
    (0..64u32)
        .flat_map(|i| ((i * 37 % 64) as f32 / 7.0).to_le_bytes())
        .collect()
}

fn rg16_bytes() -> Vec<u8> {
    (0..128u32)
        .flat_map(|i| ((i * 2741 + 13) as u16).to_le_bytes())
        .collect()
}

/// 256 values from xorshift32 seeded 0x9e3779b9, the low 16 bits of each: 12 of the 64 averages
/// of mip 1 of an 8 by 8 rgba16 texture fall exactly halfway between two steps.
fn rgba16_bytes() -> Vec<u8> {
    let mut x = 0x9e37_79b9u32;
    (0..256)
        .flat_map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x as u16).to_le_bytes()
        })
        .collect()
}

/// A side whose `r32float` is not filterable filters it in the program.
fn r32float_filtered_in_the_program_without_filtering(
    _: bool,
    caps: &Caps,
    slow: &SlowPaths,
) -> Result<(), String> {
    let filterable = caps
        .formats
        .get(Format::R32Float)
        .contains(FormatUse::FILTER);
    if !filterable && slow.count(SlowPath::FilterInShader) == 0 {
        return Err("r32float is not filterable, but no FilterInShader was counted".into());
    }
    Ok(())
}

/// Every comparison, each a test natively and a page in Chrome.
pub const COMPARISONS: [Comparison; 9] = [
    Comparison {
        name: "both_window_origins_equal_webgpu",
        reference: Reference::Full,
        routes: BOTH_ORIGINS,
        scene: Scene::Origin,
        expect: nothing,
    },
    // WebGPU's native fractional viewport against `ViewportInShader` on this back end.
    Comparison {
        name: "a_fractional_viewport_in_the_program_equals_webgpu_native",
        reference: Reference::Full,
        routes: BOTH_ORIGINS,
        scene: Scene::Fraction,
        expect: |webgpu, caps, _| {
            if caps.features.contains(Features::FRACTIONAL_VIEWPORT) == webgpu {
                Ok(())
            } else {
                Err(format!(
                    "FRACTIONAL_VIEWPORT {} on {}",
                    if webgpu { "absent" } else { "present" },
                    if webgpu { "WebGPU" } else { "WebGL" }
                ))
            }
        },
    },
    Comparison {
        name: "cube_array_and_3d_textures_equal_webgpu",
        reference: Reference::Full,
        routes: BOTH_ORIGINS,
        scene: Scene::Shapes,
        expect: nothing,
    },
    // GLSL ES 3.00 has no gathers: this back end samples each texel of the footprint by direction,
    // which crosses into the next face as WebGPU's gathers do.
    Comparison {
        name: "cube_gathers_across_face_edges_equal_webgpu",
        reference: Reference::Full,
        routes: &[&[]],
        scene: Scene::CubeGathers,
        expect: nothing,
    },
    // Gathers read level 0 whatever the sampler's minimum level of detail, as WebGPU's do, while a
    // sample through the same pair keeps the minimum.
    Comparison {
        name: "gathers_through_a_sampler_with_a_minimum_level_equal_webgpu",
        reference: Reference::Full,
        routes: &[&[]],
        scene: Scene::LevelZeroGathers,
        expect: nothing,
    },
    // `r32float` filtered in the program where OES_texture_float_linear is switched off, against
    // WebGPU's filtering sampler.
    Comparison {
        name: "generate_mips_of_r32float_without_float_filtering_equals_webgpu",
        reference: Reference::Full,
        routes: &[&["OES_texture_float_linear"], &[]],
        scene: Scene::Mips {
            format: Format::R32Float,
            values: Format::R32Float,
            bytes: r32float_bytes,
        },
        expect: r32float_filtered_in_the_program_without_filtering,
    },
    // `r32float` filtered in the program on both: WebGPU on a device made without
    // float32-filterable (as the WebGPU back end's own comparison with Direct3D makes it), this back
    // end without OES_texture_float_linear.
    Comparison {
        name: "generate_mips_of_r32float_filtered_in_the_program_on_both_equals_webgpu",
        reference: Reference::WithoutFloatFiltering,
        routes: &[&["OES_texture_float_linear"]],
        scene: Scene::Mips {
            format: Format::R32Float,
            values: Format::R32Float,
            bytes: r32float_bytes,
        },
        expect: |_, caps, slow| {
            if caps
                .formats
                .get(Format::R32Float)
                .contains(FormatUse::FILTER)
            {
                return Err("r32float is filterable".into());
            }
            r32float_filtered_in_the_program_without_filtering(false, caps, slow)
        },
    },
    // A 16-bit normalised format stored as its integer twin, filtered in the program on both.
    Comparison {
        name: "generate_mips_of_an_integer_twin_equals_webgpu",
        reference: Reference::Full,
        routes: &[&[]],
        scene: Scene::Mips {
            format: Format::Rg16Uint,
            values: Format::Rg16Unorm,
            bytes: rg16_bytes,
        },
        expect: nothing,
    },
    // The same where box averages are exact ties, which each side's float arithmetic rounds.
    Comparison {
        name: "generate_mips_of_an_integer_twin_at_exact_ties_equals_webgpu",
        reference: Reference::Full,
        routes: &[&[]],
        scene: Scene::Mips {
            format: Format::Rgba16Uint,
            values: Format::Rgba16Unorm,
            bytes: rgba16_bytes,
        },
        expect: nothing,
    },
];

/// Draws `scene`; an error if the device did not stay whole.
pub fn draw<B: Backend>(b: &mut B, scene: Scene) -> Result<Parts, String> {
    let parts = match scene {
        Scene::Origin => origin(b),
        Scene::Fraction => fraction(b),
        Scene::Shapes => shapes(b),
        Scene::CubeGathers => cube_gathers(b),
        Scene::LevelZeroGathers => level_zero_gathers(b),
        Scene::Mips {
            format,
            values,
            bytes,
        } => mips(b, format, values, &bytes()),
    };
    match b.status() {
        DeviceStatus::Ok => Ok(parts),
        s => Err(format!("the device is {s:?}")),
    }
}

/// The route's name, by the extensions it switches off.
pub fn route_name(off: &[&str]) -> String {
    if off.is_empty() {
        "with every extension".into()
    } else {
        format!("without {}", off.join(" "))
    }
}

/// Draws `c` on the WebGPU back end `gpu` and on a WebGL back end per route, which `gl` makes with
/// those extensions off; one verdict per route.
pub fn compare<G: Backend, L: Backend>(
    c: &Comparison,
    gpu: &mut G,
    mut gl: impl FnMut(&[&str]) -> L,
) -> Vec<(String, Result<(), String>)> {
    let reference = draw(gpu, c.scene)
        .and_then(|p| (c.expect)(true, &gpu.caps().clone(), gpu.slow_paths()).map(|()| p))
        .map_err(|e| format!("WebGPU: {e}"));
    c.routes
        .iter()
        .map(|off| {
            let verdict = reference.clone().and_then(|reference| {
                let mut b = gl(off);
                let got = draw(&mut b, c.scene)?;
                (c.expect)(false, &b.caps().clone(), b.slow_paths())?;
                equal(&reference, &got)
            });
            (route_name(off), verdict)
        })
        .collect()
}

/// Every part equal, byte for byte.
fn equal(reference: &Parts, got: &Parts) -> Result<(), String> {
    if reference.len() != got.len() {
        return Err(format!("{} parts, WebGPU {}", got.len(), reference.len()));
    }
    for ((part, want), (_, got)) in reference.iter().zip(got) {
        let differ = want.iter().zip(got).filter(|(a, b)| a != b).count();
        if differ != 0 || want.len() != got.len() {
            return Err(format!(
                "{part}: {differ} of {} bytes differ from WebGPU's\n  webgpu {want:?}\n  webgl  {got:?}",
                want.len()
            ));
        }
    }
    Ok(())
}

/// A triangle reaching past its viewport on every side but the bottom right, coloured by values
/// interpolated across it. `place` maps clip position `c4` to where the drawing's viewport puts it:
/// `c4` itself under the viewport, or the in-program adjustment of `ViewportInShader` under the
/// enclosing whole-pixel one.
fn fraction_program(place: &str) -> String {
    format!(
        "
struct V {{
    @builtin(position) p: vec4<f32>,
    @location(0) c: vec4<f32>,
}}
@vertex fn vs(@builtin(vertex_index) i: u32) -> V {{
    var p = array<vec2<f32>, 3>(vec2(-1.6, -1.3), vec2(1.5, -0.6), vec2(-0.3, 1.7));
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

/// A viewport at x 2.25, y 1.25, 11 by 12.5 pixels of a 16 by 16 target: natively where the back
/// end has `FRACTIONAL_VIEWPORT`; otherwise as a Direct3D front without it draws it (the interface's
/// `FRACTIONAL_VIEWPORT` text): the enclosing whole-pixel viewport, the program variant mapping
/// positions onto the fractional one (the adjustment of `HelperValue::ViewportAdjust`, written
/// into the program here), and the scissor cut to Direct3D's implicit one, each edge rounded down.
/// The triangle is clipped at the viewport's edges, so the scissor decides those pixels: the last
/// row, whose centre (13.5) the viewport holds, is outside Direct3D's scissor. The left and top
/// edges leave no pixel centre in the half pixel Direct3D leaves undefined.
fn fraction<B: Backend>(b: &mut B) -> Parts {
    let k = Kit::new(b);
    let (x, y, w, h) = (2.25f32, 1.25f32, 11.0f32, 12.5f32);
    let native = b.caps().features.contains(Features::FRACTIONAL_VIEWPORT);
    let (x0, y0) = (x.floor(), y.floor());
    let (ww, wh) = ((x + w).ceil() - x0, (y + h).ceil() - y0);
    let (program, vp) = if native {
        (fraction_program("c4"), kit::viewport(x, y, w, h))
    } else {
        // Window x = x + (cx + 1) / 2 * w, written as a clip x of the whole-pixel viewport; y
        // likewise from the top.
        let place = format!(
            "vec4<f32>(c4.x * {sx:.9} + c4.w * {ox:.9}, c4.y * {sy:.9} + c4.w * {oy:.9}, c4.z, c4.w)",
            sx = w / ww,
            ox = (2.0 * (x - x0) + w) / ww - 1.0,
            sy = h / wh,
            oy = 1.0 - (2.0 * (y - y0) + h) / wh,
        );
        (fraction_program(&place), kit::viewport(x0, y0, ww, wh))
    };
    // Direct3D's implicit scissor.
    let cut = [ScissorRect {
        x: x.floor() as u32,
        y: y.floor() as u32,
        width: ((x + w).floor() - x.floor()) as u32,
        height: ((y + h).floor() - y.floor()) as u32,
    }];
    let scissored = b
        .create_raster(&RasterDesc {
            fill: FillMode::Solid,
            cull: CullMode::None,
            front: FrontFace::Clockwise,
            depth_bias: 0,
            depth_bias_slope: 0,
            depth_bias_clamp: 0,
            depth_clip: true,
            scissor: true,
            line_width: 1.0f32.to_bits(),
            provoking: if b.caps().features.contains(Features::PROVOKING_VERTEX_FIRST) {
                ProvokingVertex::First
            } else {
                ProvokingVertex::Last
            },
            depth_minus_one: !b.caps().features.contains(Features::CLIP_DEPTH_ZERO_ONE),
            sample_mask: u32::MAX,
        })
        .expect("create_raster");
    let program = kit::program(b, &program, "vs", "fs");
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let f = Format::Rgba8Unorm;
    let t = kit::texture(b, TextureDimension::D2, f, RGBA8, 1, usage, false);
    let v = kit::target(b, &t, f, 0, 0);
    let vps = [vp];
    let mut call = kit::call(&k, &program, None, &vps, Topology::TriangleList, 3);
    if !native {
        call.raster = &scissored;
        call.scissors = &cut;
    }
    b.begin_pass(&kit::pass(&v, [0; 4], None));
    b.draw(&call);
    b.end_pass();
    vec![(
        "viewport".to_string(),
        kit::read(b, &t, 0, 0, RGBA8, 4, false),
    )]
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
    vec![(
        "samples".to_string(),
        kit::read(b, &t, 0, 0, size, 4, false),
    )]
}

/// Pixel (x, face) gathers red from face `face` of a 4 by 4 cube at a footprint crossing edge x / 3
/// of the face (left, right, top, bottom), at the edge's texel x % 3; corners are left out, as
/// APIs leave them to the implementation.
const CUBE_GATHERS: &str = "
@group(0) @binding(16) var s: sampler;
@group(0) @binding(32) var c: texture_cube<f32>;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let x = u32(p.x);
    let face = u32(p.y);
    let along = (f32(x % 3u) + 0.75) / 4.0;
    var st = vec2<f32>(0.05, along);
    switch x / 3u {
        case 1u: { st = vec2<f32>(0.95, along); }
        case 2u: { st = vec2<f32>(along, 0.05); }
        case 3u: { st = vec2<f32>(along, 0.95); }
        default: {}
    }
    let q = st * 2.0 - 1.0;
    var d = vec3<f32>(1.0, -q.y, -q.x);
    switch face {
        case 1u: { d = vec3<f32>(-1.0, -q.y, q.x); }
        case 2u: { d = vec3<f32>(q.x, 1.0, q.y); }
        case 3u: { d = vec3<f32>(q.x, -1.0, -q.y); }
        case 4u: { d = vec3<f32>(q.x, -q.y, 1.0); }
        case 5u: { d = vec3<f32>(-q.x, -q.y, -1.0); }
        default: {}
    }
    return textureGather(0, c, s, d);
}
";

fn cube_gathers<B: Backend>(b: &mut B) -> Parts {
    let k = Kit::new(b);
    let program = kit::program(b, CUBE_GATHERS, "vs", "fs");
    let f = Format::Rgba8Unorm;
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_DST;
    let cube = kit::texture(b, TextureDimension::D2, f, [4, 4, 6], 1, usage, true);
    // Red tells every texel of every face apart.
    let bytes: Vec<u8> = (0..96u8)
        .flat_map(|i| [2 * i + 10, i, 255 - i, 255])
        .collect();
    b.write_texture(
        &TextureRegion {
            texture: &cube,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        },
        Extent {
            width: 4,
            height: 4,
            depth_or_layers: 6,
        },
        &bytes,
        16,
        4,
        WriteMode::Ordered,
    );
    let view = kit::view(b, &cube, f, ViewDimension::Cube, 0, 1, 0, 6);
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
    let set = b
        .create_binding_set(&[
            BindingEntry {
                binding: 16,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Sampler(&sampler),
            },
            BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&view),
            },
        ])
        .expect("create_binding_set");
    let size = [12, 6, 1];
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let t = kit::texture(b, TextureDimension::D2, f, size, 1, usage, false);
    let v = kit::target(b, &t, f, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, 12.0, 6.0)];
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
    vec![(
        "gathers".to_string(),
        kit::read(b, &t, 0, 0, size, 4, false),
    )]
}

/// Gathers of red over an 8 by 8 target, alone and beside a sample at level 0 through the same
/// texture and sampler, in alpha.
const LEVEL_ZERO_GATHERS: &str = "
@group(0) @binding(16) var s: sampler;
@group(0) @binding(32) var t: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
@fragment fn gathered(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return textureGather(0, t, s, (p.xy + 0.25) / 8.0);
}
@fragment fn gathered_and_sampled(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = (p.xy + 0.25) / 8.0;
    return vec4<f32>(textureGather(0, t, s, uv).xyz, textureSampleLevel(t, s, uv, 0.0).x);
}
";

/// Each program of [`LEVEL_ZERO_GATHERS`] through samplers whose minimum level of detail is 0 and
/// 1, from an 8 by 8 texture of four levels whose red tells every texel of every level apart.
fn level_zero_gathers<B: Backend>(b: &mut B) -> Parts {
    let k = Kit::new(b);
    let f = Format::Rgba8Unorm;
    let levels = 4;
    let side = 8;
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_DST;
    let texture = kit::texture(
        b,
        TextureDimension::D2,
        f,
        [side, side, 1],
        levels,
        usage,
        false,
    );
    for level in 0..levels {
        let width = side >> level;
        let bytes: Vec<u8> = (0..width * width)
            .flat_map(|i| [(64 * level + i) as u8, 0, 0, 255])
            .collect();
        b.write_texture(
            &TextureRegion {
                texture: &texture,
                mip: level,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width,
                height: width,
                depth_or_layers: 1,
            },
            &bytes,
            width * 4,
            width,
            WriteMode::Ordered,
        );
    }
    let view = kit::view(b, &texture, f, ViewDimension::D2, 0, levels, 0, 1);
    let size = [side, side, 1];
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let target = kit::texture(b, TextureDimension::D2, f, size, 1, usage, false);
    let v = kit::target(b, &target, f, 0, 0);
    let vp = [kit::viewport(0.0, 0.0, side as f32, side as f32)];
    let mut parts = Vec::new();
    for entry in ["gathered", "gathered_and_sampled"] {
        let program = kit::program(b, LEVEL_ZERO_GATHERS, "vs", entry);
        for minimum in [0f32, 1.0] {
            let sampler = b
                .create_sampler(&SamplerDesc {
                    min: Filter::Nearest,
                    mag: Filter::Nearest,
                    mip: Filter::Nearest,
                    address: [AddressMode::Clamp; 3],
                    lod_range: [minimum.to_bits(), 32f32.to_bits()],
                    compare: None,
                    anisotropy: 1,
                    border: BorderColor::TransparentBlack,
                })
                .expect("create_sampler");
            let set = b
                .create_binding_set(&[
                    BindingEntry {
                        binding: 16,
                        stages: StageSet::FRAGMENT,
                        resource: BindingResource::Sampler(&sampler),
                    },
                    BindingEntry {
                        binding: 32,
                        stages: StageSet::FRAGMENT,
                        resource: BindingResource::Texture(&view),
                    },
                ])
                .expect("create_binding_set");
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
            parts.push((
                format!("{entry}, minimum level {minimum}"),
                kit::read(b, &target, 0, 0, size, 4, false),
            ));
            b.destroy(Object::BindingSet(set));
            b.destroy(Object::Sampler(sampler));
        }
        b.destroy(Object::Program(program));
    }
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
    (1..4)
        .map(|mip| {
            let n = 8 >> mip;
            (
                format!("mip {mip}"),
                kit::read(b, &t, mip, 0, [n, n, 1], texel as usize, false),
            )
        })
        .collect()
}
