//! Copies from a texture into a buffer write the copied rows and nothing between them: the bytes
//! between rows and between layers keep what the buffer held, at any offset and pitch. Shared by
//! tests/copies.rs and the browser page.

use wcontainers_gpu::{
    Backend, BufferDesc, BufferUsage, CopyBufferTexture, DeviceStatus, Extent, Format, MapIntent,
    Origin, ReadSource, TextureDesc, TextureDimension, TextureRegion, TextureUsage, WriteMode,
};
use wcontainers_gpu_webgpu::WebGpu;

/// What the buffer holds before each copy.
const FILL: u8 = 0xcd;
/// The texture's size and layers.
const SIZE: (u32, u32, u32) = (26, 14, 2);
/// Where the copied region starts, and its size.
const ORIGIN: (u32, u32) = (7, 2);
const EXTENT: (u32, u32) = (5, 4);
/// Rows of blocks between layers in the buffer: more than the region has, so layers have gaps too.
const ROWS_PER_LAYER: u32 = EXTENT.1 + 2;
/// (offset, pitch) of the copies, each rounded up to a whole number of the format's blocks: one
/// Direct3D 12 places unaligned with gaps, one aligned with WebGPU's own pitch.
const PLACES: [(u64, u32); 2] = [(256, 512), (0, 256)];
/// Formats copied: native ones of two texel sizes, one held converted, one held as four channels.
const FORMATS: [Format; 4] = [
    Format::Rgba8Unorm,
    Format::Rgba32Float,
    Format::B5g6r5Unorm,
    Format::Rgb32Float,
];

/// Copies part of two layers of a seeded texture of each format into a buffer filled with
/// [`FILL`] at each of [`PLACES`]; fails where a copied byte differs from the texture's read back,
/// or a byte outside the copied rows changed. One line saying what was checked.
pub fn reads_keep_the_bytes_between_rows(b: &mut WebGpu, seed: u64) -> Result<String, String> {
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut checked = Vec::new();
    for format in FORMATS {
        let (bw, _, block_bytes) = format.block();
        let (w, h, layers) = SIZE;
        let texture = b
            .create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width: w,
                height: h,
                depth_or_layers: layers,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
                cube: false,
                reinterpret: false,
            })
            .map_err(|e| format!("{format:?}: {e:?}"))?;
        let pitch = w.div_ceil(bw) * block_bytes;
        let data: Vec<u8> = (0..pitch * h * layers).map(|_| rng.u8(..)).collect();
        let whole = |origin: (u32, u32)| TextureRegion {
            texture: &texture,
            mip: 0,
            layer: 0,
            origin: Origin {
                x: origin.0,
                y: origin.1,
                z: 0,
            },
            stencil: false,
        };
        b.write_texture(
            &whole((0, 0)),
            Extent {
                width: w,
                height: h,
                depth_or_layers: layers,
            },
            &data,
            pitch,
            h,
            WriteMode::Discard,
        );
        let extent = Extent {
            width: EXTENT.0,
            height: EXTENT.1,
            depth_or_layers: layers,
        };
        let row = (EXTENT.0.div_ceil(bw) * block_bytes) as usize;
        let rb = b
            .read_back(&ReadSource::Texture {
                region: whole(ORIGIN),
                extent,
            })
            .map_err(|e| format!("{format:?}: {e:?}"))?;
        b.submit();
        b.wait_read(&rb);
        let mut want = vec![0; row * EXTENT.1 as usize * layers as usize];
        b.finish_read(rb, &mut want);
        for (offset, row_pitch) in PLACES {
            let offset = offset.next_multiple_of(u64::from(block_bytes));
            let row_pitch = row_pitch.next_multiple_of(block_bytes);
            let size =
                offset + u64::from(row_pitch) * u64::from(ROWS_PER_LAYER) * u64::from(layers);
            let buffer = b
                .create_buffer(&BufferDesc {
                    size,
                    usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
                    intent: MapIntent::Rarely,
                })
                .map_err(|e| format!("{e:?}"))?;
            b.write_buffer(&buffer, 0, &vec![FILL; size as usize], WriteMode::Discard);
            b.copy_texture_to_buffer(
                &whole(ORIGIN),
                &CopyBufferTexture {
                    buffer: &buffer,
                    offset,
                    row_pitch,
                    rows_per_layer: ROWS_PER_LAYER,
                },
                extent,
            );
            let rb = b
                .read_back(&ReadSource::Buffer {
                    buffer: &buffer,
                    offset: 0,
                    size,
                })
                .map_err(|e| format!("{e:?}"))?;
            b.submit();
            b.wait_read(&rb);
            let mut got = vec![0; size as usize];
            b.finish_read(rb, &mut got);
            if b.status() != DeviceStatus::Ok {
                return Err(format!("{format:?}: device {:?}", b.status()));
            }
            let mut expected = vec![FILL; size as usize];
            for l in 0..layers as usize {
                for r in 0..EXTENT.1 as usize {
                    let at =
                        offset as usize + (l * ROWS_PER_LAYER as usize + r) * row_pitch as usize;
                    let from = (l * EXTENT.1 as usize + r) * row;
                    expected[at..at + row].copy_from_slice(&want[from..from + row]);
                }
            }
            let differ: Vec<usize> = (0..got.len()).filter(|&i| got[i] != expected[i]).collect();
            if let (Some(first), Some(last)) = (differ.first(), differ.last()) {
                return Err(format!(
                    "{format:?} at offset {offset}, pitch {row_pitch}: {} bytes differ, first at \
                     {first}, last at {last}",
                    differ.len()
                ));
            }
        }
        checked.push(format!("{format:?}"));
    }
    Ok(format!(
        "texture reads into buffers at (offset, pitch) {PLACES:?}, {} layers: rows exact, every \
         byte between them kept, for {}",
        SIZE.2,
        checked.join(", ")
    ))
}
