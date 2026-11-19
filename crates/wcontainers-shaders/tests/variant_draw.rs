//! Each built variant drawn through a back end that lacks the feature it stands in for, against the
//! plain program drawn with that feature natively (on the WebGPU back end, or the WebGL 2 one where
//! only it has the feature), or against real Direct3D 11's picture where neither has it. Both routes
//! (fxc's and dxc's builds) are drawn.
//!
//! Windows only, and needs a GPU: run as a GPU job (docs/status-graphics-shaders.md).

#![cfg(windows)]

mod common;

use common::*;
use wcontainers_gpu::{
    AddressMode, BlendComponent, BlendFactor, BlendOp, DrawBaseSource, Filter, Format, HelperValue,
    LinkedValues, ProvokingVertex, TargetInShader, TexelBufferRoute, VariantFlags, VariantKey,
    VertexFormat,
};
use wcontainers_shaders::{Source, convert};
use windows::Win32::Graphics::Direct3D11::*;

const ATTRIBUTES: &[(&str, u32, VertexFormat)] = &[
    ("POSITION", 0, VertexFormat::Float32x3),
    ("TEXCOORD", 12, VertexFormat::Float32x2),
];

/// The own shaders `vs` and `ps` over the grid, by the route of `dxil`.
fn grid_draw<'a>(code: &'a [Vec<u8>; 2], dxil: bool, grid: &'a [f32]) -> Draw<'a> {
    let source = |c: &'a Vec<u8>| {
        if dxil {
            Source::Dxil(c)
        } else {
            Source::Dxbc(c)
        }
    };
    Draw {
        vs: source(&code[0]),
        ps: source(&code[1]),
        format: Format::Rgba8Unorm,
        size: SIZE,
        vertices: grid,
        stride: 20,
        attributes: ATTRIBUTES,
        count: 96,
        constants: constants([1.0, 0.75, 0.5, 1.5]),
        clear: CLEAR.map(f32::to_bits),
        setup: Setup::plain(SIZE),
    }
}

/// A stage's helper block for `key`, its values put where the reflection lays them out.
fn helper_block(source: Source<'_>, key: &VariantKey, values: &[(HelperValue, &[u8])]) -> Vec<u8> {
    let c = convert(source, key, None).unwrap();
    let mut block = vec![0; c.reflection.helper_size as usize];
    for slot in &c.reflection.helpers {
        let (_, bytes) = values
            .iter()
            .find(|(v, _)| *v == slot.value)
            .unwrap_or_else(|| panic!("no value for {:?}", slot.value));
        let at = slot.offset as usize;
        block[at..at + bytes.len()].copy_from_slice(bytes);
    }
    block
}

/// The first location past every one the draw's two programs list, as a front links them for the
/// values clip distances and depth clamping add.
fn linked_location(draw: &Draw<'_>) -> u32 {
    let vs = convert(draw.vs, &VariantKey::default(), None).unwrap();
    let ps = convert(draw.ps, &VariantKey::default(), None).unwrap();
    vs.reflection
        .outputs
        .iter()
        .chain(&ps.reflection.inputs)
        .filter(|v| v.system.is_none())
        .map(|v| v.location + 1)
        .max()
        .unwrap_or(0)
}

/// The clip distances the draw's vertex program writes, counted from its reflection as a front does.
fn clip_distances(draw: &Draw<'_>) -> u8 {
    let vs = convert(draw.vs, &VariantKey::default(), None).unwrap();
    vs.reflection
        .outputs
        .iter()
        .filter(|v| v.system == Some(wcontainers_gpu::SystemValue::ClipDistance))
        .map(|v| v.mask.count_ones())
        .sum::<u32>() as u8
}

