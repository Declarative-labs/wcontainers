//! Views in a texture's other sRGB form where that form lacks a usage the texture has: an sRGB view
//! of a texture that also allows storage (Direct3D 12 reads such a shader resource view in its UNORM
//! texture's sRGB twin), read as a texture made sRGB reads, also after the texture is renamed.
//! Shared by tests/bindings.rs and the browser page.

use crate::draw;
use crate::renames::{Kit, begin, draw_at, finish, kit};
use anyhow::{Result, ensure};
use wcontainers_gpu::{
    Backend, BindingEntry, BindingResource, DeviceStatus, Extent, Format, Origin, StageSet,
    TextureRegion, TextureUsage, TextureViewDesc, ViewDimension, WriteMode,
};
use wcontainers_gpu_webgpu::WebGpu;

/// A full-target triangle drawing, at each pixel, the texel of row 0 its column wraps to.
const WRAPPED_LOAD_WGSL: &str = r#"
@group(1) @binding(32) var t: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn ps(@builtin(position) p: vec4f) -> @location(0) vec4f {
    return textureLoad(t, vec2u(u32(p.x) % textureDimensions(t).x, 0u), 0);
}
"#;

/// The binding the program reads its texture at.
const TEXTURE_BINDING: u32 = 32;

/// The texels each texture holds, written once before and once after the rename.
const ROUNDS: [[[u8; 4]; 4]; 2] = [
    [
        [0, 0, 0, 255],
        [64, 32, 16, 255],
        [188, 100, 7, 128],
        [255, 255, 255, 255],
    ],
    [
        [13, 200, 99, 0],
        [128, 128, 128, 128],
        [250, 1, 77, 255],
        [33, 66, 99, 255],
    ],
];

/// Texels of one written row.
const ROW: u32 = ROUNDS[0].len() as u32;

/// Channels of a texel.
const CHANNELS: usize = ROUNDS[0][0].len();

/// Bytes of an `Rgba8Unorm` texel.
const TEXEL: u32 = size_of::<[u8; CHANNELS]>() as u32;

/// Bytes of an `Rgba32Float` texel.
const FLOAT_TEXEL: u32 = size_of::<[f32; 4]>() as u32;

/// The view of the whole of a one-level 2D texture in `format`.
fn whole_view(format: Format) -> TextureViewDesc {
    TextureViewDesc {
        format,
        dimension: ViewDimension::D2,
        base_mip: 0,
        mip_count: 1,
        base_layer: 0,
        layer_count: 1,
        stencil: false,
    }
}

fn write_row(b: &mut WebGpu, texture: &<WebGpu as Backend>::Texture, texels: &[[u8; CHANNELS]]) {
    b.write_texture(
        &TextureRegion {
            texture,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        },
        Extent {
            width: ROW,
            height: 1,
            depth_or_layers: 1,
        },
        texels.as_flattened(),
        ROW * TEXEL,
        1,
        WriteMode::Discard,
    );
}

/// Draws `view` of `texture` into the first `ROW` pixels of a fresh float target, writes the next
/// round's texels with `Discard` (which renames the texture the recorded draws read) and draws the
/// view again into the next `ROW`; returns the target's texels and the renames it took.
fn draw_both_rounds(
    b: &mut WebGpu,
    texture: &<WebGpu as Backend>::Texture,
    view: &<WebGpu as Backend>::TextureView,
) -> Result<(Vec<u8>, u64)> {
    let width = ROW * ROUNDS.len() as u32;
    let (target, target_view) = draw::texture_2d(
        b,
        Format::Rgba32Float,
        width,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    )?;
    let k = Kit {
        program: draw::program(b, WRAPPED_LOAD_WGSL)?,
        target,
        view: target_view,
        ..kit(b, Format::Rgba32Float, width, &draw::opaque())
    };
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: TEXTURE_BINDING,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(view),
        }])
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    write_row(b, texture, &ROUNDS[0]);
    let before = b.counters().texture_renames;
    begin(b, &k, [0.0; 4]);
    for (round, texels) in ROUNDS.iter().enumerate() {
        if round > 0 {
            write_row(b, texture, texels);
        }
        for x in 0..ROW {
            draw_at(b, &k, &set, Some(round as u32 * ROW + x));
        }
    }
    let got = finish(b, &k, FLOAT_TEXEL);
    ensure!(
        b.status() == DeviceStatus::Ok,
        "device lost: {:?}",
        b.status()
    );
    Ok((got, b.counters().texture_renames - before))
}

/// An `Rgba8UnormSrgb` view of an `Rgba8Unorm` texture that allows storage reads each texel as an
/// `Rgba8UnormSrgb` texture holding the same bytes does, bit for bit, before and after a `Discard`
/// write renames the texture; its colour channels strictly between 0 and 1 read below their linear
/// value (decoded from sRGB). The texture's own view stays usable. One line, or the failed check.
pub fn srgb_views_of_storage_textures(b: &mut WebGpu) -> Result<String> {
    let (texture, _own) = draw::texture_2d(
        b,
        Format::Rgba8Unorm,
        ROW,
        1,
        TextureUsage::SAMPLE | TextureUsage::STORAGE | TextureUsage::COPY_DST,
    )?;
    let srgb = b
        .create_view(&texture, &whole_view(Format::Rgba8UnormSrgb))
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let (reference, reference_view) = draw::texture_2d(
        b,
        Format::Rgba8UnormSrgb,
        ROW,
        1,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
    )?;
    let (got, renames) = draw_both_rounds(b, &texture, &srgb)?;
    let (want, _) = draw_both_rounds(b, &reference, &reference_view)?;
    ensure!(renames > 0, "the Discard write did not rename the texture");
    let floats: Vec<f32> = bytemuck::pod_collect_to_vec(&got);
    let wanted: Vec<f32> = bytemuck::pod_collect_to_vec(&want);
    let bytes = ROUNDS.as_flattened().as_flattened();
    for (i, ((&read, &byte), &reference)) in floats.iter().zip(bytes).zip(&wanted).enumerate() {
        ensure!(
            read.to_bits() == reference.to_bits(),
            "channel {i}: the sRGB view read {read}, the sRGB texture {reference}"
        );
        let linear = f32::from(byte) / f32::from(u8::MAX);
        let colour = i % CHANNELS != CHANNELS - 1;
        ensure!(
            !colour || byte == 0 || byte == u8::MAX || read < linear,
            "channel {i}: {read} not decoded from sRGB (linear {linear})"
        );
    }
    Ok(format!(
        "sRGB views of storage textures: {} channels equal to an sRGB texture's, bit for bit, \
         across {renames} rename",
        floats.len()
    ))
}
