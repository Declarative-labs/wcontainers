//! Formats WebGPU lacks that the back end holds as `rgba8unorm` (`FormatConverted`): 8-bit alpha
//! and the 16-bit packed formats written with seeded texels, read back, copied through buffers and
//! between textures, loaded and filtered in a program; and 8-bit alpha drawn into with blending.
//! Shared by tests/converted.rs, tests/direct3d.rs (against real Direct3D 11) and the browser page.

use anyhow::{Result, anyhow, ensure};
use wcontainers_gpu::{
    AddressMode, Backend, BindingEntry, BindingResource, BlendComponent, BlendFactor, BlendOp,
    BorderColor, BufferDesc, BufferUsage, ColorAttachment, CopyBufferTexture, DeviceStatus,
    DrawCall, DrawRange, Extent, Filter, Format, FormatUse, LoadOp, MapIntent, Origin, PassDesc,
    ReadSource, SamplerDesc, SlowPath, StageSet, StoreOp, TextureRegion, TextureUsage, Topology,
    WriteMode,
};
use wcontainers_gpu_webgpu::WebGpu;

use crate::draw;

type B = WebGpu;

/// Every texture's size.
pub const SIZE: (u32, u32) = (16, 8);

/// The formats the back end converts.
pub const FORMATS: [Format; 8] = [
    Format::A8Unorm,
    Format::Bgrx8Unorm,
    Format::Bgrx8UnormSrgb,
    Format::B5g6r5Unorm,
    Format::Bgr5a1Unorm,
    Format::Bgra4Unorm,
    Format::Rgbg8Unorm,
    Format::Grgb8Unorm,
];

/// The converted formats drawn into.
pub const DRAWN: [Format; 3] = [Format::A8Unorm, Format::Bgrx8Unorm, Format::Bgrx8UnormSrgb];

/// Bytes of a row of `width` texels of `format`: whole blocks.
pub fn row_bytes(format: Format, width: u32) -> usize {
    let (bw, _, bytes) = format.block();
    (width.div_ceil(bw) * bytes) as usize
}

/// The left half of the target loads each texel; the right half filters the texture a quarter
/// texel up and left of each texel's centre. The same as tests/direct3d.rs's HLSL.
pub const SAMPLED_WGSL: &str = "
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
@group(1) @binding(32) var t: texture_2d<f32>;
@group(1) @binding(16) var s: sampler;
@fragment fn ps(@builtin(position) p: vec4f) -> @location(0) vec4f {
    let size = vec2f(textureDimensions(t));
    if (p.x < size.x) {
        return textureLoad(t, vec2i(p.xy), 0);
    }
    return textureSampleLevel(t, s, (p.xy - vec2f(size.x, 0.0) - 0.25) / size, 0.0);
}
";

/// A colour with an alpha varying over the target, blended into 8-bit alpha.
pub const ALPHA_WGSL: &str = "
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn ps(@builtin(position) p: vec4f) -> @location(0) vec4f {
    return vec4f(0.9, 0.6, 0.3, fract((p.x * 7.0 + p.y * 13.0) / 32.0));
}
";

/// The alpha targets drawn into are cleared to.
pub const CLEAR_ALPHA: f32 = 0.25;

/// Seeded texels of `format` (splitmix64 bytes), rows packed.
pub fn texels(format: Format, seed: u64) -> Vec<u8> {
    let mut s = seed;
    let n = row_bytes(format, SIZE.0) * SIZE.1 as usize;
    (0..n)
        .map(|_| {
            s = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            (z ^ (z >> 31)) as u8
        })
        .collect()
}

