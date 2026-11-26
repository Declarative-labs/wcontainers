//! Copies of 24-bit depth between buffers and textures, converted on the GPU from and to Metal's 32-bit floats.
//!
//! A copy in converts the words into tight floats and blits them in; a copy out blits tight floats and converts
//! them into the words at the buffer's pitches. Both give the host's `f64` bits (`DEPTH_PROGRAMS`).

use wcontainers_gpu::{CopyBufferTexture, Extent, Gpu, ShaderModule, ShaderStage, TextureRegion};
use wcontainers_metal::{ComputeCommand, Metal, Size};

use crate::MetalGpu;
use crate::bytes::{COPY_GRANULE, rounded};
use crate::objects::BufferObj;

/// The threads of one threadgroup of the depth programs, as their `workgroup_size` says.
///
/// https://www.w3.org/TR/WGSL/#attribute-workgroup_size
const DEPTH_THREADS: u32 = 64;

/// The programs that convert depth words a word at a time, each thread converting one texel's word.
///
/// Each function's comments give its rounding, which is the host's: `(f64::from(n) / 16777215.0) as f32` in, and
/// `(f64::from(x).clamp(0.0, 1.0) * 16777215.0).round_ties_even() as u32` out.
const DEPTH_PROGRAMS: &str = "
struct Span { at: u32, pitch: u32, layer: u32, width: u32, rows: u32, layers: u32, low: u32, high: u32 }
@group(0) @binding(0) var<uniform> span: Span;
@group(0) @binding(1) var<storage, read> source: array<u32>;
@group(0) @binding(2) var<storage, read_write> destination: array<u32>;

fn unorm_to_float(word: u32) -> u32 {
    // The float nearest n / (2^24 - 1). Below the binary point its bits are the 24 bits of n repeated, so the float's
    // 24-bit significand is n rotated to its leading one, and what follows is that rotation again: more than half a
    // unit, which rounds up. The host's two roundings agree, as the bits past 53 never make a tie.
    let n = word & 0xffffffu;
    if n == 0u {
        return 0u;
    }
    if n == 0xffffffu {
        return 0x3f800000u;
    }
    let lead = countLeadingZeros(n) - 8u;
    let turned = ((n << lead) | (n >> (24u - lead))) & 0xffffffu;
    return ((126u - lead) << 23u) | ((turned + 1u) & 0x7fffffu);
}

fn float_to_unorm(word: u32) -> u32 {
    // x * (2^24 - 1) of the float x clamped to zero to one, rounded to the nearest with ties to even (a NaN gives
    // zero). For x = m * 2^(e - 150) below one, of 24-bit significand m, the product is
    // ((m - 1) * 2^24 + (2^24 - m)) / 2^(t + 24) with t = 126 - e: its whole part is (m - 1) >> t, and as the low digit
    // 2^24 - m is above zero and at most 2^23, it rounds up exactly when bit t - 1 of m - 1 is set, with a tie only at
    // t = 0 and m = 2^23 (one half, giving 2^23).
    if word > 0x7f800000u {
        return 0u;
    }
    if word >= 0x3f800000u {
        return 0xffffffu;
    }
    let e = word >> 23u;
    if e < 102u {
        return 0u;
    }
    let h = ((word & 0x7fffffu) | 0x800000u) - 1u;
    let t = 126u - e;
    if t == 0u {
        return select(h, h + 1u, h == 0x7fffffu);
    }
    return (h >> t) + ((h >> (t - 1u)) & 1u);
}

// The word of texel `id` at the buffer's pitches, and its place in the tight floats.
fn spaced(id: vec3<u32>) -> u32 {
    return span.at + id.z * span.layer + id.y * span.pitch + id.x;
}

fn tight(id: vec3<u32>) -> u32 {
    return (id.z * span.rows + id.y) * span.width + id.x;
}

fn inside(id: vec3<u32>) -> bool {
    return id.x < span.width && id.y < span.rows && id.z < span.layers;
}

@compute @workgroup_size(64) fn words_to_floats(@builtin(global_invocation_id) id: vec3<u32>) {
    if inside(id) {
        destination[tight(id)] = unorm_to_float(source[spaced(id)]);
    }
}

@compute @workgroup_size(64) fn floats_to_words(@builtin(global_invocation_id) id: vec3<u32>) {
    if inside(id) {
        destination[spaced(id)] = float_to_unorm(source[tight(id)]);
    }
}
";

/// One of the depth programs, made at its first use, and the buffer slots of its bindings.
pub(crate) struct DepthPipeline<M: Metal> {
    state: M::ComputePipelineState,
    span: u32,
    source: u32,
    destination: u32,
}

/// The two depth programs: into floats, and into words.
pub(crate) struct DepthPrograms<M: Metal> {
    into_floats: Option<DepthPipeline<M>>,
    into_words: Option<DepthPipeline<M>>,
}