fn floats(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// One comparison: how many texels differ and by how much, against what.
struct Compared {
    name: String,
    count: usize,
    largest: u64,
    /// The most texels allowed to differ, and by how much (zero for an exact match).
    allowed: (usize, u64),
}

fn compare(
    name: String,
    reference: &[u8],
    ours: &[u8],
    format: Format,
    allowed: (usize, u64),
) -> Compared {
    let (count, largest) = differ(reference, ours, format);
    let kept = name.replace(' ', "_");
    keep(&format!("{kept}.reference"), reference, format);
    keep(&format!("{kept}.ours"), ours, format);
    Compared {
        name,
        count,
        largest,
        allowed,
    }
}

#[test]
fn variants_draw_as_the_native_feature_does() {
    let mut gpu = webgpu();
    let mut gl = webgl();
    let mut vulkan = webgpu_on(wgpu::Backends::VULKAN);
    let d3d = wcontainers_graphics_test::d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let grid = grid();
    let mut results = Vec::new();
    for (route, dxil) in [("dxbc", false), ("dxil", true)] {
        let code = [
            bytecode(&format!("vs_transform.{route}")),
            bytecode(&format!("ps_textured.{route}")),
        ];
        let plain = grid_draw(&code, dxil, &grid);

        // A target of another type than the program's output: the output's bits written as the
        // target's type, against real Direct3D 11 drawing the same program into that target. fxc's
        // ps_integer (uint4, no dxc build) into a float target too.
        {
            let mut cases = vec![("ps_textured", Format::Rgba32Uint, code.clone())];
            if !dxil {
                cases.push((
                    "ps_integer",
                    Format::Rgba32Float,
                    [code[0].clone(), bytecode("ps_integer.dxbc")],
                ));
            }
            for (ps, format, code) in cases {
                let (reference, _) = direct3d_11(
                    &d3d,
                    "vs_transform",
                    ps,
                    format,
                    [1.0, 0.75, 0.5, 1.5],
                    true,
                    &Reference::default(),
                );
                let mut variant = grid_draw(&code, dxil, &grid);
                variant.format = format;
                variant.clear = match format {
                    Format::Rgba32Uint => [0; 4],
                    _ => CLEAR.map(f32::to_bits),
                };
                variant.setup.ps_key.targets_in_shader = vec![TargetInShader {
                    target: 0,
                    format,
                    combine: false,
                }];
                let texels = ours(&mut gpu, &variant, ProvokingVertex::First);
                results.push(compare(
                    format!("targets_in_shader {ps} {format:?} {route} webgpu"),
                    &reference,
                    &texels,
                    Format::Rgba32Uint,
                    (0, 0),
                ));
            }
        }

        // The fractional viewport (3.25, 2.25, 50.5, 41.75) natively on WebGPU, and as the whole-pixel
        // one (3, 2, 51, 42) with clip x and y mapped in the program. Only the left edge is clipped by
        // the viewport, at x 3.25 natively and 3 with the variant, short of pixel 3's centre either way.
        {
            let mut native = plain.clone();
            native.setup.viewport = [3.25, 2.25, 50.5, 41.75, 0.0, 1.0];
            let reference = ours(&mut gpu, &native, ProvokingVertex::First);
            let (v, w) = ([3.25f32, 2.25, 50.5, 41.75], [3.0f32, 2.0, 51.0, 42.0]);
            let adjust = [
                v[2] / w[2],
                v[3] / w[3],
                (2.0 * (v[0] - w[0]) + v[2] - w[2]) / w[2],
                (w[3] - v[3] - 2.0 * (v[1] - w[1])) / w[3],
            ];
            let mut variant = plain.clone();
            variant.setup.viewport = [w[0], w[1], w[2], w[3], 0.0, 1.0];
            variant.setup.vs_key.flags = VariantFlags::VIEWPORT_ADJUST;
            variant.setup.helpers[0] = helper_block(
                variant.vs,
                &variant.setup.vs_key,
                &[(HelperValue::ViewportAdjust, &floats(&adjust))],
            );
            for (back_end, texels) in [
                ("webgpu", ours(&mut gpu, &variant, ProvokingVertex::First)),
                ("webgl", ours(&mut gl, &variant, ProvokingVertex::Last)),
            ] {
                results.push(compare(
                    format!("viewport_adjust {route} {back_end}"),
                    &reference,
                    &texels,
                    Format::Rgba8Unorm,
                    (0, 0),
                ));
            }
        }

        // Clip depth from 0 to 1: WebGL 2 with EXT_clip_control and WebGPU natively, and WebGL 2
        // without the extension with the program rewriting z as 2z - w; each depth buffer against
        // real Direct3D 11's, compared as f32 steps.
        {
            let (reference, _) = direct3d_11(
                &d3d,
                "vs_transform",
                "ps_textured",
                Format::Rgba8Unorm,
                [1.0, 0.75, 0.5, 1.5],
                true,
                &Reference {
                    depth: Some([0.0, 1.0]),
                    ..Default::default()
                },
            );
            let mut native = plain.clone();
            native.setup.depth = true;
            for (back_end, texels) in [
                (
                    "webgl-native",
                    ours(&mut gl, &native, ProvokingVertex::Last),
                ),
                (
                    "webgpu-native",
                    ours(&mut gpu, &native, ProvokingVertex::First),
                ),
            ] {
                results.push(compare(
                    format!("clip_depth {route} {back_end} against direct3d 11"),
                    &reference,
                    &texels,
                    Format::Depth32Float,
                    (SIZE as usize * SIZE as usize, 2),
                ));
            }
        }

        // A view's component mapping: the plain program reading texels uploaded already mapped, as a
        // view with the mapping reads them, and the variant applying it to the texels as they are.
        {
            use wcontainers_gpu::ViewComponent as C;
            let map = [C::B, C::One, C::Zero, C::R];
            let mut native = plain.clone();
            native.setup.texel_map = Some(map);
            let mut variant = plain.clone();
            variant.setup.ps_key.swizzled = vec![(0, map)];
            for (back_end, reference, texels) in [
                (
                    "webgpu",
                    ours(&mut gpu, &native, ProvokingVertex::First),
                    ours(&mut gpu, &variant, ProvokingVertex::First),
                ),
                (
                    "webgl",
                    ours(&mut gl, &native, ProvokingVertex::Last),
                    ours(&mut gl, &variant, ProvokingVertex::Last),
                ),
            ] {
                results.push(compare(
                    format!("swizzled {route} {back_end}"),
                    &reference,
                    &texels,
                    Format::Rgba8Unorm,
                    (0, 0),
                ));
            }
        }

        // A swapped depth range (near 1, far 0, as Vulkan and OpenGL allow and WebGPU does not), on
        // WebGPU as (0, 1) with the program flipping z. Neither Direct3D 11 nor ANGLE here draws a
        // swapped range as those APIs define it (window depth near + (far - near) z), so the reference
        // is that definition applied to real Direct3D 11's depth of the plain range: 1 - z where drawn,
        // which the rewrite meets up to rounding.
        {
            let (plain_depth, _) = direct3d_11(
                &d3d,
                "vs_transform",
                "ps_textured",
                Format::Rgba8Unorm,
                [1.0, 0.75, 0.5, 1.5],
                true,
                &Reference {
                    depth: Some([0.0, 1.0]),
                    ..Default::default()
                },
            );
            let reference: Vec<u8> = plain_depth
                .chunks(4)
                .flat_map(|t| {
                    let z = f32::from_le_bytes(t.try_into().unwrap());
                    if z == 1.0 { z } else { 1.0 - z }.to_le_bytes()
                })
                .collect();
            let mut variant = plain.clone();
            variant.setup.depth = true;
            variant.setup.vs_key.flags = VariantFlags::VIEWPORT_DEPTH_FLIP;
            let texels = ours(&mut gpu, &variant, ProvokingVertex::First);
            results.push(compare(
                format!("viewport_depth_flip {route} webgpu against 1 - direct3d 11's depth"),
                &reference,
                &texels,
                Format::Depth32Float,
                // Rounding only: the program rounds w - z before the divide, the reference 1 - z/w
                // after it, each within two f32 steps of depths near 0.5.
                (SIZE as usize * SIZE as usize, 4),
            ));
        }

        // Depth bias on a 24-bit depth buffer whose back end stores floats: real Direct3D 11 drawing
        // into D24_UNORM_S8_UINT with the rasteriser's bias, and each back end drawing into a
        // Depth32Float target with the program adding the bias with the 24-bit step; ours taken to
        // 24-bit codes as Direct3D's buffer stores them. Window depth runs from 0.67 to 0.4 across
        // the grid (w = x / 4 + 1), so the slope term shows; the clamps, just past the constant term,
        // cut it wherever it shows, from above and (negated) from below.
        let mut biased: Vec<Vec<u8>> = Vec::new();
        for bias in [
            (2000, 0.25f32, 0.0f32),
            (2000, 0.25, 0.0002),
            (-2000, -0.25, -0.0002),
        ] {
            let (reference, _) = direct3d_11(
                &d3d,
                "vs_transform",
                "ps_textured",
                Format::Rgba8Unorm,
                [1.0, 0.75, 0.5, 1.5],
                true,
                &Reference {
                    depth: Some([0.0, 1.0]),
                    depth_bias: Some(bias),
                    ..Default::default()
                },
            );
            let mut variant = plain.clone();
            variant.setup.depth = true;
            variant.setup.ps_key.flags = VariantFlags::DEPTH_BIAS;
            let step = 1.0 / 16_777_215.0f32;
            variant.setup.helpers[1] = helper_block(
                variant.ps,
                &variant.setup.ps_key,
                &[(
                    HelperValue::DepthBias,
                    &floats(&[bias.0 as f32, bias.1, bias.2, step]),
                )],
            );
            for (back_end, texels) in [
                ("webgpu", ours(&mut gpu, &variant, ProvokingVertex::First)),
                ("webgl", ours(&mut gl, &variant, ProvokingVertex::Last)),
            ] {
                let codes: Vec<u8> = texels
                    .chunks(4)
                    .flat_map(|t| {
                        let z = f32::from_le_bytes(t.try_into().unwrap()).clamp(0.0, 1.0);
                        ((z as f64 * 16_777_215.0).round() as u32).to_le_bytes()
                    })
                    .collect();
                results.push(compare(
                    format!(
                        "depth_bias {bias:?} {route} {back_end} against direct3d 11, 24-bit codes"
                    ),
                    &reference,
                    &codes,
                    // Codes compare as integers this way.
                    Format::Depth32Float,
                    (SIZE as usize * SIZE as usize, 1),
                ));
            }
            biased.push(reference);
        }
        // The clamp acts: Direct3D's two depth buffers differ where the slope is steepest.
        let (clamped, _) = differ(&biased[0], &biased[1], Format::Depth32Float);
        eprintln!("depth_bias {route}: the clamp changes {clamped} of Direct3D 11's texels");
        assert!(clamped > 0);

        // The vertex and instance numbers of draws with non-zero bases (Direct3D's `SV_VertexID` and
        // `SV_InstanceID` leave the bases out where WebGPU's numbers include them), direct with the
        // bases in the helper block and indirect with them read from the argument buffer, against
        // real Direct3D 11 drawing the same ranges.
        let ids = [
            bytecode(&format!("vs_ids.{route}")),
            bytecode(&format!("ps_ids.{route}")),
        ];
        let mut cases = Vec::new();
        for indirect in [false, true] {
            cases.push(Bases {
                count: 48,
                first: 48,
                base_vertex: 0,
                first_instance: 1,
                instances: 2,
                indexed: false,
                indirect,
            });
            cases.push(Bases {
                count: 42,
                first: 6,
                base_vertex: 48,
                first_instance: 2,
                instances: 2,
                indexed: true,
                indirect,
            });
        }
        for bases in cases {
            let (reference, _) = direct3d_11(
                &d3d,
                "vs_ids",
                "ps_ids",
                Format::Rgba8Unorm,
                [1.0; 4],
                true,
                &Reference {
                    bases: Some(bases),
                    ..Default::default()
                },
            );
            let mut variant = grid_draw(&ids, dxil, &grid);
            variant.setup.bases = Some(bases);
            let block = if bases.indirect {
                variant.setup.vs_key.draw_base = DrawBaseSource::Arguments;
                let words = [ARGUMENTS_WORD, bases.indexed as u32, 0, 0];
                (HelperValue::DrawArguments, words)
            } else {
                variant.setup.vs_key.draw_base = DrawBaseSource::Helpers;
                let first = if bases.indexed {
                    bases.base_vertex
                } else {
                    bases.first as i32
                };
                (
                    HelperValue::DrawBase,
                    [first as u32, bases.first_instance, 0, 0],
                )
            };
            let words: Vec<u8> = block.1.iter().flat_map(|w| w.to_le_bytes()).collect();
            variant.setup.helpers[0] =
                helper_block(variant.vs, &variant.setup.vs_key, &[(block.0, &words)]);
            let mut drawn = vec![("webgpu", ours(&mut gpu, &variant, ProvokingVertex::First))];
            // WebGL 2 has no indirect draws (its front reads the arguments back and draws them
            // directly) and no storage buffers to read them from.
            if !bases.indirect {
                drawn.push(("webgl", ours(&mut gl, &variant, ProvokingVertex::Last)));
            }
            for (back_end, texels) in drawn {
                results.push(compare(
                    format!(
                        "draw_base {route} {back_end} {}{} against direct3d 11",
                        if bases.indirect { "indirect " } else { "" },
                        if bases.indexed { "indexed" } else { "vertices" },
                    ),
                    &reference,
                    &texels,
                    Format::Rgba8Unorm,
                    (0, 0),
                ));
            }
        }

        // The sample mask 0b0101 of four samples: natively on WebGPU, and written by the program on
        // WebGPU; the resolved pictures compared. WebGL 2 cannot take this variant: its language writes
        // no sample mask (no OES_sample_variables in WebGL).
        {
            let mut native = plain.clone();
            native.setup.samples = 4;
            native.setup.sample_mask = 0b0101;
            let reference = ours(&mut gpu, &native, ProvokingVertex::First);
            let mut variant = plain.clone();
            variant.setup.samples = 4;
            variant.setup.ps_key.flags = VariantFlags::SAMPLE_MASK;
            variant.setup.helpers[1] = helper_block(
                variant.ps,
                &variant.setup.ps_key,
                &[(HelperValue::SampleMask, &5u32.to_le_bytes())],
            );
            let texels = ours(&mut gpu, &variant, ProvokingVertex::First);
            results.push(compare(
                format!("sample_mask {route} webgpu"),
                &reference,
                &texels,
                Format::Rgba8Unorm,
                (0, 0),
            ));
        }

        // Constant buffer 0 read as a storage buffer, against the plain program on WebGPU.
        {
            let reference = ours(&mut gpu, &plain, ProvokingVertex::First);
            let mut variant = plain.clone();
            variant.setup.vs_key.constants_as_storage = 1;
            variant.setup.ps_key.constants_as_storage = 1;
            let texels = ours(&mut gpu, &variant, ProvokingVertex::First);
            results.push(compare(
                format!("constants_as_storage {route} webgpu"),
                &reference,
                &texels,
                Format::Rgba8Unorm,
                (0, 0),
            ));
        }

        // Constant buffer 0 read through a texture, in both stages, with a fixed row (`vs_transform`,
        // `ps_textured`) and a row picked by the pixel's column (`ps_rows`), against the plain
        // programs' uniform blocks on WebGL 2 and on WebGPU.
        for ps in ["ps_textured", "ps_rows"] {
            let code = [
                bytecode(&format!("vs_transform.{route}")),
                bytecode(&format!("{ps}.{route}")),
            ];
            let plain = grid_draw(&code, dxil, &grid);
            let mut variant = plain.clone();
            variant.setup.vs_key.constants_as_texture = 1;
            variant.setup.ps_key.constants_as_texture = 1;
            for (back_end, reference, texels) in [
                (
                    "webgl",
                    ours(&mut gl, &plain, ProvokingVertex::Last),
                    ours(&mut gl, &variant, ProvokingVertex::Last),
                ),
                (
                    "webgpu",
                    ours(&mut gpu, &plain, ProvokingVertex::First),
                    ours(&mut gpu, &variant, ProvokingVertex::First),
                ),
            ] {
                results.push(compare(
                    format!("constants_as_texture {ps} {route} {back_end}"),
                    &reference,
                    &texels,
                    Format::Rgba8Unorm,
                    (0, 0),
                ));
            }
        }

        // The sampler's level-of-detail bias (2.5, over a texture of four levels), which neither
        // back end's samplers have: against real Direct3D 11's MipLODBias (fxc's build).
        {
            let bias = 2.5f32;
            let (reference, _) = direct3d_11(
                &d3d,
                "vs_transform",
                "ps_textured",
                Format::Rgba8Unorm,
                [1.0, 0.75, 0.5, 1.5],
                true,
                &Reference {
                    levels: 4,
                    lod_bias: bias,
                    ..Default::default()
                },
            );
            let mut variant = plain.clone();
            variant.setup.levels = 4;
            variant.setup.ps_key.lod_bias = 1;
            variant.setup.helpers[1] = helper_block(
                variant.ps,
                &variant.setup.ps_key,
                &[(HelperValue::LodBias { sampler: 0 }, &bias.to_le_bytes())],
            );
            for (back_end, texels) in [
                ("webgpu", ours(&mut gpu, &variant, ProvokingVertex::First)),
                ("webgl", ours(&mut gl, &variant, ProvokingVertex::Last)),
            ] {
                results.push(compare(
                    format!("lod_bias {route} {back_end} against direct3d 11"),
                    &reference,
                    &texels,
                    Format::Rgba8Unorm,
                    (0, 1),
                ));
            }
        }

        // Mirror-once addressing in u beside repeat in v, on a 4-level texture sampled far outside it
        // (levels past 0 chosen): the program folds u on a clamping sampler, against real Direct3D
        // 11's MIRROR_ONCE, on both back ends (neither has mirror-once). The mode must change
        // Direct3D's picture from clamping, or the case shows nothing.
        {
            let code = [
                bytecode(&format!("vs_transform.{route}")),
                bytecode(&format!("ps_mirror.{route}")),
            ];
            let drawn = |u| {
                direct3d_11(
                    &d3d,
                    "vs_transform",
                    "ps_mirror",
                    Format::Rgba8Unorm,
                    [1.0, 0.75, 0.5, 1.5],
                    true,
                    &Reference {
                        levels: 4,
                        address: Some([u, D3D11_TEXTURE_ADDRESS_WRAP, D3D11_TEXTURE_ADDRESS_CLAMP]),
                        ..Default::default()
                    },
                )
                .0
            };
            let reference = drawn(D3D11_TEXTURE_ADDRESS_MIRROR_ONCE);
            let (changed, _) = differ(
                &drawn(D3D11_TEXTURE_ADDRESS_CLAMP),
                &reference,
                Format::Rgba8Unorm,
            );
            eprintln!("mirror_once {route}: the mode changes {changed} of Direct3D's texels");
            assert!(changed > 0);
            let mut variant = grid_draw(&code, dxil, &grid);
            variant.setup.levels = 4;
            variant.setup.address = [AddressMode::Clamp, AddressMode::Repeat, AddressMode::Clamp];
            variant.setup.ps_key.mirror_once_in_shader = 1;
            let codes = [Filter::Linear as u32; 3]
                .into_iter()
                .chain(
                    [
                        AddressMode::MirrorOnce,
                        AddressMode::Repeat,
                        AddressMode::Clamp,
                    ]
                    .map(|m| m as u32),
                )
                .chain([1, 0, f32::MAX.to_bits()]);
            let words: Vec<u8> = codes.flat_map(u32::to_le_bytes).collect();
            variant.setup.helpers[1] = helper_block(
                variant.ps,
                &variant.setup.ps_key,
                &[(HelperValue::SamplerState { sampler: 0 }, &words)],
            );
            for (back_end, texels) in [
                ("webgpu", ours(&mut gpu, &variant, ProvokingVertex::First)),
                ("webgl", ours(&mut gl, &variant, ProvokingVertex::Last)),
            ] {
                results.push(compare(
                    format!("mirror_once {route} {back_end} against direct3d 11"),
                    &reference,
                    &texels,
                    Format::Rgba8Unorm,
                    // The level of detail from explicit gradients rounds apart from the implicit
                    // one: trilinear weights move by a step.
                    (SIZE as usize * SIZE as usize, 1),
                ));
            }
        }

        // The per-resource minimum level-of-detail clamp, which neither back end has: against real
        // Direct3D 11's SetResourceMinLOD on a 4-level texture, unclamped, fractional (samples blend
        // levels 1 and 2, loads of level 0 read zero), at the last level, and past it (every access
        // reads zero). Gathers, which read level 0 only, on WebGPU (GLSL ES 3.00 has no gather).
        for (ps, clamps, back_ends) in [
            ("ps_minlod", &[0.0f32, 1.5, 3.0, 3.5][..], 2),
            ("ps_mingather", &[0.5, 1.0, 3.5][..], 1),
        ] {
            let code = [
                bytecode(&format!("vs_transform.{route}")),
                bytecode(&format!("{ps}.{route}")),
            ];
            let plain = grid_draw(&code, dxil, &grid);
            let mut unclamped = None;
            for &clamp in clamps {
                let (reference, _) = direct3d_11(
                    &d3d,
                    "vs_transform",
                    ps,
                    Format::Rgba8Unorm,
                    [1.0, 0.75, 0.5, 1.5],
                    true,
                    &Reference {
                        levels: 4,
                        min_lod: Some(clamp),
                        ..Default::default()
                    },
                );
                // The clamp must change Direct3D's picture, or the case shows nothing.
                let first = unclamped.get_or_insert_with(|| reference.clone());
                if clamp > clamps[0] {
                    let (changed, _) = differ(first, &reference, Format::Rgba8Unorm);
                    eprintln!("{ps} clamp {clamp}: {changed} of Direct3D's texels changed by it");
                    assert!(changed > 0, "{ps} clamp {clamp} changes nothing");
                }
                let mut variant = plain.clone();
                variant.setup.levels = 4;
                variant.setup.min_lod = Some(clamp);
                variant.setup.ps_key.min_lod_clamped = 1;
                let mut value = clamp.to_bits().to_le_bytes().to_vec();
                value.extend(4u32.to_le_bytes());
                variant.setup.helpers[1] = helper_block(
                    variant.ps,
                    &variant.setup.ps_key,
                    &[(HelperValue::MinLod { texture: 0 }, &value)],
                );
                let mut drawn = vec![("webgpu", ours(&mut gpu, &variant, ProvokingVertex::First))];
                if back_ends > 1 {
                    drawn.push(("webgl", ours(&mut gl, &variant, ProvokingVertex::Last)));
                }
                for (back_end, texels) in drawn {
                    results.push(compare(
                        format!("min_lod {ps} {clamp} {route} {back_end} against direct3d 11"),
                        &reference,
                        &texels,
                        Format::Rgba8Unorm,
                        (0, 1),
                    ));
                }
            }
        }

        // Clip distances natively (WebGPU's clip_distances, on wgpu's Vulkan path: its Direct3D 12
        // path has none), against real Direct3D 11; the planes must change Direct3D's picture.
        for vs in ["vs_clip", "vs_clip_six"] {
            let code = [
                bytecode(&format!("{vs}.{route}")),
                bytecode(&format!("ps_textured.{route}")),
            ];
            let native = grid_draw(&code, dxil, &grid);
            let tint = [1.0, 0.75, 0.5, 1.5];
            let (reference, _) = direct3d_11(
                &d3d,
                vs,
                "ps_textured",
                Format::Rgba8Unorm,
                tint,
                true,
                &Reference::default(),
            );
            let (unclipped, _) = direct3d_11(
                &d3d,
                "vs_transform",
                "ps_textured",
                Format::Rgba8Unorm,
                tint,
                true,
                &Reference::default(),
            );
            let (changed, _) = differ(&unclipped, &reference, Format::Rgba8Unorm);
            eprintln!("clip distances {vs} {route}: they change {changed} of Direct3D's texels");
            assert!(changed > 0);
            let texels = ours(&mut vulkan, &native, ProvokingVertex::First);
            results.push(compare(
                format!("clip distances {vs} {route} webgpu-native (vulkan) against direct3d 11"),
                &reference,
                &texels,
                Format::Rgba8Unorm,
                (0, 0),
            ));

            // The distances passed as values and clipped by the pixel program, where the back end
            // has none (WebGPU on wgpu's Direct3D 12 path, WebGL 2 without the extension): one sample
            // against Direct3D 11, four against the native distances on Vulkan, resolved.
            let linked = LinkedValues {
                location: linked_location(&native),
                clip_distances: clip_distances(&native),
            };
            let mut variant = native.clone();
            for key in [&mut variant.setup.vs_key, &mut variant.setup.ps_key] {
                key.flags = VariantFlags::CLIP_DISTANCE;
                key.linked = linked;
                key.samples = 1;
            }
            for (back_end, texels) in [
                ("webgpu", ours(&mut gpu, &variant, ProvokingVertex::First)),
                ("webgl", ours(&mut gl, &variant, ProvokingVertex::Last)),
            ] {
                results.push(compare(
                    format!("clip_distance {vs} {route} {back_end} against direct3d 11"),
                    &reference,
                    &texels,
                    Format::Rgba8Unorm,
                    // The same pixels drawn; attributes interpolated over the whole triangle round
                    // apart from those over the clipped one by a step or two. A pixel drawn on one
                    // side only would differ by far more.
                    (SIZE as usize * SIZE as usize, 2),
                ));
            }
            let mut native4 = native.clone();
            native4.setup.samples = 4;
            let reference4 = ours(&mut vulkan, &native4, ProvokingVertex::First);
            let mut variant4 = variant.clone();
            variant4.setup.samples = 4;
            variant4.setup.ps_key.samples = 4;
            let texels = ours(&mut gpu, &variant4, ProvokingVertex::First);
            results.push(compare(
                format!("clip_distance {vs} {route} webgpu four samples against vulkan's native"),
                &reference4,
                &texels,
                Format::Rgba8Unorm,
                (SIZE as usize * SIZE as usize, 2),
            ));
        }

        // Depth clamped instead of clipped, with depth reaching past both ends of the clip volume and
        // a depth range of 0.25 to 0.75: against real Direct3D 11 with DepthClipEnable false, depth
        // buffers compared as f32 steps. Clipping must change Direct3D's depth, or the case shows
        // nothing.
        {
            let code = [
                bytecode(&format!("vs_deep.{route}")),
                bytecode(&format!("ps_textured.{route}")),
            ];
            let range = [0.25f32, 0.75];
            let drawn = |clip_off| {
                direct3d_11(
                    &d3d,
                    "vs_deep",
                    "ps_textured",
                    Format::Rgba8Unorm,
                    [1.0, 0.75, 0.5, 1.5],
                    true,
                    &Reference {
                        depth: Some(range),
                        depth_clip_off: clip_off,
                        ..Default::default()
                    },
                )
                .0
            };
            let (reference, clipped) = (drawn(true), drawn(false));
            let (changed, _) = differ(&clipped, &reference, Format::Depth32Float);
            eprintln!("depth_clamp {route}: clipping changes {changed} of Direct3D's texels");
            assert!(changed > 0);
            let mut variant = grid_draw(&code, dxil, &grid);
            variant.setup.depth = true;
            variant.setup.viewport = [0.0, 0.0, SIZE as f32, SIZE as f32, range[0], range[1]];
            let linked = LinkedValues {
                location: linked_location(&variant),
                clip_distances: 0,
            };
            for key in [&mut variant.setup.vs_key, &mut variant.setup.ps_key] {
                key.flags = VariantFlags::DEPTH_CLAMP;
                key.linked = linked;
            }
            variant.setup.helpers[1] = helper_block(
                variant.ps,
                &variant.setup.ps_key,
                &[(HelperValue::DepthRange, &floats(&range))],
            );
            for (back_end, texels) in [
                ("webgpu", ours(&mut gpu, &variant, ProvokingVertex::First)),
                ("webgl", ours(&mut gl, &variant, ProvokingVertex::Last)),
            ] {
                results.push(compare(
                    format!("depth_clamp {route} {back_end} against direct3d 11"),
                    &reference,
                    &texels,
                    Format::Depth32Float,
                    (SIZE as usize * SIZE as usize, 2),
                ));
            }
        }

        // The counting variant: blended One plus One into an rgba8unorm target (the grid's triangles do
        // not overlap, so each texel counts at most one; the WebGL 2 back end reads no float target
        // back), its sum equals real Direct3D 11's occlusion query of the plain program (which
        // discards where alpha is low).
        {
            let (_, count) = direct3d_11(
                &d3d,
                "vs_transform",
                "ps_textured",
                Format::Rgba8Unorm,
                [1.0, 0.75, 0.5, 1.5],
                true,
                &Reference {
                    count: true,
                    ..Default::default()
                },
            );
            let count = count.unwrap();
            let mut variant = plain.clone();
            variant.format = Format::Rgba8Unorm;
            variant.clear = [0; 4];
            variant.setup.ps_key.flags = VariantFlags::COUNTING;
            let one = BlendComponent {
                src: BlendFactor::One,
                dst: BlendFactor::One,
                op: BlendOp::Add,
            };
            variant.setup.blend = Some((one, one));
            for (back_end, texels) in [
                ("webgpu", ours(&mut gpu, &variant, ProvokingVertex::First)),
                ("webgl", ours(&mut gl, &variant, ProvokingVertex::Last)),
            ] {
                let sum: f32 = texels.chunks(4).map(|t| f32::from(t[0]) / 255.0).sum();
                let name = format!("counting {route} {back_end} against direct3d 11's query");
                eprintln!("{name}: {sum} counted, {count} queried");
                results.push(Compared {
                    name,
                    count: usize::from(sum as u64 != count),
                    largest: (sum as u64).abs_diff(count),
                    allowed: (0, 0),
                });
            }
        }

        // Texture level counts from the helper block (for WebGL 2, whose language cannot ask them),
        // against the native query on WebGPU and against real Direct3D 11.
        {
            let code = [
                bytecode(&format!("vs_transform.{route}")),
                bytecode(&format!("ps_levels.{route}")),
            ];
            let mut native = grid_draw(&code, dxil, &grid);
            native.setup.levels = 4;
            let reference = ours(&mut gpu, &native, ProvokingVertex::First);
            let (d3d11_levels, _) = direct3d_11(
                &d3d,
                "vs_transform",
                "ps_levels",
                Format::Rgba8Unorm,
                [1.0, 0.75, 0.5, 1.5],
                true,
                &Reference {
                    levels: 4,
                    ..Default::default()
                },
            );
            results.push(compare(
                format!("texture_info {route} webgpu-native against direct3d 11"),
                &d3d11_levels,
                &reference,
                Format::Rgba8Unorm,
                (0, 0),
            ));
            let mut variant = native.clone();
            variant.setup.ps_key.flags = VariantFlags::TEXTURE_INFO;
            variant.setup.helpers[1] = helper_block(
                variant.ps,
                &variant.setup.ps_key,
                &[(
                    HelperValue::TextureLevels { texture: 0 },
                    &4u32.to_le_bytes(),
                )],
            );
            for (back_end, texels) in [
                ("webgpu", ours(&mut gpu, &variant, ProvokingVertex::First)),
                ("webgl", ours(&mut gl, &variant, ProvokingVertex::Last)),
            ] {
                results.push(compare(
                    format!("texture_info {route} {back_end}"),
                    &reference,
                    &texels,
                    Format::Rgba8Unorm,
                    (0, 0),
                ));
            }
        }

        // The targets' sample count fixed by the variant, against real Direct3D 11 on one sample.
        if !dxil {
            let code = [bytecode("vs_transform.dxbc"), bytecode("ps_samples.dxbc")];
            let mut variant = grid_draw(&code, false, &grid);
            variant.setup.ps_key.samples = 1;
            let (reference, _) = direct3d_11(
                &d3d,
                "vs_transform",
                "ps_samples",
                Format::Rgba8Unorm,
                [1.0, 0.75, 0.5, 1.5],
                true,
                &Reference::default(),
            );
            for (back_end, texels) in [
                ("webgpu", ours(&mut gpu, &variant, ProvokingVertex::First)),
                ("webgl", ours(&mut gl, &variant, ProvokingVertex::Last)),
            ] {
                results.push(compare(
                    format!("samples dxbc {back_end} against direct3d 11"),
                    &reference,
                    &texels,
                    Format::Rgba8Unorm,
                    (0, 0),
                ));
            }
        }

        // Dual-source blending (target 0 times source 1's colour), natively on WebGPU through the
        // variant's blend sources, against real Direct3D 11 (fxc's build of ps_dual).
        {
            let code = [
                bytecode(&format!("vs_transform.{route}")),
                bytecode(&format!("ps_dual.{route}")),
            ];
            let mut variant = grid_draw(&code, dxil, &grid);
            variant.setup.ps_key.flags = VariantFlags::DUAL_SOURCE;
            let colour = BlendComponent {
                src: BlendFactor::One,
                dst: BlendFactor::Src1,
                op: BlendOp::Add,
            };
            let alpha = BlendComponent {
                src: BlendFactor::One,
                dst: BlendFactor::Src1Alpha,
                op: BlendOp::Add,
            };
            variant.setup.blend = Some((colour, alpha));
            let texels = ours(&mut gpu, &variant, ProvokingVertex::First);
            let (reference, _) = direct3d_11(
                &d3d,
                "vs_transform",
                "ps_dual",
                Format::Rgba8Unorm,
                [1.0, 0.75, 0.5, 1.5],
                true,
                &Reference {
                    blend: Some(D3D11_RENDER_TARGET_BLEND_DESC {
                        BlendEnable: true.into(),
                        SrcBlend: D3D11_BLEND_ONE,
                        DestBlend: D3D11_BLEND_SRC1_COLOR,
                        BlendOp: D3D11_BLEND_OP_ADD,
                        SrcBlendAlpha: D3D11_BLEND_ONE,
                        DestBlendAlpha: D3D11_BLEND_SRC1_ALPHA,
                        BlendOpAlpha: D3D11_BLEND_OP_ADD,
                        RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
                    }),
                    ..Default::default()
                },
            );
            results.push(compare(
                format!("dual_source {route} webgpu against direct3d 11"),
                &reference,
                &texels,
                Format::Rgba8Unorm,
                (0, 0),
            ));
        }
    }
    // WebGL 2 without EXT_clip_control, the program rewriting z as 2z - w, against real Direct3D
    // 11's depth buffer. The back end's GL context is made current when it is made and stays so, so
    // this second one is made after the first one's last draw.
    drop(gl);
    let mut gl_no_clip = webgl_without(&["EXT_clip_control"]);
    for (route, dxil) in [("dxbc", false), ("dxil", true)] {
        let code = [
            bytecode(&format!("vs_transform.{route}")),
            bytecode(&format!("ps_textured.{route}")),
        ];
        let mut variant = grid_draw(&code, dxil, &grid);
        variant.setup.depth = true;
        variant.setup.vs_key.flags = VariantFlags::CLIP_DEPTH_REWRITE;
        variant.setup.depth_minus_one = true;
        let (reference, _) = direct3d_11(
            &d3d,
            "vs_transform",
            "ps_textured",
            Format::Rgba8Unorm,
            [1.0, 0.75, 0.5, 1.5],
            true,
            &Reference {
                depth: Some([0.0, 1.0]),
                ..Default::default()
            },
        );
        let texels = ours(&mut gl_no_clip, &variant, ProvokingVertex::Last);
        results.push(compare(
            format!("clip_depth {route} webgl-without-clip-control against direct3d 11"),
            &reference,
            &texels,
            Format::Depth32Float,
            (SIZE as usize * SIZE as usize, 2),
        ));
    }

    let mut failures = Vec::new();
    for r in &results {
        eprintln!(
            "{}: {} of {} texels differ, largest difference {} (allowed {} by {})",
            r.name,
            r.count,
            SIZE * SIZE,
            r.largest,
            r.allowed.0,
            r.allowed.1
        );
        if r.count > r.allowed.0 || r.largest > r.allowed.1 {
            failures.push(r.name.clone());
        }
    }
    assert!(failures.is_empty(), "differ: {failures:?}");
}

/// Inputs evaluated at an offset, at a sample and at the centroid, an input interpolated per sample
/// evaluated at an offset, and the position of a program run per sample, drawn through the WebGPU
/// back end at one and four samples against real Direct3D 11 (fxc's build; dxc's is refused as
/// `InterpolateAt`). The shader sends each value through a steep wave, so a place a sixteenth of a
/// pixel off moves a texel by several steps; the allowance is the filtering one, for the waves'
/// rounding and the resolve.
#[test]
fn inputs_evaluated_elsewhere_match_direct3d() {
    let mut gpu = webgpu();
    let d3d = wcontainers_graphics_test::d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let grid = grid();
    let code = [bytecode("vs_evaluate.dxbc"), bytecode("ps_evaluate.dxbc")];
    let dxil = convert(
        Source::Dxil(&bytecode("ps_evaluate.dxil")),
        &VariantKey::default(),
        None,
    );
    assert!(
        matches!(
            dxil,
            Err(wcontainers_shaders::ConvertError::Unsupported(
                "InterpolateAt"
            ))
        ),
        "{dxil:?}"
    );
    let mut results = Vec::new();
    for samples in [1u8, LAYER_SAMPLES as u8] {
        let (reference, _) = direct3d_11(
            &d3d,
            "vs_evaluate",
            "ps_evaluate",
            Format::Rgba8Unorm,
            [1.0, 0.75, 0.5, 1.5],
            true,
            &Reference {
                samples: samples.into(),
                ..Default::default()
            },
        );
        let mut ours_draw = grid_draw(&code, false, &grid);
        ours_draw.setup.samples = samples.into();
        ours_draw.setup.ps_key.samples = samples;
        let texels = ours(&mut gpu, &ours_draw, ProvokingVertex::First);
        results.push(compare(
            format!("evaluate {samples} samples webgpu against direct3d 11"),
            &reference,
            &texels,
            Format::Rgba8Unorm,
            (
                SIZE as usize * SIZE as usize,
                (FILTER_ALLOWANCE * f32::from(u8::MAX)).ceil() as u64,
            ),
        ));
    }
    let mut failures = Vec::new();
    for r in &results {
        eprintln!(
            "{}: {} of {} texels differ, largest difference {} (allowed {} by {})",
            r.name,
            r.count,
            SIZE * SIZE,
            r.largest,
            r.allowed.0,
            r.allowed.1
        );
        if r.count > r.allowed.0 || r.largest > r.allowed.1 {
            failures.push(r.name.clone());
        }
    }
    assert!(failures.is_empty(), "differ: {failures:?}");
}

/// Formats the typed buffer views of `ps_texels` take: textures of every back end, so both routes
/// read them.
const PIXEL_FLOATS: [Format; 14] = [
    Format::R8Unorm,
    Format::R8Snorm,
    Format::Rg8Unorm,
    Format::Rg8Snorm,
    Format::Rgba8Unorm,
    Format::Rgba8Snorm,
    Format::R16Float,
    Format::Rg16Float,
    Format::Rgba16Float,
    Format::R32Float,
    Format::Rg32Float,
    Format::Rgba32Float,
    Format::Rgb10a2Unorm,
    Format::Rg11b10Float,
];
const PIXEL_UINTS: [Format; 10] = [
    Format::R8Uint,
    Format::Rg8Uint,
    Format::Rgba8Uint,
    Format::R16Uint,
    Format::Rg16Uint,
    Format::Rgba16Uint,
    Format::R32Uint,
    Format::Rg32Uint,
    Format::Rgba32Uint,
    Format::Rgb10a2Uint,
];

/// Typed buffers read by a pixel program through storage buffers (WebGPU) and through textures
/// (WebGL 2 and WebGPU), against real Direct3D 11's typed buffer views, over seeded formats, bytes,
/// first elements and lengths (`WCONTAINERS_TEXEL_SEED`, default 1, and `WCONTAINERS_TEXEL_CASES`
/// seeds, default 8).
#[test]
fn typed_buffers_draw_as_direct3d_does() {
    let mut gpu = webgpu();
    let mut gl = webgl();
    let d3d = wcontainers_graphics_test::d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let seed: u64 = std::env::var("WCONTAINERS_TEXEL_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_TEXEL_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(8);
    let supported = |f: Format| {
        let need = D3D11_FORMAT_SUPPORT_BUFFER.0 as u32;
        unsafe { d3d.device.CheckFormatSupport(dxgi(f)) }.is_ok_and(|s| s & need == need)
    };
    let floats: Vec<Format> = PIXEL_FLOATS.into_iter().filter(|&f| supported(f)).collect();
    let uints: Vec<Format> = PIXEL_UINTS.into_iter().filter(|&f| supported(f)).collect();
    let grid = grid();
    // The same programs compiled by fxc (DXBC) and dxc (DXIL), each drawn by both routes.
    let code = [bytecode("vs_transform.dxbc"), bytecode("ps_texels.dxbc")];
    let code_dxil = [bytecode("vs_transform.dxil"), bytecode("ps_texels.dxil")];
    let mut results = Vec::new();
    for seed in seed..seed + seeds {
        let mut s = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let mut next = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 16) as u32
        };
        let formats = [
            floats[next() as usize % floats.len()],
            uints[next() as usize % uints.len()],
        ];
        let texel_buffers: Vec<TexelData> = formats
            .iter()
            .map(|&format| {
                let first = next() % 8;
                let count = 100 + next() % 29;
                let elements = (first + count + 3).next_multiple_of(4);
                let bytes = (0..elements * format.block().2)
                    .map(|_| next() as u8)
                    .collect();
                TexelData {
                    format,
                    bytes,
                    first,
                    count,
                }
            })
            .collect();
        let (reference, _) = direct3d_11(
            &d3d,
            "vs_transform",
            "ps_texels",
            Format::Rgba32Uint,
            [1.0, 0.75, 0.5, 1.5],
            true,
            &Reference {
                texel_buffers: texel_buffers.clone(),
                ..Default::default()
            },
        );
        for (code, dxil) in [(&code, false), (&code_dxil, true)] {
            let mut draw = grid_draw(code, dxil, &grid);
            draw.format = Format::Rgba32Uint;
            draw.clear = [0; 4];
            draw.setup.texel_buffers = texel_buffers.clone();
            for (route, back_end) in [
                (TexelBufferRoute::Storage, "webgpu"),
                (TexelBufferRoute::Texture, "webgpu"),
                (TexelBufferRoute::Texture, "webgl"),
            ] {
                draw.setup.ps_key = VariantKey {
                    texel_buffers: route,
                    texel_formats: vec![(0, formats[0]), (1, formats[1])],
                    ..Default::default()
                };
                let texels = match back_end {
                    "webgpu" => ours(&mut gpu, &draw, ProvokingVertex::First),
                    _ => ours(&mut gl, &draw, ProvokingVertex::Last),
                };
                let source = if dxil { "dxil" } else { "dxbc" };
                results.push(compare(
                    format!("typed buffers seed {seed} {formats:?} {source} {route:?} {back_end}"),
                    &reference,
                    &texels,
                    Format::Rgba32Uint,
                    (0, 0),
                ));
            }
        }
    }
    let mut failed = 0;
    for r in &results {
        let ok = r.count <= r.allowed.0 && r.largest <= r.allowed.1;
        failed += !ok as usize;
        eprintln!(
            "{}: {} texels differ, largest {}{}",
            r.name,
            r.count,
            r.largest,
            if ok { "" } else { " FAILED" }
        );
    }
    eprintln!("{} comparisons, {failed} failed", results.len());
    assert_eq!(failed, 0);
}

