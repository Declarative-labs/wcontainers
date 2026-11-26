//! Checks of what WebGL extensions give the back end: blending per target and from a second source,
//! polygon edges, depth clamping, clamped depth bias, mirror-once addressing, and clip and cull
//! distances; and points sized by the program, which WebGL 2 has without one. Each draws where the back end declares the feature; `absent_features_are_refused`
//! checks the refusal where it does not.

use super::Host;
use crate::kit::{self, Kit};
use wcontainers_gpu::{
    caps::*, draw::*, error::*, format::*, helpers::*, occlusion::*, passes::*, reflect::*,
    resource::*, shader::*, slow::*, state::*, *,
};

/// A module validated with every naga capability, for programs WGSL's core lacks.
pub(super) fn module_all(wgsl: &str, entry: &str, stage: ShaderStage) -> ShaderModule {
    let module = naga::front::wgsl::parse_str(wgsl).expect("the WGSL parses");
    module_of(module, entry, stage)
}

fn module_of(module: naga::Module, entry: &str, stage: ShaderStage) -> ShaderModule {
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("the module validates");
    ShaderModule {
        module,
        info,
        entry_point: entry.into(),
        stage,
        key: [0; 32],
    }
}

fn program_of<B: Gpu>(
    b: &mut B,
    vertex: &ShaderModule,
    fragment: &ShaderModule,
) -> Result<B::Program, GpuError> {
    let v = b.create_shader(vertex)?;
    let f = b.create_shader(fragment)?;
    b.create_program(&Program::Render {
        vertex: &v,
        fragment: Some(&f),
    })
}

/// A `width` by `height` `rgba8unorm` target, and its view.
fn rgba<B: Gpu>(b: &mut B, width: u32, height: u32) -> (B::Texture, B::TextureView) {
    let f = Format::Rgba8Unorm;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let t = kit::texture(
        b,
        TextureDimension::D2,
        f,
        [width, height, 1],
        1,
        usage,
        false,
    );
    let v = kit::target(b, &t, f, 0, 0);
    (t, v)
}

/// The clear value of `rgba8unorm` texels holding `c`.
fn clear(c: [u8; 4]) -> [u32; 4] {
    c.map(|x| (f32::from(x) / 255.0).to_bits())
}

/// A pass on the views in `colours`, each loaded by `load`.
fn pass<'a, B: Gpu>(colours: &[&'a B::TextureView], load: LoadOp<[u32; 4]>) -> PassDesc<'a, B> {
    PassDesc {
        colors: core::array::from_fn(|i| {
            colours.get(i).map(|&view| ColorAttachment {
                view,
                depth_slice: 0,
                load,
                store: StoreOp::Store,
                resolve: None,
            })
        }),
        depth: None,
        occlusion: None,
        empty: None,
    }
}

/// A raster block like the kit's, changed by `change`.
fn raster<B: Gpu>(b: &mut B, change: impl FnOnce(&mut RasterDesc)) -> B::Raster {
    let mut d = RasterDesc {
        fill: FillMode::Solid,
        cull: CullMode::None,
        front: FrontFace::Clockwise,
        depth_bias: 0,
        depth_bias_slope: 0,
        depth_bias_clamp: 0,
        depth_clip: true,
        scissor: false,
        line_width: 1.0f32.to_bits(),
        provoking: ProvokingVertex::Last,
        depth_minus_one: !b.caps().features.contains(Features::CLIP_DEPTH_ZERO_ONE),
        sample_mask: u32::MAX,
    };
    change(&mut d);
    b.create_raster(&d).expect("create_raster")
}

fn texels(bytes: &[u8]) -> &[[u8; 4]] {
    bytes.as_chunks::<4>().0
}

/// Every texel within one step of `want`.
fn near(got: &[u8], want: [u8; 4]) -> bool {
    texels(got)
        .iter()
        .all(|t| t.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 1))
}