impl<M: Metal> Default for DepthPrograms<M> {
    fn default() -> Self {
        DepthPrograms {
            into_floats: None,
            into_words: None,
        }
    }
}

/// Where a copy's texels lie in a buffer and in the tight floats, all in 32-bit words.
pub(crate) struct DepthSpan {
    /// The word of the first texel in the buffer.
    at: u32,
    /// Words from one row to the next in the buffer.
    pitch: u32,
    /// Words from one layer to the next in the buffer.
    layer: u32,
    /// Texels of a row, rows of a layer, and layers.
    width: u32,
    rows: u32,
    layers: u32,
    /// The bytes of the tight floats.
    pub(crate) tight_bytes: u64,
}

impl DepthSpan {
    /// Returns where a copy of `extent` lies in a buffer of `size` bytes at `side`'s offset and pitches.
    ///
    /// None where any texel's word is outside the buffer, off a word, or past what a 32-bit word number reaches, or
    /// where rows or layers would overlap.
    pub(crate) fn of<B: Gpu>(
        side: &CopyBufferTexture<'_, B>,
        extent: Extent,
        size: u64,
    ) -> Option<DepthSpan> {
        let (rows, layers) = (extent.height, extent.depth_or_layers);
        let (last_row, last_layer) = (rows.checked_sub(1)?, layers.checked_sub(1)?);
        let row_bytes = u64::from(extent.width).checked_mul(COPY_GRANULE)?;
        let pitch = u64::from(side.row_pitch);
        let offset_whole =
            side.offset.is_multiple_of(COPY_GRANULE) && pitch.is_multiple_of(COPY_GRANULE);
        // Rows per layer only space layers, so one layer takes any.
        let layers_apart = last_layer == 0 || side.rows_per_layer >= rows;
        if extent.width == 0 || !offset_whole || pitch < row_bytes || !layers_apart {
            return None;
        }
        let layer_bytes = if last_layer == 0 {
            u64::MIN
        } else {
            pitch.checked_mul(u64::from(side.rows_per_layer))?
        };
        // The last texel's word ends the copy's bytes; every other texel's lies before it.
        let end = u64::from(last_layer)
            .checked_mul(layer_bytes)?
            .checked_add(u64::from(last_row).checked_mul(pitch)?)?
            .checked_add(row_bytes)?
            .checked_add(side.offset)?;
        let tight_bytes = row_bytes
            .checked_mul(u64::from(rows))?
            .checked_mul(u64::from(layers))?;
        let most = u64::from(u32::MAX);
        if end > size || end / COPY_GRANULE > most || tight_bytes / COPY_GRANULE > most {
            return None;
        }
        Some(DepthSpan {
            at: u32::try_from(side.offset / COPY_GRANULE).ok()?,
            pitch: u32::try_from(pitch / COPY_GRANULE).ok()?,
            layer: u32::try_from(layer_bytes / COPY_GRANULE).ok()?,
            width: extent.width,
            rows,
            layers,
            tight_bytes,
        })
    }

    /// Returns the words of the uniform the depth programs read.
    fn uniform(&self) -> Vec<u8> {
        [
            self.at,
            self.pitch,
            self.layer,
            self.width,
            self.rows,
            self.layers,
            u32::MIN,
            u32::MIN,
        ]
        .iter()
        .flat_map(|n| n.to_le_bytes())
        .collect()
    }
}