/// A pixel program's layer and viewport index: real Direct3D 11 draws the full-screen triangle into
/// each of four layers through `gs_layers` (layer n through viewport n % 2); ours draws each layer
/// alone, with the run's layer and viewport as helper values, as a front splitting the draw does.
#[test]
fn layer_and_viewport_inputs_read_the_draws_run() {
    use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
    use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;
    use windows::core::Interface;
    const LAYERS: u32 = 4;
    let viewports = [
        [0.0f32, 0.0, SIZE as f32, SIZE as f32],
        [8.0, 4.0, 40.0, 48.0],
    ];
    let mut gpu = webgpu();
    let d3d = wcontainers_graphics_test::d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let (vs, ps, gs) = (
        bytecode("vs_fullscreen.dxbc"),
        bytecode("ps_layer.dxbc"),
        bytecode("gs_layers.dxbc"),
    );
    d3d.program(&vs, &ps, &[]).unwrap();
    let geometry = d3d.gs(&gs).unwrap();
    let mut desc = wcontainers_graphics_test::d3d11::tex2d(
        SIZE,
        SIZE,
        DXGI_FORMAT_R8G8B8A8_UNORM,
        D3D11_BIND_RENDER_TARGET,
    );
    desc.ArraySize = LAYERS;
    let target = d3d.texture2d(&desc, &[]).unwrap();
    let rtv = d3d.rtv(&target.cast().unwrap(), None).unwrap();
    let rects: Vec<D3D11_VIEWPORT> = viewports
        .iter()
        .map(|v| D3D11_VIEWPORT {
            TopLeftX: v[0],
            TopLeftY: v[1],
            Width: v[2],
            Height: v[3],
            MinDepth: 0.0,
            MaxDepth: 1.0,
        })
        .collect();
    unsafe {
        let c = &d3d.context;
        c.GSSetShader(&geometry, None);
        c.ClearRenderTargetView(&rtv, &CLEAR);
        c.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
        c.RSSetViewports(Some(&rects));
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        c.Draw(3, 0);
        c.GSSetShader(None, None);
    }
    let mut failed = Vec::new();
    for layer in 0..LAYERS {
        let reference = d3d.read(&target, 0, layer).unwrap().to_rgba8().into_raw();
        let viewport = layer % 2;
        let v = viewports[viewport as usize];
        let block = helper_block(
            Source::Dxbc(&ps),
            &VariantKey::default(),
            &[
                (HelperValue::Layer, &layer.to_le_bytes()),
                (HelperValue::Viewport, &viewport.to_le_bytes()),
            ],
        );
        let mut setup = Setup::plain(SIZE);
        setup.helpers = [Vec::new(), block];
        setup.viewport = [v[0], v[1], v[2], v[3], 0.0, 1.0];
        let draw = Draw {
            vs: Source::Dxbc(&vs),
            ps: Source::Dxbc(&ps),
            format: Format::Rgba8Unorm,
            size: SIZE,
            vertices: &[],
            stride: 0,
            attributes: &[],
            count: 3,
            constants: constants([1.0; 4]),
            clear: CLEAR.map(f32::to_bits),
            setup,
        };
        let made = ours(&mut gpu, &draw, ProvokingVertex::First);
        let c = compare(
            format!("layer {layer}"),
            &reference,
            &made,
            Format::Rgba8Unorm,
            (0, 0),
        );
        eprintln!(
            "{}: {} texels apart, largest {}",
            c.name, c.count, c.largest
        );
        if c.count > 0 {
            failed.push(c.name);
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}

/// A multisampled array texture (`VariantKey::texture_layers`): real Direct3D 11 fills each layer of a
/// `Texture2DMSArray` with `ps_fill` and reads it through `ps_msarray` (layers picked at run time,
/// past the count too, and its dimensions); ours fills a texture per layer and reads them through
/// the variant, by both routes.
#[test]
fn multisampled_array_layers_read_as_direct3d_does() {
    use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC};
    use windows::core::Interface;
    const LAYERS: u32 = 3;
    let d3d = wcontainers_graphics_test::d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let vs = bytecode("vs_fullscreen.dxbc");
    let mut desc = wcontainers_graphics_test::d3d11::tex2d(
        TEXTURE_SIZE,
        TEXTURE_SIZE,
        DXGI_FORMAT_R8G8B8A8_UNORM,
        D3D11_BIND_FLAG(D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0),
    );
    desc.ArraySize = LAYERS;
    desc.SampleDesc = DXGI_SAMPLE_DESC {
        Count: LAYER_SAMPLES,
        Quality: 0,
    };
    let layered = d3d.texture2d(&desc, &[]).unwrap();
    d3d.program(&vs, &bytecode("ps_fill.dxbc"), &[]).unwrap();
    d3d.viewport(TEXTURE_SIZE, TEXTURE_SIZE);
    let mut kept = Vec::new();
    for layer in 0..LAYERS {
        let mut view = D3D11_RENDER_TARGET_VIEW_DESC {
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2DMSARRAY,
            ..Default::default()
        };
        view.Anonymous.Texture2DMSArray = D3D11_TEX2DMS_ARRAY_RTV {
            FirstArraySlice: layer,
            ArraySize: 1,
        };
        let rtv = d3d.rtv(&layered.cast().unwrap(), Some(&view)).unwrap();
        let constants = d3d
            .immutable(&fill_constants(layer), D3D11_BIND_CONSTANT_BUFFER)
            .unwrap();
        unsafe {
            let c = &d3d.context;
            c.PSSetConstantBuffers(0, Some(&[Some(constants.clone())]));
            c.ClearRenderTargetView(&rtv, &[0.0; 4]);
            c.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
            c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            c.Draw(3, 0);
        }
        kept.push((rtv, constants));
    }
    let srv = d3d.srv(&layered.cast().unwrap(), None).unwrap();
    let (target, rtv) = d3d.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM).unwrap();
    d3d.program(&vs, &bytecode("ps_msarray.dxbc"), &[]).unwrap();
    d3d.viewport(SIZE, SIZE);
    unsafe {
        let c = &d3d.context;
        c.ClearRenderTargetView(&rtv, &CLEAR);
        c.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
        c.PSSetShaderResources(0, Some(&[Some(srv.clone())]));
        c.Draw(3, 0);
    }
    let reference = d3d.read(&target, 0, 0).unwrap().to_rgba8().into_raw();

    let mut gpu = webgpu();
    let mut failed = Vec::new();
    for route in ["dxbc", "dxil"] {
        let ps = bytecode(&format!("ps_msarray.{route}"));
        let source = match route {
            "dxil" => Source::Dxil(&ps),
            _ => Source::Dxbc(&ps),
        };
        assert_eq!(
            convert(source, &VariantKey::default(), None).unwrap_err(),
            wcontainers_shaders::ConvertError::Unsupported("MultisampledArrayTexture")
        );
        let mut setup = Setup::plain(SIZE);
        setup.layers = LAYERS;
        setup.ps_key.texture_layers = vec![(0, LAYERS)];
        let draw = Draw {
            vs: Source::Dxbc(&vs),
            ps: source,
            format: Format::Rgba8Unorm,
            size: SIZE,
            vertices: &[],
            stride: 0,
            attributes: &[],
            count: 3,
            constants: constants([1.0; 4]),
            clear: CLEAR.map(f32::to_bits),
            setup,
        };
        let made = ours(&mut gpu, &draw, ProvokingVertex::First);
        let c = compare(
            format!("multisampled array {route}"),
            &reference,
            &made,
            Format::Rgba8Unorm,
            (0, 0),
        );
        eprintln!(
            "{}: {} texels apart, largest {}",
            c.name, c.count, c.largest
        );
        if c.count > 0 {
            failed.push(c.name);
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}
