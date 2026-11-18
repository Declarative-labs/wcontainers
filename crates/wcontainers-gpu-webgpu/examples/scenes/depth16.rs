//! Copies between `depth16unorm` and the colour formats of Direct3D's `R16_TYPELESS` group: shared
//! by tests/copies.rs and the browser page.

use wcontainers_gpu::{
    Backend, DeviceStatus, Extent, Format, FormatUse, Origin, ReadSource, TextureRegion,
    TextureUsage, WriteMode,
};
use wcontainers_gpu_webgpu::WebGpu;

use crate::draw;

type Tex = <WebGpu as Backend>::Texture;

const W: u32 = 8;
const H: u32 = 4;

fn at(texture: &Tex, x: u32, y: u32) -> TextureRegion<'_, WebGpu> {
    TextureRegion {
        texture,
        mip: 0,
        layer: 0,
        origin: Origin { x, y, z: 0 },
        stencil: false,
    }
}

fn extent(width: u32, height: u32) -> Extent {
    Extent {
        width,
        height,
        depth_or_layers: 1,
    }
}

fn read(b: &mut WebGpu, texture: &Tex) -> Result<Vec<u8>, String> {
    let rb = b
        .read_back(&ReadSource::Texture {
            region: at(texture, 0, 0),
            extent: extent(W, H),
        })
        .map_err(|e| format!("read back: {e:?}"))?;
    b.submit();
    b.wait_read(&rb);
    let mut out = vec![0; (W * H * 2) as usize];
    b.finish_read(rb, &mut out);
    Ok(out)
}

/// `whole` after a copy of its 3x2 texels at (0, 0) to (1, 1).
fn moved(whole: &[u8]) -> Vec<u8> {
    let mut want = whole.to_vec();
    for y in 0..2u32 {
        for x in 0..3u32 {
            let (s, d) = (
                ((y * W + x) * 2) as usize,
                (((y + 1) * W + x + 1) * 2) as usize,
            );
            want[d..d + 2].copy_from_slice(&whole[s..s + 2]);
        }
    }
    want
}

/// Every colour format of the group the device has copied into `depth16unorm` and back, whole and
/// then 3x2 texels at (0, 0) to (1, 1) (the copies of Wine's test_format_compatibility, whose
/// Direct3D 11 front holds a typeless texture not bound as depth as `r16float`): the bits are kept.
/// One line naming the formats, or the first difference.
pub fn group_copies(b: &mut WebGpu) -> Result<String, String> {
    let bytes: Vec<u8> = (0..W * H * 2).map(|i| (i * 37 + 11) as u8).collect();
    let fill = [0x5a; (W * H * 2) as usize];
    let mut copied = Vec::new();
    for colour in [
        Format::R16Unorm,
        Format::R16Uint,
        Format::R16Float,
        Format::R16Snorm,
        Format::R16Sint,
    ] {
        if !b.caps().formats.get(colour).contains(FormatUse::SAMPLE) {
            continue;
        }
        let usage = TextureUsage::SAMPLE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
        let made = |b: &mut WebGpu, format: Format, usage: TextureUsage| {
            draw::texture_2d(b, format, W, H, usage)
                .map(|t| t.0)
                .map_err(|e| format!("{format:?}: {e}"))
        };
        let src = made(b, colour, usage)?;
        b.write_texture(
            &at(&src, 0, 0),
            extent(W, H),
            &bytes,
            W * 2,
            H,
            WriteMode::Ordered,
        );
        let depth = made(b, Format::Depth16Unorm, usage | TextureUsage::RENDER)?;
        b.write_texture(
            &at(&depth, 0, 0),
            extent(W, H),
            &fill,
            W * 2,
            H,
            WriteMode::Ordered,
        );
        b.copy_texture(&at(&src, 0, 0), &at(&depth, 0, 0), extent(W, H));
        b.copy_texture(&at(&src, 0, 0), &at(&depth, 1, 1), extent(3, 2));
        let in_depth = moved(&bytes);
        if read(b, &depth)? != in_depth {
            return Err(format!("{colour:?} into depth16unorm changed the bits"));
        }
        let dst = made(b, colour, usage)?;
        b.write_texture(
            &at(&dst, 0, 0),
            extent(W, H),
            &fill,
            W * 2,
            H,
            WriteMode::Ordered,
        );
        b.copy_texture(&at(&depth, 0, 0), &at(&dst, 0, 0), extent(W, H));
        b.copy_texture(&at(&depth, 0, 0), &at(&dst, 1, 1), extent(3, 2));
        if read(b, &dst)? != moved(&in_depth) {
            return Err(format!("depth16unorm into {colour:?} changed the bits"));
        }
        if b.status() != DeviceStatus::Ok {
            return Err(format!("{colour:?}: device {:?}", b.status()));
        }
        copied.push(format!("{colour:?}"));
    }
    Ok(format!(
        "16-bit group copies with depth16unorm, whole and in part, both ways: bits kept for {}",
        copied.join(", ")
    ))
}
