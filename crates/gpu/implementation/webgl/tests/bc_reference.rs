//! Block-compressed textures loaded texel by texel through this back end on ANGLE, checked by the
//! rig's own rule against real Direct3D 11's pictures of the same blocks (the rig's `textures-bc`
//! and `textures-bc67`): through GL's compressed formats with every extension, and decoded on the
//! host (`BlockDecoded`) with the four BC extensions off. Windows only; ANGLE's directory as in
//! tests/suite.rs.
#![cfg(windows)]

#[path = "../examples/common/kit.rs"]
#[allow(dead_code)]
mod kit;

use rand::RngCore;
use std::path::{Path, PathBuf};
use wcontainers_gpu::{
    caps::*, draw::*, error::*, format::*, helpers::*, occlusion::*, passes::*, reflect::*,
    resource::*, shader::*, slow::*, state::*, *,
};
use wcontainers_gpu_webgl::Gl;
use wcontainers_graphics_test::{check, compare, load, save, scene::CLEAR};

/// The rig's block-compressed formats in its order, with the bytes of a block of each.
const BC: [(Format, usize); 16] = [
    (Format::Bc1RgbaUnorm, 8),
    (Format::Bc1RgbaUnormSrgb, 8),
    (Format::Bc2RgbaUnorm, 16),
    (Format::Bc3RgbaUnorm, 16),
    (Format::Bc3RgbaUnormSrgb, 16),
    (Format::Bc4RUnorm, 8),
    (Format::Bc4RSnorm, 8),
    (Format::Bc5RgUnorm, 16),
    (Format::Bc5RgSnorm, 16),
    (Format::Bc6hRgbUfloat, 16),
    (Format::Bc6hRgbFloat, 16),
    (Format::Bc7RgbaUnorm, 16),
    (Format::Bc7RgbaUnormSrgb, 16),
    (Format::Bc2RgbaUnormSrgb, 16),
    (Format::Bc7RgbaUnorm, 16),
    (Format::Bc6hRgbUfloat, 16),
];

const BC_EXTENSIONS: [&str; 4] = [
    "WEBGL_compressed_texture_s3tc",
    "WEBGL_compressed_texture_s3tc_srgb",
    "EXT_texture_compression_rgtc",
    "EXT_texture_compression_bptc",
];

/// Whether a format is drawn in the rig's `bc67` part.
fn bc67(format: Format) -> bool {
    matches!(
        format,
        Format::Bc6hRgbUfloat
            | Format::Bc6hRgbFloat
            | Format::Bc7RgbaUnorm
            | Format::Bc7RgbaUnormSrgb
    )
}

/// The rig's random blocks of each format: 8 by 8 blocks from seed 11, in the order of `BC`.
fn blocks() -> Vec<Vec<u8>> {
    let mut random: rand_chacha::ChaCha8Rng = rand::SeedableRng::seed_from_u64(11);
    BC.iter()
        .map(|&(_, block)| {
            let mut blocks = vec![0u8; 8 * 8 * block];
            random.fill_bytes(&mut blocks);
            blocks
        })
        .collect()
}

/// The texel under the fragment, of the 32 by 32 texture its cell shows.
const LOAD: &str = "
@group(0) @binding(32) var t: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((v & 1u) << 2u) - 1.0, f32((v & 2u) << 1u) - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(t, vec2<i32>(p.xy) % 32, 0);
}
";

fn webgl(off: &[&str]) -> Gl {
    let dir = std::env::var_os("WEBGL_ANGLE_DIR").map_or_else(
        || PathBuf::from(r"C:\wc\tools\angle-mozangle-0.7.1"),
        PathBuf::from,
    );
    let b = Gl::on_angle(&dir, off).expect("ANGLE's OpenGL ES 3.0 context");
    assert!(b.last_log().is_empty(), "{}", b.last_log());
    b
}