/// A full-screen triangle writing one colour to targets 0 and 1, and a second output for blending.
pub(super) const TWO: &str = "
enable dual_source_blending;
const C = vec4<f32>(102.0, 51.0, 204.0, 255.0) / 255.0;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
struct Targets {
    @location(0) a: vec4<f32>,
    @location(1) b: vec4<f32>,
}
@fragment fn fs() -> Targets {
    return Targets(C, C);
}
struct Sources {
    @location(0) @blend_src(0) a: vec4<f32>,
    @location(0) @blend_src(1) b: vec4<f32>,
}
@fragment fn fs_second() -> Sources {
    return Sources(C, vec4<f32>(0.5, 0.0, 1.0, 1.0));
}
";

/// Two targets with a blend and write mask each (OES_draw_buffers_indexed): target 0 adds, target 1
/// writes red and alpha only. A block the same for both follows, and the per-target one again, so
/// both changes between GL's calls for all targets and for one are seen.
pub fn independent_blends_per_target<H: Host>() {
    let mut b = H::make();
    if !b.caps().features.contains(Features::INDEPENDENT_BLEND) {
        return super::not_run(Features::INDEPENDENT_BLEND);
    }
    let k = Kit::new(&mut b);
    let p = program_of(
        &mut b,
        &module_all(TWO, "vs", ShaderStage::Vertex),
        &module_all(TWO, "fs", ShaderStage::Fragment),
    )
    .expect("the program");
    let add = BlendComponent {
        src: BlendFactor::One,
        dst: BlendFactor::One,
        op: BlendOp::Add,
    };
    let mut targets = [RenderTargetBlend {
        blend: None,
        writes: ColorWrites::all(),
    }; MAX_COLOR_TARGETS];
    targets[0].blend = Some((add, add));
    targets[1].writes = ColorWrites::R | ColorWrites::A;
    let each = b
        .create_blend(&BlendDesc {
            targets,
            same_for_all: false,
            alpha_to_coverage: false,
            logic_op: None,
        })
        .expect("a blend per target");
    let (t0, v0) = rgba(&mut b, 4, 4);
    let (t1, v1) = rgba(&mut b, 4, 4);
    let whole = [kit::viewport(0.0, 0.0, 4.0, 4.0)];
    let grey = clear([64; 4]);
    let rounds = [
        (Some(grey), &each, [166, 115, 255, 255], [102, 64, 64, 255]),
        (None, &k.blend, [102, 51, 204, 255], [102, 51, 204, 255]),
        (Some(grey), &each, [166, 115, 255, 255], [102, 64, 64, 255]),
    ];
    for (i, (load, blend, want0, want1)) in rounds.into_iter().enumerate() {
        let load = load.map_or(LoadOp::Load, LoadOp::Clear);
        b.begin_pass(&pass::<H::Backend>(&[&v0, &v1], load));
        b.draw(&DrawCall {
            blend,
            ..kit::call(&k, &p, None, &whole, Topology::TriangleList, 3)
        });
        b.end_pass();
        let got0 = kit::read(&mut b, &t0, 0, 0, [4, 4, 1], 4, false);
        let got1 = kit::read(&mut b, &t1, 0, 0, [4, 4, 1], 4, false);
        assert_eq!(b.status(), DeviceStatus::Ok);
        assert!(near(&got0, want0), "round {i}, target 0: {:?}", &got0[..4]);
        assert!(near(&got1, want1), "round {i}, target 1: {:?}", &got1[..4]);
    }
}