impl<M: Metal> MetalGpu<M> {
    /// Returns a depth program, made at its first use.
    fn depth_pipeline(&mut self, into_floats: bool) -> Result<&DepthPipeline<M>, &'static str> {
        let made = if into_floats {
            self.depth_programs.into_floats.is_some()
        } else {
            self.depth_programs.into_words.is_some()
        };
        if !made {
            let entry = if into_floats {
                "words_to_floats"
            } else {
                "floats_to_words"
            };
            let module = ShaderModule::from_wgsl(DEPTH_PROGRAMS, entry, ShaderStage::Compute)
                .map_err(|_| "a depth program that does not parse")?;
            let shader = self
                .create_shader(&module)
                .map_err(|_| "a depth program refused")?;
            let function = self.make_function(&shader, &[], false)?;
            let state = self
                .metal
                .make_compute_pipeline_state(&function)
                .map_err(|_| "a depth program Metal refuses")?;
            let slot = |binding: u32| shader.slots.get(&(0, binding)).map(|&(_, index)| index);
            let missing = "a depth program without its bindings";
            let pipeline = DepthPipeline {
                state,
                span: slot(0).ok_or(missing)?,
                source: slot(1).ok_or(missing)?,
                destination: slot(2).ok_or(missing)?,
            };
            if into_floats {
                self.depth_programs.into_floats = Some(pipeline);
            } else {
                self.depth_programs.into_words = Some(pipeline);
            }
        }
        let made = if into_floats {
            &self.depth_programs.into_floats
        } else {
            &self.depth_programs.into_words
        };
        made.as_ref().ok_or("a depth program not made")
    }

    /// Makes the tight buffer of floats a depth copy goes through.
    pub(crate) fn tight_floats(&mut self, span: &DepthSpan) -> Result<M::Buffer, &'static str> {
        let length = rounded(span.tight_bytes).ok_or("tight floats past the largest length")?;
        self.metal
            .make_buffer(length)
            .map_err(|_| "no buffer for tight floats")
    }

    /// Encodes the depth program that converts between `span`'s words of `buffer` and the tight `floats`.
    ///
    /// The words become floats where `into_floats`, else the floats become words.
    pub(crate) fn convert_depth(
        &mut self,
        into_floats: bool,
        buffer: &BufferObj<M>,
        floats: &M::Buffer,
        span: &DepthSpan,
    ) -> Result<(), &'static str> {
        let raw = buffer.raw.borrow().clone();
        let (raw_length, floats_length) = (
            rounded(buffer.size).ok_or("a buffer past the largest length")?,
            rounded(span.tight_bytes).ok_or("tight floats past the largest length")?,
        );
        let pipeline = self.depth_pipeline(into_floats)?;
        let (state, span_slot, source_slot, destination_slot) = (
            pipeline.state.clone(),
            pipeline.span,
            pipeline.source,
            pipeline.destination,
        );
        let ((source, source_length), (destination, destination_length)) = if into_floats {
            ((raw, raw_length), (floats.clone(), floats_length))
        } else {
            ((floats.clone(), floats_length), (raw, raw_length))
        };
        let groups = span.width.div_ceil(DEPTH_THREADS);
        let commands = vec![
            ComputeCommand::SetComputePipelineState(state),
            ComputeCommand::SetBytes {
                bytes: span.uniform(),
                index: span_slot,
            },
            ComputeCommand::SetBuffer {
                buffer: source,
                offset: u64::MIN,
                length: source_length,
                index: source_slot,
            },
            ComputeCommand::SetBuffer {
                buffer: destination,
                offset: u64::MIN,
                length: destination_length,
                index: destination_slot,
            },
            ComputeCommand::DispatchThreadgroups(Size {
                width: groups,
                height: span.rows,
                depth: span.layers,
            }),
        ];
        self.encode_own_compute(&commands)
    }

    /// Copies a buffer's depth words into a texture's region: converted into tight floats, then blitted in.
    pub(crate) fn depth_into_texture(
        &mut self,
        src: &CopyBufferTexture<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) -> Result<(), &'static str> {
        // The region is checked first, so the copies below are bounded by the texture.
        if !crate::texture_region_inside(dst, extent) {
            return Err("a depth copy outside its texture");
        }
        let span = DepthSpan::of(src, extent, src.buffer.size)
            .ok_or("a depth copy outside its buffer, off its words or overlapping")?;
        let floats = self.tight_floats(&span)?;
        self.convert_depth(true, src.buffer, &floats, &span)?;
        let copies = self.tight_copies(dst, extent, &floats, true)?;
        for copy in copies {
            self.queue_blit(copy);
        }
        Ok(())
    }

    /// Copies a texture's region of depth into a buffer's words.
    ///
    /// The depth is blitted out as tight floats, then converted into the buffer at its pitches.
    pub(crate) fn depth_out_of_texture(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &CopyBufferTexture<'_, Self>,
        extent: Extent,
    ) -> Result<(), &'static str> {
        if !crate::texture_region_inside(src, extent) {
            return Err("a depth copy outside its texture");
        }
        let span = DepthSpan::of(dst, extent, dst.buffer.size)
            .ok_or("a depth copy outside its buffer, off its words or overlapping")?;
        let floats = self.tight_floats(&span)?;
        let copies = self.tight_copies(src, extent, &floats, false)?;
        for copy in copies {
            self.queue_blit(copy);
        }
        self.convert_depth(false, dst.buffer, &floats, &span)
    }

    /// Returns the blits between a texture's region and tight floats, one per layer.
    fn tight_copies(
        &self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        floats: &M::Buffer,
        into_texture: bool,
    ) -> Result<Vec<wcontainers_metal::BlitCommand<M>>, &'static str> {
        let texture = region.texture.clone();
        let (row, rows) = self
            .tight_rows(texture.desc.format, false, extent)
            .ok_or("a depth copy whose rows overflow")?;
        self.buffer_texture_copies(&texture, region, extent, floats, 0, row, rows, into_texture)
            .ok_or("a depth copy whose places overflow")
    }
}
