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
    BlendComponent, BlendFactor, BlendOp, Format, HelperValue, ProvokingVertex, VariantFlags,
    VariantKey, VertexFormat,
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