/// A blend reading the fragment's second output (WEBGL_blend_func_extended): colour as source plus
/// destination times the second source, alpha as source plus destination times one minus the second
/// source's alpha. With two colour targets the draw loses the device, as GL blends a second source
/// into one target only.
pub fn second_source_blends<H: Host>() {
    let mut b = H::make();
    if !b.caps().features.contains(Features::DUAL_SOURCE_BLEND) {
        return super::not_run(Features::DUAL_SOURCE_BLEND);
    }
    let k = Kit::new(&mut b);
    let p = program_of(
        &mut b,
        &module_all(TWO, "vs", ShaderStage::Vertex),
        &module_all(TWO, "fs_second", ShaderStage::Fragment),
    )
    .expect("the program");
    let blend = kit::blend(
        &mut b,
        Some((
            BlendComponent {
                src: BlendFactor::One,
                dst: BlendFactor::Src1,
                op: BlendOp::Add,
            },
            BlendComponent {
                src: BlendFactor::One,
                dst: BlendFactor::OneMinusSrc1Alpha,
                op: BlendOp::Add,
            },
        )),
    );
    let (t, v) = rgba(&mut b, 4, 4);
    let whole = [kit::viewport(0.0, 0.0, 4.0, 4.0)];
    b.begin_pass(&pass::<H::Backend>(&[&v], LoadOp::Clear(clear([102; 4]))));
    b.draw(&DrawCall {
        blend: &blend,
        ..kit::call(&k, &p, None, &whole, Topology::TriangleList, 3)
    });
    b.end_pass();
    let got = kit::read(&mut b, &t, 0, 0, [4, 4, 1], 4, false);
    assert_eq!(b.status(), DeviceStatus::Ok);
    // 102 + 102 * 0.5, 51 + 102 * 0, 204 + 102 * 1 (saturated), 255 + 102 * 0.
    assert!(near(&got, [153, 51, 255, 255]), "{:?}", &got[..4]);
    let (_, w) = rgba(&mut b, 4, 4);
    b.begin_pass(&pass::<H::Backend>(&[&v, &w], LoadOp::Clear([0; 4])));
    b.draw(&DrawCall {
        blend: &blend,
        ..kit::call(&k, &p, None, &whole, Topology::TriangleList, 3)
    });
    b.end_pass();
    assert_eq!(b.status(), DeviceStatus::Lost(LossReason::Error));
}

/// A full-screen triangle at window depth `k.z` (written in the raster block's clip convention by
/// the caller), red.
const AT_DEPTH: &str = "
struct Imm {
    z: f32,
}
var<immediate> k: Imm;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, k.z, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
";

/// AT_DEPTH's program (its immediates need naga's capability).
fn at_depth<B: Gpu>(b: &mut B) -> B::Program {
    program_of(
        b,
        &module_all(AT_DEPTH, "vs", ShaderStage::Vertex),
        &module_all(AT_DEPTH, "fs", ShaderStage::Fragment),
    )
    .expect("the program")
}

/// Draws AT_DEPTH at window depth `z` into a `depth32float` texture cleared to 0.5, with `raster`
/// and the viewport's depth range `range`, and returns every stored depth.
fn stored_depth<B: Gpu>(
    b: &mut B,
    k: &Kit<B>,
    program: &B::Program,
    raster: &B::Raster,
    z: f32,
    range: [f32; 2],
) -> Vec<f32> {
    let f = Format::Depth32Float;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let d = kit::texture(b, TextureDimension::D2, f, [4, 4, 1], 1, usage, false);
    let dv = kit::target(b, &d, f, 0, 0);
    let (_, colour) = rgba(b, 4, 4);
    let always = kit::depth_stencil(b, Some(CompareFn::Always), true, None);
    let zero_one = b.caps().features.contains(Features::CLIP_DEPTH_ZERO_ONE);
    // Window depth runs over the range from clip z 0 (or -1) to 1.
    let t = (z - range[0]) / (range[1] - range[0]);
    let clip = if zero_one { t } else { 2.0 * t - 1.0 };
    let viewport = [Viewport {
        min_depth: range[0].to_bits(),
        max_depth: range[1].to_bits(),
        ..kit::viewport(0.0, 0.0, 4.0, 4.0)
    }];
    b.begin_pass(&kit::pass(&colour, [0; 4], Some((&dv, 0.5, false))));
    b.draw(&DrawCall {
        raster,
        depth_stencil: &always,
        immediates: &clip.to_le_bytes(),
        ..kit::call(k, program, None, &viewport, Topology::TriangleList, 3)
    });
    b.end_pass();
    let got = kit::read(b, &d, 0, 0, [4, 4, 1], 4, false);
    got.as_chunks::<4>()
        .0
        .iter()
        .map(|t| f32::from_le_bytes(*t))
        .collect()
}

