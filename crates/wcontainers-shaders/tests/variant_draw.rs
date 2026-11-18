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
    BlendComponent, BlendFactor, BlendOp, DrawBaseSource, Format, HelperValue, ProvokingVertex,
    VariantFlags, VariantKey, VertexFormat,
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
        {
            let code = [
                bytecode(&format!("vs_clip.{route}")),
                bytecode(&format!("ps_textured.{route}")),
            ];
            let native = grid_draw(&code, dxil, &grid);
            let tint = [1.0, 0.75, 0.5, 1.5];
            let (reference, _) = direct3d_11(
                &d3d,
                "vs_clip",
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
            eprintln!("clip distances {route}: they change {changed} of Direct3D's texels");
            assert!(changed > 0);
            let texels = ours(&mut vulkan, &native, ProvokingVertex::First);
            results.push(compare(
                format!("clip distances {route} webgpu-native (vulkan) against direct3d 11"),
                &reference,
                &texels,
                Format::Rgba8Unorm,
                (0, 0),
            ));
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