/// sRGB-encoded `c` in linear light.
pub fn linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The float RGBA of each texel by Direct3D's rule: each channel `v / (2^n - 1)` (sRGB in linear
/// light), a missing alpha one, 8-bit alpha's colour zero, each pixel of a pair its own.
pub fn expected(format: Format, texels: &[u8]) -> Vec<f32> {
    let n = |v: u8| f32::from(v) / 255.0;
    let quads = texels.as_chunks::<4>().0.iter();
    match format {
        Format::A8Unorm => {
            return texels.iter().flat_map(|&a| [0.0, 0.0, 0.0, n(a)]).collect();
        }
        Format::Bgrx8Unorm => {
            return quads
                .flat_map(|t| [n(t[2]), n(t[1]), n(t[0]), 1.0])
                .collect();
        }
        Format::Bgrx8UnormSrgb => {
            return quads
                .flat_map(|t| {
                    [n(t[2]), n(t[1]), n(t[0])]
                        .map(linear)
                        .into_iter()
                        .chain([1.0])
                })
                .collect();
        }
        Format::Rgbg8Unorm | Format::Grgb8Unorm => {
            return quads
                .flat_map(|p| {
                    let [r, g0, b, g1] = match format {
                        Format::Rgbg8Unorm => *p,
                        _ => [p[1], p[0], p[3], p[2]],
                    };
                    [n(r), n(g0), n(b), 1.0, n(r), n(g1), n(b), 1.0]
                })
                .collect();
        }
        _ => {}
    }
    let fields: [u32; 4] = match format {
        Format::B5g6r5Unorm => [5, 6, 5, 0],
        Format::Bgr5a1Unorm => [5, 5, 5, 1],
        _ => [4, 4, 4, 4],
    };
    texels
        .as_chunks::<2>()
        .0
        .iter()
        .flat_map(|t| {
            let v = u32::from(u16::from_le_bytes(*t));
            let (mut bgra, mut at) = ([1.0f32; 4], 0);
            for (c, bits) in bgra.iter_mut().zip(fields).filter(|(_, b)| *b > 0) {
                let max = (1u32 << bits) - 1;
                *c = ((v >> at) & max) as f32 / max as f32;
                at += bits;
            }
            [bgra[2], bgra[1], bgra[0], bgra[3]]
        })
        .collect()
}

/// `texels` (float RGBA, `SIZE`) filtered as the right half of [`SAMPLED_WGSL`] samples them:
/// three quarters of each texel and a quarter of its neighbours up and left, clamped at the edges.
pub fn filtered(texels: &[f32]) -> Vec<f32> {
    let (w, h) = (SIZE.0 as usize, SIZE.1 as usize);
    let at = |x: usize, y: usize, c: usize| texels[(y * w + x) * 4 + c];
    let mut out = Vec::with_capacity(texels.len());
    for y in 0..h {
        for x in 0..w {
            let (x0, y0) = (x.saturating_sub(1), y.saturating_sub(1));
            for c in 0..4 {
                let top = 0.25 * at(x0, y0, c) + 0.75 * at(x, y0, c);
                let row = 0.25 * at(x0, y, c) + 0.75 * at(x, y, c);
                out.push(0.25 * top + 0.75 * row);
            }
        }
    }
    out
}

/// The alpha bytes [`ALPHA_WGSL`] blended over [`CLEAR_ALPHA`] gives, each within one step.
pub fn blended() -> Vec<f32> {
    let d = (CLEAR_ALPHA * 255.0).round() / 255.0;
    let mut out = Vec::new();
    for y in 0..SIZE.1 {
        for x in 0..SIZE.0 {
            let s = ((x as f32 + 0.5) * 7.0 + (y as f32 + 0.5) * 13.0) / 32.0;
            let s = s.fract();
            out.push((s * s + d * (1.0 - s)) * 255.0);
        }
    }
    out
}

fn gpu<T>(r: Result<T, wcontainers_gpu::GpuError>) -> Result<T> {
    r.map_err(|e| anyhow!("{e:?}"))
}

fn whole() -> Extent {
    Extent {
        width: SIZE.0,
        height: SIZE.1,
        depth_or_layers: 1,
    }
}

fn at(texture: &<B as Backend>::Texture, x: u32, y: u32) -> TextureRegion<'_, B> {
    TextureRegion {
        texture,
        mip: 0,
        layer: 0,
        origin: Origin { x, y, z: 0 },
        stencil: false,
    }
}

/// A texture of `format` holding `texels` (none: left as made).
pub fn upload(
    b: &mut B,
    format: Format,
    texels: Option<&[u8]>,
    usage: TextureUsage,
) -> Result<(<B as Backend>::Texture, <B as Backend>::TextureView)> {
    let usage = usage | TextureUsage::SAMPLE | TextureUsage::COPY_DST | TextureUsage::COPY_SRC;
    let (t, v) = draw::texture_2d(b, format, SIZE.0, SIZE.1, usage)?;
    if let Some(texels) = texels {
        let pitch = row_bytes(format, SIZE.0) as u32;
        b.write_texture(
            &at(&t, 0, 0),
            whole(),
            texels,
            pitch,
            SIZE.1,
            WriteMode::Ordered,
        );
    }
    Ok((t, v))
}