/// Depth clamped instead of clipped (EXT_depth_clamp), to the viewport's depth range: a triangle
/// past each end of the range of 0.25 to 0.75 stores that end; clipped, it stores nothing.
pub fn depth_clamped_to_the_viewport_range<H: Host>() {
    let mut b = H::make();
    if !b.caps().features.contains(Features::DEPTH_CLAMP) {
        return super::not_run(Features::DEPTH_CLAMP);
    }
    let k = Kit::new(&mut b);
    let p = at_depth(&mut b);
    let clamped = raster(&mut b, |d| d.depth_clip = false);
    let range = [0.25, 0.75];
    for (z, end) in [(1.5f32, 0.75f32), (-0.5, 0.25)] {
        let got = stored_depth(&mut b, &k, &p, &clamped, z, range);
        assert!(got.iter().all(|d| *d == end), "clamped at {z}: {got:?}");
        let got = stored_depth(&mut b, &k, &p, &k.raster, z, range);
        assert!(got.iter().all(|d| *d == 0.5), "clipped at {z}: {got:?}");
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// A constant depth bias clamped (EXT_polygon_offset_clamp): a million units at depth 0.5 of a
/// `depth32float` texture (2^-24 each, so 0.0596) clamped to 0.01, then minus a million clamped to
/// -0.01; without a clamp the whole bias.
pub fn depth_bias_clamped<H: Host>() {
    let mut b = H::make();
    if !b.caps().features.contains(Features::DEPTH_BIAS_CLAMP) {
        return super::not_run(Features::DEPTH_BIAS_CLAMP);
    }
    let k = Kit::new(&mut b);
    let p = at_depth(&mut b);
    let units = 1.0e6f32;
    let whole = units * 2f32.powi(-24);
    for (bias, clamp, want) in [
        (units, 0.01f32, 0.51f32),
        (-units, -0.01, 0.49),
        (units, 0.0, 0.5 + whole),
    ] {
        let r = raster(&mut b, |d| {
            d.depth_bias = bias.to_bits();
            d.depth_bias_clamp = clamp.to_bits();
        });
        let got = stored_depth(&mut b, &k, &p, &r, 0.5, [0.0, 1.0]);
        assert!(
            got.iter().all(|d| (d - want).abs() < 1.0e-6),
            "bias {bias} clamped to {clamp}: {got:?}, want {want}"
        );
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// A triangle in a 16 by 16 target, drawn red.
const TRIANGLE: &str = "
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2<f32>(-0.8, -0.8), vec2<f32>(0.0, 0.85), vec2<f32>(0.75, -0.6));
    return vec4<f32>(p[v], 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
";

/// Polygon edges (WEBGL_polygon_mode): a triangle filled with lines leaves its inside clear, and
/// every pixel it draws lies on an edge, within a pixel; the same triangle filled covers its inside.
pub fn polygons_filled_with_lines<H: Host>() {
    let mut b = H::make();
    if !b.caps().features.contains(Features::POLYGON_MODE) {
        return super::not_run(Features::POLYGON_MODE);
    }
    let k = Kit::new(&mut b);
    let p = kit::program(&mut b, TRIANGLE, "vs", "fs");
    let lines = raster(&mut b, |d| d.fill = FillMode::Line);
    let (t, v) = rgba(&mut b, 16, 16);
    let whole = [kit::viewport(0.0, 0.0, 16.0, 16.0)];
    // The corners in pixels, rows from the top.
    let corners = [(-0.8f32, -0.8f32), (0.0, 0.85), (0.75, -0.6)]
        .map(|(x, y)| ((x + 1.0) * 8.0, (1.0 - y) * 8.0));
    let distance = |px: f32, py: f32| {
        (0..3)
            .map(|i| {
                let (a, c) = (corners[i], corners[(i + 1) % 3]);
                let (dx, dy) = (c.0 - a.0, c.1 - a.1);
                let t = (((px - a.0) * dx + (py - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
                ((px - a.0 - t * dx).powi(2) + (py - a.1 - t * dy).powi(2)).sqrt()
            })
            .fold(f32::MAX, f32::min)
    };
    for (raster, filled) in [(&lines, false), (&k.raster, true)] {
        b.begin_pass(&kit::pass(&v, [0; 4], None));
        b.draw(&DrawCall {
            raster,
            ..kit::call(&k, &p, None, &whole, Topology::TriangleList, 3)
        });
        b.end_pass();
        let got = kit::read(&mut b, &t, 0, 0, [16, 16, 1], 4, false);
        assert_eq!(b.status(), DeviceStatus::Ok);
        let drawn: Vec<(f32, f32)> = texels(&got)
            .iter()
            .enumerate()
            .filter(|(_, t)| t[0] == 255)
            .map(|(i, _)| ((i % 16) as f32 + 0.5, (i / 16) as f32 + 0.5))
            .collect();
        // The middle of the triangle, (8, 9.5) in pixels, is drawn only when filled.
        let middle = drawn.contains(&(7.5, 9.5));
        assert_eq!(middle, filled, "filled {filled}: the middle");
        if !filled {
            assert!(drawn.len() >= 30, "only {} pixels of edges", drawn.len());
            for (x, y) in drawn {
                assert!(distance(x, y) <= 1.0, "({x}, {y}) is off every edge");
            }
        }
    }
}

/// A 4 by 1 texture of 10, 20, 30 and 40 sampled across u from -1.875 with mirror-once addressing
/// (EXT_texture_mirror_clamp_to_edge): mirrored once about 0, then clamped to the edge.
const MIRROR: &str = "
@group(0) @binding(16) var s: sampler;
@group(0) @binding(32) var t: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return textureSample(t, s, vec2<f32>(p.x / 2.0 - 2.125, 0.5));
}
";

pub fn mirror_once_addressing<H: Host>() {
    let mut b = H::make();
    if !b.caps().features.contains(Features::MIRROR_ONCE) {
        return super::not_run(Features::MIRROR_ONCE);
    }
    let k = Kit::new(&mut b);
    let p = kit::program(&mut b, MIRROR, "vs", "fs");
    let f = Format::Rgba8Unorm;
    let tex = kit::texture(
        &mut b,
        TextureDimension::D2,
        f,
        [4, 1, 1],
        1,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
        false,
    );
    let texels: Vec<u8> = [10u8, 20, 30, 40]
        .iter()
        .flat_map(|x| [*x, 0, 0, 255])
        .collect();
    b.write_texture(
        &TextureRegion {
            texture: &tex,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        },
        Extent {
            width: 4,
            height: 1,
            depth_or_layers: 1,
        },
        &texels,
        16,
        1,
        WriteMode::Ordered,
    );
    let view = kit::view(&mut b, &tex, f, ViewDimension::D2, 0, 1, 0, 1);
    let sampler = b
        .create_sampler(&SamplerDesc {
            min: Filter::Nearest,
            mag: Filter::Nearest,
            mip: Filter::Nearest,
            address: [
                AddressMode::MirrorOnce,
                AddressMode::Clamp,
                AddressMode::Clamp,
            ],
            lod_range: [0f32.to_bits(), 0f32.to_bits()],
            anisotropy: 1,
            compare: None,
            border: BorderColor::TransparentBlack,
        })
        .expect("a mirror-once sampler");
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
    let (t, v) = rgba(&mut b, 8, 1);
    let whole = [kit::viewport(0.0, 0.0, 8.0, 1.0)];
    b.begin_pass(&kit::pass(&v, [0; 4], None));
    b.draw(&kit::call(
        &k,
        &p,
        Some(&set),
        &whole,
        Topology::TriangleList,
        3,
    ));
    b.end_pass();
    let got = kit::read(&mut b, &t, 0, 0, [8, 1, 1], 4, false);
    assert_eq!(b.status(), DeviceStatus::Ok);
    // u from -1.875 to 1.625 in steps of 0.5, each at a texel's middle: mirrored about 0 once, so
    // -0.375 reads texel 1, and clamped to the last texel past 1.
    let red: Vec<u8> = texels_red(&got);
    assert_eq!(red, [40, 40, 40, 20, 10, 30, 40, 40]);
}

fn texels_red(bytes: &[u8]) -> Vec<u8> {
    texels(bytes).iter().map(|t| t[0]).collect()
}

/// A full-screen triangle with two clip distances, x and y in clip space, so only the top right
/// quarter is kept; then a program without them draws everywhere.
pub(super) const CLIPPED: &str = "
enable clip_distances;
struct Out {
    @builtin(position) p: vec4<f32>,
    @builtin(clip_distances) d: array<f32, 2>,
}
@vertex fn vs(@builtin(vertex_index) v: u32) -> Out {
    let p = vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
    return Out(p, array<f32, 2>(p.x, p.y));
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
";

/// Two triangles over the whole target: the first red with every cull distance -1, so it is culled;
/// the second green with +1.
const CULLED: &str = "
enable clip_distances;
struct Out {
    @builtin(position) p: vec4<f32>,
    @builtin(clip_distances) d: array<f32, 1>,
    @location(0) @interpolate(flat) c: vec4<f32>,
}
@vertex fn vs(@builtin(vertex_index) v: u32) -> Out {
    let p = vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
    let second = v >= 3u;
    let c = select(vec4<f32>(1.0, 0.0, 0.0, 1.0), vec4<f32>(0.0, 1.0, 0.0, 1.0), second);
    return Out(p, array<f32, 1>(select(-1.0, 1.0, second)), c);
}
@fragment fn fs(@location(0) @interpolate(flat) c: vec4<f32>) -> @location(0) vec4<f32> {
    return c;
}
";

/// User clip distances (WEBGL_clip_cull_distance) keep the quarter where both are positive, and a
/// later program without them is not clipped; cull distances drop the whole triangle where every
/// vertex's is negative.
pub fn clip_and_cull_distances<H: Host>() {
    let mut b = H::make();
    let k = Kit::new(&mut b);
    let (t, v) = rgba(&mut b, 8, 8);
    let whole = [kit::viewport(0.0, 0.0, 8.0, 8.0)];
    let draw = |b: &mut H::Backend, p: &<H::Backend as Gpu>::Program, count| {
        b.begin_pass(&kit::pass(&v, [0; 4], None));
        b.draw(&kit::call(
            &k,
            p,
            None,
            &whole,
            Topology::TriangleList,
            count,
        ));
        b.end_pass();
        let got = kit::read(b, &t, 0, 0, [8, 8, 1], 4, false);
        assert_eq!(b.status(), DeviceStatus::Ok);
        got
    };
    if b.caps().features.contains(Features::CLIP_DISTANCE) {
        let p = program_of(
            &mut b,
            &module_all(CLIPPED, "vs", ShaderStage::Vertex),
            &module_all(CLIPPED, "fs", ShaderStage::Fragment),
        )
        .expect("the program");
        let got = draw(&mut b, &p, 3);
        for (i, t) in texels(&got).iter().enumerate() {
            let kept = i % 8 >= 4 && i / 8 < 4;
            assert_eq!(t[0] == 255, kept, "pixel ({}, {}): {t:?}", i % 8, i / 8);
        }
        // Red over the whole target, at depth 0.
        let plain = at_depth(&mut b);
        let got = draw(&mut b, &plain, 3);
        assert!(
            texels(&got).iter().all(|t| t[0] == 255),
            "clipped after: {got:?}"
        );
    }
    if !b.caps().features.contains(Features::CLIP_DISTANCE) {
        super::not_run(Features::CLIP_DISTANCE);
    }
    if !b.caps().features.contains(Features::CULL_DISTANCE) {
        super::not_run(Features::CULL_DISTANCE);
    } else {
        let mut m = naga::front::wgsl::parse_str(CULLED).expect("the WGSL parses");
        // WGSL has no cull distances: the clip distance member becomes one.
        let out = m
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Out"))
            .map(|(h, _)| h)
            .expect("Out");
        let mut ty = m.types[out].clone();
        if let naga::TypeInner::Struct { members, .. } = &mut ty.inner {
            members[1].binding = Some(naga::Binding::BuiltIn(naga::BuiltIn::CullDistance));
        }
        m.types.replace(out, ty);
        let p = program_of(
            &mut b,
            &module_of(m, "vs", ShaderStage::Vertex),
            &module_all(CULLED, "fs", ShaderStage::Fragment),
        )
        .expect("the program");
        let got = draw(&mut b, &p, 6);
        assert!(near(&got, [0, 255, 0, 255]), "{:?}", &got[..4]);
    }
}

/// One point at the middle of a 16 by 16 target, its size written by the program: 6 when given.
const POINT: &str = "
struct Out {
    @builtin(position) p: vec4<f32>,
    @location(1) size: f32,
}
@vertex fn vs() -> Out {
    return Out(vec4<f32>(0.0, 0.0, 0.0, 1.0), 6.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
";

/// Points sized by the vertex program (`gl_PointSize`): a point of size 6 centred on a pixel corner
/// covers the 6 by 6 pixels around it; one whose program writes no size covers one pixel.
pub fn points_sized_by_the_program<H: Host>() {
    let mut b = H::make();
    if !b.caps().features.contains(Features::POINT_SIZE) {
        return super::not_run(Features::POINT_SIZE);
    }
    assert!(b.caps().limits.max_point_size >= 6.0);
    let k = Kit::new(&mut b);
    let mut m = naga::front::wgsl::parse_str(POINT).expect("the WGSL parses");
    // WGSL has no point size: the location member becomes one.
    let out = m
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("Out"))
        .map(|(h, _)| h)
        .expect("Out");
    let mut ty = m.types[out].clone();
    if let naga::TypeInner::Struct { members, .. } = &mut ty.inner {
        members[1].binding = Some(naga::Binding::BuiltIn(naga::BuiltIn::PointSize));
    }
    m.types.replace(out, ty);
    let sized = program_of(
        &mut b,
        &module_of(m, "vs", ShaderStage::Vertex),
        &module_all(POINT, "fs", ShaderStage::Fragment),
    )
    .expect("the program");
    let plain = kit::program(&mut b, POINT, "vs", "fs");
    let (t, v) = rgba(&mut b, 16, 16);
    let whole = [kit::viewport(0.0, 0.0, 16.0, 16.0)];
    for (p, side) in [(&sized, 6), (&plain, 1)] {
        b.begin_pass(&kit::pass(&v, [0; 4], None));
        b.draw(&kit::call(&k, p, None, &whole, Topology::PointList, 1));
        b.end_pass();
        let got = kit::read(&mut b, &t, 0, 0, [16, 16, 1], 4, false);
        assert_eq!(b.status(), DeviceStatus::Ok);
        let drawn: Vec<(usize, usize)> = texels(&got)
            .iter()
            .enumerate()
            .filter(|(_, t)| t[0] == 255)
            .map(|(i, _)| (i % 16, i / 16))
            .collect();
        assert_eq!(drawn.len(), side * side, "size {side}: {drawn:?}");
        // The point's square runs from 8 - side / 2 to 8 + side / 2; size 1 touches four pixel
        // centres at its edges, so only its count is fixed.
        if side > 1 {
            let inside = |c: usize| (8 - side / 2..8 + side / 2).contains(&c);
            assert!(
                drawn.iter().all(|(x, y)| inside(*x) && inside(*y)),
                "{drawn:?}"
            );
        }
    }
}

/// Draws and clears that break the interface's rules lose the device instead of drawing something
/// else: no viewport, two viewports, front and back stencil references that differ without
/// `STENCIL_PER_FACE`, and a buffer clear whose pattern is not 1, 2, 4, 8 or 16 bytes long.
pub fn broken_rules_lose_the_device<H: Host>() {
    for case in 0..4 {
        let mut b = H::make();
        if case == 3 {
            let buffer = super::buffer(&mut b, BufferUsage::VERTEX, &[0; 24]);
            b.clear_buffer(&buffer, 0, 24, &[1, 2, 3]);
        } else {
            let k = Kit::new(&mut b);
            let (_t, v) = rgba(&mut b, 8, 8);
            let p = kit::program(&mut b, TRIANGLE, "vs", "fs");
            let viewports = [kit::viewport(0.0, 0.0, 8.0, 8.0); 2];
            let n = [0, 2, 1][case];
            let mut call = kit::call(&k, &p, None, &viewports[..n], Topology::TriangleList, 3);
            if case == 2 {
                call.stencil_reference = [1, 2];
            }
            b.begin_pass(&kit::pass(&v, [0; 4], None));
            b.draw(&call);
            b.end_pass();
        }
        assert_eq!(
            b.status(),
            DeviceStatus::Lost(LossReason::Error),
            "case {case}"
        );
    }
}

/// A stage reading more uniform buffers than the limit is refused when it is made, with the rule,
/// and one reading as many as the limit is made and draws (GL would refuse to link past it).
pub fn stages_past_the_uniform_buffer_limit_are_refused<H: Host>() {
    let mut b = H::make();
    let limit = b.caps().limits.uniform_buffers_per_stage;
    let wgsl = |n: u32| {
        let mut s = String::from(
            "@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
                return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
            }\n",
        );
        let mut sum = String::from("vec4<f32>(0.0)");
        for i in 0..n {
            s += &format!("@group(1) @binding({i}) var<uniform> u{i}: vec4<f32>;\n");
            sum += &format!(" + u{i}");
        }
        s + &format!("@fragment fn fs() -> @location(0) vec4<f32> {{ return {sum}; }}\n")
    };
    let over = ShaderModule::from_wgsl(&wgsl(limit + 1), "fs", ShaderStage::Fragment)
        .expect("the WGSL validates");
    assert_eq!(
        b.create_shader(&over).err(),
        Some(GpuError::Invalid(
            "a stage reads more uniform buffers than Limits::uniform_buffers_per_stage"
        ))
    );
    let k = Kit::new(&mut b);
    let p = kit::program(&mut b, &wgsl(limit), "vs", "fs");
    let block = super::buffer(&mut b, BufferUsage::UNIFORM, &[0; 16]);
    let entries: Vec<_> = (0..limit)
        .map(|i| BindingEntry {
            binding: i,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &block,
                offset: 0,
                size: 16,
            }),
        })
        .collect();
    let set = b.create_binding_set(&entries).expect("create_binding_set");
    let (t, v) = rgba(&mut b, 8, 8);
    let viewports = [kit::viewport(0.0, 0.0, 8.0, 8.0)];
    let mut call = kit::call(&k, &p, None, &viewports, Topology::TriangleList, 3);
    call.bindings = [None, Some(&set), None, None];
    b.begin_pass(&kit::pass(&v, [0; 4], None));
    b.draw(&call);
    b.end_pass();
    b.submit();
    let _ = kit::read(&mut b, &t, 0, 0, [1, 1, 1], 4, false);
    assert_eq!(b.status(), DeviceStatus::Ok);
}