/// The rig's picture of one part: a 128 by 128 `rgba32float` target cleared to the rig's colour, each
/// format's texture loaded into its 32-pixel cell, four cells to a row.
fn draw(b: &mut Gl, newer: bool) -> image::DynamicImage {
    let k = kit::Kit::new(b);
    let program = kit::program(b, LOAD, "vs", "fs");
    let all = blocks();
    let mut sets = Vec::new();
    for ((format, block), bytes) in BC.iter().zip(&all).filter(|((f, _), _)| bc67(*f) == newer) {
        let usage = TextureUsage::SAMPLE | TextureUsage::COPY_DST;
        let t = kit::texture(
            b,
            TextureDimension::D2,
            *format,
            [32, 32, 1],
            1,
            usage,
            false,
        );
        let region = TextureRegion {
            texture: &t,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        };
        let extent = Extent {
            width: 32,
            height: 32,
            depth_or_layers: 1,
        };
        b.write_texture(
            &region,
            extent,
            bytes,
            8 * *block as u32,
            8,
            WriteMode::Ordered,
        );
        let view = kit::view(b, &t, *format, ViewDimension::D2, 0, 1, 0, 1);
        let set = b
            .create_binding_set(&[BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&view),
            }])
            .expect("create_binding_set");
        sets.push((t, view, set));
    }
    let f = Format::Rgba32Float;
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let target = kit::texture(b, TextureDimension::D2, f, [128, 128, 1], 1, usage, false);
    let view = kit::target(b, &target, f, 0, 0);
    b.begin_pass(&kit::pass(&view, CLEAR.map(f32::to_bits), None));
    for (i, (_, _, set)) in sets.iter().enumerate() {
        let i = i as f32;
        let cell = [kit::viewport(
            (i % 4.0) * 32.0,
            (i / 4.0).floor() * 32.0,
            32.0,
            32.0,
        )];
        b.draw(&kit::call(
            &k,
            &program,
            Some(set),
            &cell,
            Topology::TriangleList,
            3,
        ));
    }
    b.end_pass();
    let got = kit::read(b, &target, 0, 0, [128, 128, 1], 16, false);
    assert_eq!(b.status(), DeviceStatus::Ok, "{}", b.last_log());
    let floats = got
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect();
    image::Rgba32FImage::from_raw(128, 128, floats)
        .expect("128 by 128 texels")
        .into()
}

fn references() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../graphics-test/references/d3d11")
}

/// Checks both parts drawn with `off` against Direct3D 11's pictures by the rig's rule, and prints
/// each format's largest difference from them.
fn route(name: &str, off: &[&str]) {
    let mut b = webgl(off);
    let decoded = off.contains(&BC_EXTENSIONS[0]);
    assert_eq!(
        b.caps().features.contains(Features::BLOCK_COMPRESSION),
        !decoded
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("bc-reference")
        .join(name);
    let (run, reference) = (dir.join("run"), dir.join("reference"));
    for d in [&run, &reference] {
        std::fs::create_dir_all(d).expect("a scratch directory");
    }
    let lines = std::fs::read_to_string(references().join(check::TOLERANCES)).expect("tolerances");
    let mut kept = String::new();
    for (part, newer) in [("textures-bc.exr", false), ("textures-bc67.exr", true)] {
        let ours = draw(&mut b, newer);
        save(&ours, &run.join(part)).expect("saved");
        std::fs::copy(references().join(part), reference.join(part)).expect("copied");
        kept.extend(
            lines
                .lines()
                .filter(|l| l.starts_with(part))
                .map(|l| format!("{l}\n")),
        );
        let theirs = load(&references().join(part)).expect("the reference");
        let formats = BC.iter().filter(|(f, _)| bc67(*f) == newer);
        let (a, o) = (theirs.to_rgba32f(), ours.to_rgba32f());
        for (i, (format, _)) in formats.enumerate() {
            let (x0, y0) = ((i % 4) as u32 * 32, (i / 4) as u32 * 32);
            let mut largest = 0f32;
            for y in y0..y0 + 32 {
                for x in x0..x0 + 32 {
                    let (p, q) = (a.get_pixel(x, y).0, o.get_pixel(x, y).0);
                    for c in 0..4 {
                        largest = largest.max((p[c] - q[c]).abs());
                    }
                }
            }
            println!(
                "{name}: {format:?} largest difference from Direct3D 11 {largest:e} ({:.2} 8-bit steps)",
                largest * 255.0
            );
        }
        let d = compare::compare(&theirs, &ours, 0.0).expect("compared");
        println!(
            "{name}: {part} exactly equal but {} of {} pixels",
            d.differing, d.pixels
        );
    }
    std::fs::write(reference.join(check::TOLERANCES), kept).expect("tolerances written");
    let verdicts = check::check_dir(&reference, &run, |f| f.ends_with(".exr")).expect("checked");
    let failed: Vec<String> = verdicts
        .iter()
        .inspect(|v| println!("{name}: {} {:?} {}", v.file, v.state, v.detail))
        .filter(|v| v.state != check::State::Pass)
        .map(|v| format!("{} {}", v.file, v.detail))
        .collect();
    assert!(failed.is_empty(), "{name}: {failed:#?}");
}

#[test]
fn compressed_in_gl_equals_direct3d_11() {
    route("compressed", &[]);
}

#[test]
fn decoded_on_the_host_equals_direct3d_11() {
    route("decoded", &BC_EXTENSIONS);
}