/// Reads a region of a texture back.
pub fn read(b: &mut B, region: &TextureRegion<'_, B>, extent: Extent, len: usize) -> Vec<u8> {
    let source = ReadSource::Texture {
        region: TextureRegion {
            texture: region.texture,
            mip: region.mip,
            layer: region.layer,
            origin: region.origin,
            stencil: region.stencil,
        },
        extent,
    };
    let rb = gpu(b.read_back(&source)).expect("a read back");
    b.submit();
    b.wait_read(&rb);
    let mut out = vec![0; len];
    b.finish_read(rb, &mut out);
    out
}

/// What [`SAMPLED_WGSL`] draws from `view`: the loaded texels, then the filtered ones, each as
/// float RGBA, rows packed.
pub fn sampled(b: &mut B, view: &<B as Backend>::TextureView) -> Result<(Vec<f32>, Vec<f32>)> {
    let sampler = gpu(b.create_sampler(&SamplerDesc {
        min: Filter::Linear,
        mag: Filter::Linear,
        mip: Filter::Nearest,
        address: [AddressMode::Clamp; 3],
        lod_range: [0, 0],
        anisotropy: 1,
        compare: None,
        border: BorderColor::TransparentBlack,
    }))?;
    let set = gpu(b.create_binding_set(&[
        BindingEntry {
            binding: 32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(view),
        },
        BindingEntry {
            binding: 16,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Sampler(&sampler),
        },
    ]))?;
    let (w, h) = (SIZE.0 * 2, SIZE.1);
    let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
    let (target, target_view) = draw::texture_2d(b, Format::Rgba32Float, w, h, usage)?;
    let program = draw::program(b, SAMPLED_WGSL)?;
    full_draw(
        b,
        &program,
        &target_view,
        &draw::opaque(),
        [Some(&set), None],
        [0; 4],
    )?;
    let extent = Extent {
        width: w,
        height: h,
        depth_or_layers: 1,
    };
    let bytes = read(b, &at(&target, 0, 0), extent, (w * h * 16) as usize);
    let floats: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect();
    let (mut loaded, mut filtered) = (Vec::new(), Vec::new());
    for row in floats.chunks_exact((w * 4) as usize) {
        let (l, f) = row.split_at((SIZE.0 * 4) as usize);
        loaded.extend_from_slice(l);
        filtered.extend_from_slice(f);
    }
    Ok((loaded, filtered))
}

/// One draw of three vertices covering `target`, cleared to `clear` first.
fn full_draw(
    b: &mut B,
    program: &<B as Backend>::Program,
    target: &<B as Backend>::TextureView,
    blend: &wcontainers_gpu::BlendDesc,
    sets: [Option<&<B as Backend>::BindingSet>; 2],
    clear: [u32; 4],
) -> Result<()> {
    let blend = gpu(b.create_blend(blend))?;
    let depth_stencil = gpu(b.create_depth_stencil(&draw::no_depth()))?;
    let raster = gpu(b.create_raster(&draw::solid()))?;
    let layout = gpu(b.create_vertex_layout(&draw::layout(&[], &[])))?;
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: target,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear(clear),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    b.draw(&DrawCall {
        program,
        blend: &blend,
        depth_stencil: &depth_stencil,
        raster: &raster,
        vertex_layout: &layout,
        bindings: [None, sets[0], sets[1], None],
        vertex_buffers: &[],
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: &[],
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: 3,
            first_instance: 0,
            instances: 1,
        },
    });
    b.end_pass();
    Ok(())
}

