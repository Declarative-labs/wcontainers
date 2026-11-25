//! Colour clears of 32-bit float formats keep the value's bits: denormals, negative zero and NaN
//! payloads read back as given, as Direct3D's storage clears take them. Shared by tests/writes.rs
//! and the browser page.

use wcontainers_gpu::{
    Gpu, ClearValue, DeviceStatus, Extent, FORMAT_FACTS, Format, Origin, ReadSource, SlowPath,
    TextureDesc, TextureDimension, TextureRegion, TextureUsage,
};
use wcontainers_gpu_webgpu::WebGpu;

/// Values whose bits a float clear value changes (each NaN, both denormals), and ones it keeps.
const VALUES: [[u32; 4]; 3] = [
    [0x0000_0001, 0x807f_ffff, 0x7fc0_0001, 0x7f80_0001],
    [
        0xffc1_2345,
        (-0.0f32).to_bits(),
        (f32::MIN_POSITIVE / 2.0).to_bits(),
        0x7fff_ffff,
    ],
    [
        1.0f32.to_bits(),
        (-0.5f32).to_bits(),
        f32::MIN_POSITIVE.to_bits(),
        0,
    ],
];

/// Formats of 32-bit float channels that can be drawn into.
const FORMATS: [Format; 3] = [Format::R32Float, Format::Rg32Float, Format::Rgba32Float];

/// The texture's size and layers: the clear covers a level of each layer.
const SIZE: (u32, u32, u32) = (5, 3, 2);

/// Clears every format to each of [`VALUES`] and reads every texel back: fails at the first bit
/// that differs. One line saying what was checked and how many clears were drawn.
pub fn float_clears_keep_their_bits(b: &mut WebGpu) -> Result<String, String> {
    let before = b.slow_paths().count(SlowPath::FloatBitsClearByDraw);
    let (w, h, layers) = SIZE;
    for format in FORMATS {
        let desc = TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width: w,
            height: h,
            depth_or_layers: layers,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::COPY_SRC,
            cube: false,
            reinterpret: false,
        };
        let texture = b.create_texture(&desc).map_err(|e| format!("{e:?}"))?;
        let channels = FORMAT_FACTS[format as usize].components.count_ones() as usize;
        for value in VALUES {
            b.clear_texture(
                &texture,
                &desc,
                format,
                0..1,
                0..layers,
                ClearValue::Color(value),
            )
            .map_err(|e| format!("{format:?}: {e:?}"))?;
            let rb = b
                .read_back(&ReadSource::Texture {
                    region: TextureRegion {
                        texture: &texture,
                        mip: 0,
                        layer: 0,
                        origin: Origin::default(),
                        stencil: false,
                    },
                    extent: Extent {
                        width: w,
                        height: h,
                        depth_or_layers: layers,
                    },
                })
                .map_err(|e| format!("{format:?}: {e:?}"))?;
            b.submit();
            b.wait_read(&rb);
            let mut got = vec![0; (w * h * layers) as usize * channels * size_of::<u32>()];
            b.finish_read(rb, &mut got);
            if b.status() != DeviceStatus::Ok {
                return Err(format!("{format:?}: device {:?}", b.status()));
            }
            let words: Vec<u32> = bytemuck::pod_collect_to_vec(&got);
            for (i, texel) in words.chunks_exact(channels).enumerate() {
                if texel != &value[..channels] {
                    return Err(format!(
                        "{format:?} cleared to {value:08x?}: texel {i} reads {texel:08x?}"
                    ));
                }
            }
        }
    }
    let drawn = b.slow_paths().count(SlowPath::FloatBitsClearByDraw) - before;
    Ok(format!(
        "float clears: {:?} to {} values each, every bit kept, {drawn} clears drawn",
        FORMATS,
        VALUES.len()
    ))
}