/// [`ALPHA_WGSL`] blended (source alpha over one minus it, colour and alpha alike) into a target
/// of `format` cleared to a colour and [`CLEAR_ALPHA`]: its bytes, and its texels loaded.
pub fn drawn(b: &mut B, format: Format) -> Result<(Vec<u8>, Vec<f32>)> {
    let (target, view) = upload(b, format, None, TextureUsage::RENDER)?;
    let program = draw::program(b, ALPHA_WGSL)?;
    let mut blend = draw::opaque();
    let over = BlendComponent {
        src: BlendFactor::SrcAlpha,
        dst: BlendFactor::OneMinusSrcAlpha,
        op: BlendOp::Add,
    };
    blend.targets[0].blend = Some((over, over));
    let clear = [0.5, 0.75, 1.0, CLEAR_ALPHA].map(f32::to_bits);
    full_draw(b, &program, &view, &blend, [None, None], clear)?;
    let len = row_bytes(format, SIZE.0) * SIZE.1 as usize;
    let bytes = read(b, &at(&target, 0, 0), whole(), len);
    let (loaded, _) = sampled(b, &view)?;
    Ok((bytes, loaded))
}

/// `texels` of `format` as the back end keeps them: BGRX's X read back as one.
pub fn kept(format: Format, texels: &[u8]) -> Vec<u8> {
    let mut out = texels.to_vec();
    if matches!(format, Format::Bgrx8Unorm | Format::Bgrx8UnormSrgb) {
        out.iter_mut().skip(3).step_by(4).for_each(|x| *x = 255);
    }
    out
}

/// Writes, read backs and copies of `texels` in `format`: read back whole; copied out into a
/// buffer at a padded pitch whose padding stays; copied from that buffer into part of another
/// texture; copied between textures. Every byte must come back (BGRX's X as one).
pub fn copies(b: &mut B, format: Format, written: &[u8]) -> Result<()> {
    let texels = &kept(format, written)[..];
    let h = SIZE.1 as usize;
    let row = row_bytes(format, SIZE.0);
    let len = row * h;
    // Bytes from the start of a row to texel `x`, on a block edge.
    let col = |x: u32| row_bytes(format, x);
    let (t, _) = upload(b, format, Some(written), TextureUsage::empty())?;
    ensure!(
        read(b, &at(&t, 0, 0), whole(), len) == texels,
        "{format:?}: read back"
    );
    // Out into a buffer whose rows are 256 bytes apart, padding marked.
    let pitch = 256usize;
    let buffer = gpu(b.create_buffer(&BufferDesc {
        size: (pitch * h) as u64,
        usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
        intent: MapIntent::Rarely,
    }))?;
    b.write_buffer(&buffer, 0, &vec![0xab; pitch * h], WriteMode::Ordered);
    let copy = |offset: u64| CopyBufferTexture {
        buffer: &buffer,
        offset,
        row_pitch: pitch as u32,
        rows_per_layer: SIZE.1,
    };
    b.copy_texture_to_buffer(&at(&t, 0, 0), &copy(0), whole());
    let rb = gpu(b.read_back(&ReadSource::Buffer {
        buffer: &buffer,
        offset: 0,
        size: (pitch * h) as u64,
    }))?;
    b.submit();
    b.wait_read(&rb);
    let mut out = vec![0; pitch * h];
    b.finish_read(rb, &mut out);
    for y in 0..h {
        let r = &out[y * pitch..(y + 1) * pitch];
        ensure!(
            r[..row] == texels[y * row..(y + 1) * row],
            "{format:?}: row {y} copied into a buffer"
        );
        ensure!(
            r[row..].iter().all(|&v| v == 0xab),
            "{format:?}: padding of row {y} overwritten"
        );
    }
    // From the buffer, starting at texel (2, 1), into another texture at (4, 2).
    let part = Extent {
        width: SIZE.0 - 6,
        height: SIZE.1 - 3,
        depth_or_layers: 1,
    };
    let (u, _) = upload(b, format, Some(&vec![0; len]), TextureUsage::empty())?;
    b.copy_buffer_to_texture(&copy((pitch + col(2)) as u64), &at(&u, 4, 2), part);
    let got = read(
        b,
        &at(&u, 4, 2),
        part,
        col(part.width) * part.height as usize,
    );
    let want: Vec<u8> = (1..1 + part.height as usize)
        .flat_map(|y| texels[y * row + col(2)..y * row + col(2 + part.width)].to_vec())
        .collect();
    ensure!(
        got == want,
        "{format:?}: copied from a buffer into part of a texture"
    );
    // Between two textures of the format, from (2, 1) to (0, 0).
    let (v, _) = upload(b, format, Some(&vec![0; len]), TextureUsage::empty())?;
    b.copy_texture(&at(&t, 2, 1), &at(&v, 0, 0), part);
    let got = read(b, &at(&v, 0, 0), part, want.len());
    ensure!(got == want, "{format:?}: copied between textures");
    Ok(())
}

/// The largest difference between two lists of floats.
pub fn largest(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

/// How far loads may lie from [`expected`], in 8-bit steps: as far as the RTX 3080's Direct3D 11
/// loads lie (0.72 for 5 and 6 bits; 0.96 for sRGB decoded).
pub fn load_bound(format: Format) -> f32 {
    if format == Format::Bgrx8UnormSrgb {
        1.0 / 255.0
    } else {
        0.72 / 255.0
    }
}

/// Every check without a reference: what is declared, copies exact, texels loaded within
/// [`load_bound`] of Direct3D's rule and filtered within one and a half 8-bit steps; 8-bit alpha drawn with its colour kept zero and its alpha within one step
/// of the blend, BGRX drawn with X kept one. One line of the largest differences.
pub fn check(b: &mut B) -> Result<String> {
    let mut line = String::from("converted formats:");
    for format in FORMATS {
        let uses = b.caps().formats.get(format);
        let drawn = DRAWN.contains(&format);
        let want = FormatUse::SAMPLE | FormatUse::FILTER | FormatUse::COPY_OUT;
        ensure!(uses.contains(want), "{format:?} declares {uses:?}");
        ensure!(
            uses.contains(FormatUse::RENDER | FormatUse::BLEND) == drawn
                && !uses.intersects(FormatUse::STORAGE_READ | FormatUse::STORAGE_WRITE),
            "{format:?} declares {uses:?}"
        );
    }
    for (i, format) in FORMATS.into_iter().enumerate() {
        let texels = texels(format, 1 + i as u64);
        copies(b, format, &texels)?;
        let (_, view) = upload(b, format, Some(&texels), TextureUsage::empty())?;
        let (loaded, filter) = sampled(b, &view)?;
        let want = expected(format, &texels);
        let (l, f) = (largest(&loaded, &want), largest(&filter, &filtered(&want)));
        ensure!(
            l <= load_bound(format),
            "{format:?}: loaded {l} from v / (2^n - 1)"
        );
        ensure!(
            f <= 1.5 / 255.0,
            "{format:?}: filtered {f} from v / (2^n - 1)"
        );
        line += &format!(
            " {format:?} loaded {:.3} filtered {:.3} steps;",
            l * 255.0,
            f * 255.0
        );
    }
    let (bytes, loaded) = drawn(b, Format::A8Unorm)?;
    let as_bytes: Vec<f32> = bytes.iter().map(|&a| f32::from(a)).collect();
    let blend = largest(&as_bytes, &blended());
    ensure!(
        blend <= 1.0,
        "8-bit alpha drawn {blend} steps from the blend"
    );
    ensure!(
        largest(&loaded, &expected(Format::A8Unorm, &bytes)) < 1e-6,
        "8-bit alpha drawn keeps a colour or loads other alphas"
    );
    line += &format!(" A8Unorm drawn {blend:.2} steps from the blend;");
    // BGRX drawn: alpha masked, so X stays one; loads as its bytes say.
    for format in [Format::Bgrx8Unorm, Format::Bgrx8UnormSrgb] {
        let (bytes, loaded) = drawn(b, format)?;
        ensure!(
            bytes.as_chunks::<4>().0.iter().all(|t| t[3] == 255),
            "{format:?} drawn with an X other than one"
        );
        let l = largest(&loaded, &expected(format, &bytes));
        ensure!(
            l <= load_bound(format),
            "{format:?} drawn loads {l} from its bytes"
        );
    }
    let hits = b.slow_paths().count(SlowPath::FormatConverted);
    ensure!(hits > 0, "FormatConverted was not counted");
    ensure!(
        b.status() == DeviceStatus::Ok,
        "device lost: {:?}",
        b.status()
    );
    line += &format!(" FormatConverted {hits}");
    Ok(line)
}
