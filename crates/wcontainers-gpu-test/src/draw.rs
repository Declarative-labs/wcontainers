//! The recording back end's draws: vertex fetch, the programs run on the CPU, clipping, rasterising
//! by Direct3D's rules (vertices snapped to 1/256 pixel, pixel centres, the top-left rule), depth and
//! stencil tests, and blending. Sampling and loads of textures are here too.

use crate::program::{self, Error, Level, Resources, StorageWrite, Value};
use crate::recorder::{
    Bound, DepthOps, Obj, Recorder, TextureObj, as_kind, level_size, subresource,
};
use crate::texel;
use naga::{Binding, BuiltIn, Interpolation, ScalarKind, TypeInner};
use std::cell::RefCell;
use wcontainers_gpu::*;

/// One value a vertex passes on.
#[derive(Clone, Debug)]
struct Varying {
    location: u32,
    interpolation: Interpolation,
    numbers: Vec<f64>,
}

/// A vertex in clip space, its values split by how they are interpolated.
#[derive(Clone, Debug)]
struct ClipVertex {
    position: [f64; 4],
    perspective: Vec<f64>,
    linear: Vec<f64>,
}

/// Where each of a draw's values lives in a [`ClipVertex`].
struct ValueLayout {
    entries: Vec<(Varying, usize)>,
}

/// A draw's attachment: the bytes of its one subresource, taken out of the texture while the draw
/// writes them, so the draw may read the texture's other subresources.
struct Target {
    texture: crate::recorder::Id,
    sub: usize,
    slice: u32,
    format: Format,
    width: u32,
    height: u32,
    bytes: Vec<u8>,
}

impl Target {
    /// Sample `sample` of a texel: a multisampled target (2D, slice 0) holds its samples as planes.
    fn texel(&mut self, x: u32, y: u32, sample: u32) -> &mut [u8] {
        let size = self.format.block().2 as usize;
        let at = (((self.slice + sample) * self.height + y) * self.width + x) as usize * size;
        &mut self.bytes[at..at + size]
    }
}

/// The resources a draw's binding sets name.
struct Bindings<'a> {
    rec: &'a Recorder,
    sets: &'a [Option<Vec<(u32, Bound)>>; MAX_BINDING_SETS],
    /// The draw's writes into storage buffers, applied when it ends.
    writes: &'a RefCell<Vec<StorageWrite>>,
}

impl Bindings<'_> {
    fn find(&self, group: u32, binding: u32) -> Result<&Bound, Error> {
        self.sets
            .get(group as usize)
            .and_then(Option::as_ref)
            .and_then(|s| s.iter().find(|(b, _)| *b == binding))
            .map(|(_, r)| r)
            .ok_or_else(|| format!("nothing bound at group {group} binding {binding}"))
    }

    fn texture(&self, at: (u32, u32)) -> Result<crate::recorder::Id, Error> {
        match self.find(at.0, at.1)? {
            Bound::Texture(v) => Ok(*v),
            _ => Err(format!("no texture at {at:?}")),
        }
    }
}

impl Resources for Bindings<'_> {
    fn buffer(&self, group: u32, binding: u32) -> Result<&[u8], Error> {
        let Bound::Buffer {
            buffer,
            offset,
            size,
            ..
        } = *self.find(group, binding)?
        else {
            return Err(format!("no buffer at group {group} binding {binding}"));
        };
        self.rec
            .buffer(buffer)?
            .bytes
            .get(offset as usize..(offset + size) as usize)
            .ok_or_else(|| "a bound range past the buffer's end".into())
    }

    fn sample(
        &self,
        image: (u32, u32),
        sampler: (u32, u32),
        coord: [f32; 3],
        layer: Option<i64>,
        level: Level,
        compare: Option<f32>,
    ) -> Result<[f64; 4], Error> {
        let view = self.texture(image)?;
        let Bound::Sampler(s) = *self.find(sampler.0, sampler.1)? else {
            return Err(format!("no sampler at {sampler:?}"));
        };
        let Obj::Sampler(s) = *self.rec.obj(s)? else {
            return Err("a sampler binding holds something else".into());
        };
        sample_view(self.rec, view, &s, coord, layer, level, compare)
    }

    fn load(
        &self,
        image: (u32, u32),
        coord: [i64; 3],
        layer: Option<i64>,
        level: i64,
        sample: Option<i64>,
    ) -> Result<[f64; 4], Error> {
        let view = self.texture(image)?;
        let v = self.rec.view(view)?;
        let t = self.rec.texture(v.texture)?;
        if level < 0 || level >= i64::from(v.desc.mip_count) {
            return Ok([0.0; 4]);
        }
        if sample.is_some() != (t.desc.samples > 1) {
            return Err("a load naming a sample from a single-sampled texture, or none from a multisampled one".into());
        }
        let mip = v.desc.base_mip + level as u32;
        let (w, h, d) = level_size(&t.desc, mip);
        let layer = layer.unwrap_or(0);
        let inside = |c: i64, n: u32| c >= 0 && c < i64::from(n);
        // A multisampled texture holds its samples as planes, where a 3D one holds its slices.
        let (z, d) = match sample {
            Some(n) => (n, t.desc.samples),
            None if v.desc.dimension == ViewDimension::D3 => (coord[2], d),
            None => (0, d),
        };
        if !inside(coord[0], w)
            || !inside(coord[1], h)
            || !inside(z, d)
            || !inside(layer, v.desc.layer_count)
        {
            return Ok([0.0; 4]);
        }
        Ok(fetch(
            t,
            &v.desc,
            mip,
            v.desc.base_layer + layer as u32,
            [coord[0] as u32, coord[1] as u32, z as u32],
            None,
        ))
    }

    fn size(&self, image: (u32, u32), level: i64) -> Result<[u32; 3], Error> {
        let v = self.rec.view(self.texture(image)?)?;
        let t = self.rec.texture(v.texture)?;
        let (w, h, d) = level_size(&t.desc, v.desc.base_mip + level as u32);
        Ok([
            w,
            h,
            if v.desc.dimension == ViewDimension::D3 {
                d
            } else {
                v.desc.layer_count
            },
        ])
    }
}

/// Whether `new` passes against `stored`.
fn compare(f: CompareFn, new: f64, stored: f64) -> bool {
    match f {
        CompareFn::Never => false,
        CompareFn::Less => new < stored,
        CompareFn::Equal => new == stored,
        CompareFn::LessEqual => new <= stored,
        CompareFn::Greater => new > stored,
        CompareFn::NotEqual => new != stored,
        CompareFn::GreaterEqual => new >= stored,
        CompareFn::Always => true,
    }
}

/// One texel of a texture seen through a view's format, depth compared with `reference` if given.
fn fetch(
    t: &TextureObj,
    view: &TextureViewDesc,
    mip: u32,
    layer: u32,
    at: [u32; 3],
    reference: Option<(CompareFn, f64)>,
) -> [f64; 4] {
    let (w, h, _) = level_size(&t.desc, mip);
    let size = t.desc.format.block().2 as usize;
    let sub = subresource(
        &t.desc,
        mip,
        if t.desc.dimension == TextureDimension::D3 {
            0
        } else {
            layer
        },
    );
    let i = (((at[2] * h + at[1]) * w + at[0]) as usize) * size;
    let bytes = &t.subresources[sub][i..i + size];
    let format = view.format;
    if format.is_depth_stencil() {
        if view.stencil {
            let s = texel::stencil_byte(format).map_or(0, |b| bytes[b]);
            return [f64::from(s), 0.0, 0.0, 1.0];
        }
        let d = texel::decode_depth(format, bytes);
        return match reference {
            Some((f, r)) => {
                let v = f64::from(u8::from(compare(f, r, d)));
                [v, v, v, 1.0]
            }
            None => [d, 0.0, 0.0, 1.0],
        };
    }
    texel::decode(format, bytes)
}

fn address(mode: AddressMode, i: i64, n: u32) -> u32 {
    let n = i64::from(n);
    (match mode {
        AddressMode::Repeat => i.rem_euclid(n),
        AddressMode::Mirror => {
            let m = i.rem_euclid(2 * n);
            if m >= n { 2 * n - 1 - m } else { m }
        }
        _ => i.clamp(0, n - 1),
    }) as u32
}

fn sample_view(
    rec: &Recorder,
    view: crate::recorder::Id,
    s: &SamplerDesc,
    coord: [f32; 3],
    layer: Option<i64>,
    level: Level,
    reference: Option<f32>,
) -> Result<[f64; 4], Error> {
    let v = rec.view(view)?;
    let t = rec.texture(v.texture)?;
    if matches!(
        v.desc.dimension,
        ViewDimension::Cube | ViewDimension::CubeArray
    ) {
        return Err("cube sampling is not done by the recording back end yet".into());
    }
    let levels = v.desc.mip_count;
    let lod = match level {
        Level::Exact(l) => l,
        Level::Auto | Level::Bias(_) if levels > 1 => {
            return Err("implicit level of detail over several levels needs derivatives, which the recording back end does not compute".into());
        }
        Level::Auto => 0.0,
        Level::Bias(b) => b,
    };
    let filter = if lod > 0.0 { s.min } else { s.mag };
    let linear = Filter::Linear;
    if (filter == linear || (s.mip == linear && levels > 1))
        && reference.is_none()
        && !rec
            .caps()
            .formats
            .get(v.desc.format)
            .contains(FormatUse::FILTER)
    {
        return Err(format!(
            "{:?} filtered, which this back end does not filter",
            v.desc.format
        ));
    }
    let lod = f64::from(lod)
        .clamp(
            f64::from(f32::from_bits(s.lod_range[0])),
            f64::from(f32::from_bits(s.lod_range[1])),
        )
        .clamp(0.0, f64::from(levels - 1));
    let picks: Vec<(u32, f64)> = match s.mip {
        Filter::Nearest => vec![((lod + 0.5).floor() as u32, 1.0)],
        Filter::Linear => {
            let lo = lod.floor();
            vec![
                (lo as u32, 1.0 - (lod - lo)),
                (((lo as u32) + 1).min(levels - 1), lod - lo),
            ]
        }
    };
    let layer = match v.desc.dimension {
        ViewDimension::D2Array => {
            v.desc.base_layer
                + layer
                    .unwrap_or(0)
                    .clamp(0, i64::from(v.desc.layer_count) - 1) as u32
        }
        _ => v.desc.base_layer,
    };
    let three_d = v.desc.dimension == ViewDimension::D3;
    let reference = reference.map(|r| (s.compare.unwrap_or(CompareFn::Always), f64::from(r)));
    let mut out = [0.0; 4];
    for (l, weight) in picks {
        if weight == 0.0 {
            continue;
        }
        let mip = v.desc.base_mip + l;
        let (w, h, d) = level_size(&t.desc, mip);
        let sizes = [w, h, if three_d { d } else { 1 }];
        let axes = if three_d { 3 } else { 2 };
        // Per axis: the texels and weights the filter reads.
        let mut taps: Vec<Vec<(u32, f64)>> = Vec::new();
        for a in 0..3 {
            if a >= axes {
                taps.push(vec![(0, 1.0)]);
                continue;
            }
            let x = f64::from(coord[a]) * f64::from(sizes[a]);
            taps.push(match filter {
                Filter::Nearest => vec![(address(s.address[a], x.floor() as i64, sizes[a]), 1.0)],
                Filter::Linear => {
                    let x = x - 0.5;
                    let i = x.floor();
                    let f = x - i;
                    vec![
                        (address(s.address[a], i as i64, sizes[a]), 1.0 - f),
                        (address(s.address[a], i as i64 + 1, sizes[a]), f),
                    ]
                }
            });
        }
        for &(z, wz) in &taps[2] {
            for &(y, wy) in &taps[1] {
                for &(x, wx) in &taps[0] {
                    let texel = fetch(t, &v.desc, mip, layer, [x, y, z], reference);
                    let k = weight * wx * wy * wz;
                    for c in 0..4 {
                        out[c] += k * texel[c];
                    }
                }
            }
        }
    }
    Ok(out)
}

/// The entry point of a module the program names.
fn entry(module: &ShaderModule) -> Result<usize, Error> {
    module
        .module
        .entry_points
        .iter()
        .position(|e| e.name == module.entry_point)
        .ok_or_else(|| format!("no entry point {}", module.entry_point))
}

/// The arguments of an entry point, each input made by `input`.
fn arguments(
    module: &naga::Module,
    fun: &naga::Function,
    mut input: impl FnMut(&Binding, naga::Handle<naga::Type>) -> Result<Value, Error>,
) -> Result<Vec<Value>, Error> {
    fun.arguments
        .iter()
        .map(|a| match &a.binding {
            Some(b) => input(b, a.ty),
            None => match &module.types[a.ty].inner {
                TypeInner::Struct { members, .. } => Ok(Value::Composite(
                    members
                        .iter()
                        .map(|m| {
                            input(
                                m.binding.as_ref().ok_or("an input without a binding")?,
                                m.ty,
                            )
                        })
                        .collect::<Result<_, Error>>()?,
                )),
                _ => Err("an input without a binding".into()),
            },
        })
        .collect()
}

/// The outputs of an entry point with their bindings and types.
fn outputs(
    module: &naga::Module,
    fun: &naga::Function,
    value: Value,
) -> Result<Vec<(Binding, naga::Handle<naga::Type>, Value)>, Error> {
    let Some(result) = &fun.result else {
        return Ok(Vec::new());
    };
    match &result.binding {
        Some(b) => Ok(vec![(b.clone(), result.ty, value)]),
        None => match &module.types[result.ty].inner {
            TypeInner::Struct { members, .. } => members
                .iter()
                .zip(value.components())
                .map(|(m, v)| {
                    Ok((
                        m.binding.clone().ok_or("an output without a binding")?,
                        m.ty,
                        v,
                    ))
                })
                .collect(),
            _ => Err("an output without a binding".into()),
        },
    }
}

/// The scalar kind and component count of a type.
fn shape(
    module: &naga::Module,
    ty: naga::Handle<naga::Type>,
) -> Result<(ScalarKind, usize), Error> {
    match module.types[ty].inner {
        TypeInner::Scalar(s) => Ok((s.kind, 1)),
        TypeInner::Vector { size, scalar } => Ok((scalar.kind, size as usize)),
        ref other => Err(format!("a stage value of type {other:?}")),
    }
}

fn make_value(kind: ScalarKind, numbers: &[f64]) -> Value {
    if numbers.len() == 1 {
        as_kind(numbers[0], kind)
    } else {
        Value::Composite(numbers.iter().map(|&n| as_kind(n, kind)).collect())
    }
}

/// Component count, bytes per component and number type of a vertex format.
fn vertex_format(f: VertexFormat) -> Result<(usize, usize, char), Error> {
    use VertexFormat::*;
    Ok(match f {
        Uint8x1 | Uint8x2 | Uint8x3 | Uint8x4 => (f as usize - Uint8x1 as usize + 1, 1, 'u'),
        Sint8x1 | Sint8x2 | Sint8x3 | Sint8x4 => (f as usize - Sint8x1 as usize + 1, 1, 'i'),
        Unorm8x1 | Unorm8x2 | Unorm8x3 | Unorm8x4 => (f as usize - Unorm8x1 as usize + 1, 1, 'n'),
        Snorm8x1 | Snorm8x2 | Snorm8x3 | Snorm8x4 => (f as usize - Snorm8x1 as usize + 1, 1, 's'),
        Uint16x1 | Uint16x2 | Uint16x3 | Uint16x4 => (f as usize - Uint16x1 as usize + 1, 2, 'u'),
        Sint16x1 | Sint16x2 | Sint16x3 | Sint16x4 => (f as usize - Sint16x1 as usize + 1, 2, 'i'),
        Unorm16x1 | Unorm16x2 | Unorm16x3 | Unorm16x4 => {
            (f as usize - Unorm16x1 as usize + 1, 2, 'n')
        }
        Snorm16x1 | Snorm16x2 | Snorm16x3 | Snorm16x4 => {
            (f as usize - Snorm16x1 as usize + 1, 2, 's')
        }
        Float16x1 | Float16x2 | Float16x3 | Float16x4 => {
            (f as usize - Float16x1 as usize + 1, 2, 'f')
        }
        Float32x1 | Float32x2 | Float32x3 | Float32x4 => {
            (f as usize - Float32x1 as usize + 1, 4, 'f')
        }
        Uint32x1 | Uint32x2 | Uint32x3 | Uint32x4 => (f as usize - Uint32x1 as usize + 1, 4, 'u'),
        Sint32x1 | Sint32x2 | Sint32x3 | Sint32x4 => (f as usize - Sint32x1 as usize + 1, 4, 'i'),
        other => {
            return Err(format!(
                "vertex format {other:?} is not read by the recording back end yet"
            ));
        }
    })
}

fn decode_vertex(f: VertexFormat, bytes: &[u8]) -> Result<Vec<f64>, Error> {
    let (n, size, num) = vertex_format(f)?;
    let mut out = vec![0.0, 0.0, 0.0, 1.0];
    for (c, o) in out.iter_mut().enumerate().take(n) {
        let b = bytes
            .get(c * size..c * size + size)
            .ok_or("a vertex read past the buffer's end")?;
        let bits = b.iter().rev().fold(0u64, |a, &x| (a << 8) | u64::from(x));
        let signed = ((bits << (64 - size * 8)) as i64) >> (64 - size * 8);
        let max = ((1u64 << (size * 8)) - 1) as f64;
        *o = match num {
            'u' => bits as f64,
            'i' => signed as f64,
            'n' => bits as f64 / max,
            's' => (signed as f64 / ((1u64 << (size * 8 - 1)) - 1) as f64).max(-1.0),
            _ if size == 2 => f64::from(half::f16::from_bits(bits as u16).to_f32()),
            _ => f64::from(f32::from_bits(bits as u32)),
        };
    }
    Ok(out)
}

fn blend_factor(
    f: BlendFactor,
    c: usize,
    src: [f64; 4],
    dst: [f64; 4],
    k: [f64; 4],
) -> Result<f64, Error> {
    use BlendFactor::*;
    Ok(match f {
        Zero => 0.0,
        One => 1.0,
        Src => src[c],
        OneMinusSrc => 1.0 - src[c],
        SrcAlpha => src[3],
        OneMinusSrcAlpha => 1.0 - src[3],
        Dst => dst[c],
        OneMinusDst => 1.0 - dst[c],
        DstAlpha => dst[3],
        OneMinusDstAlpha => 1.0 - dst[3],
        SrcAlphaSaturated => {
            if c == 3 {
                1.0
            } else {
                src[3].min(1.0 - dst[3])
            }
        }
        Constant => k[c],
        OneMinusConstant => 1.0 - k[c],
        ConstantAlpha => k[3],
        OneMinusConstantAlpha => 1.0 - k[3],
        Src1 | OneMinusSrc1 | Src1Alpha | OneMinusSrc1Alpha => {
            return Err("dual-source blending, which this back end reports absent".into());
        }
    })
}

fn blend_op(op: BlendOp, s: f64, d: f64) -> f64 {
    match op {
        BlendOp::Add => s + d,
        BlendOp::Subtract => s - d,
        BlendOp::ReverseSubtract => d - s,
        BlendOp::Min => s.min(d),
        BlendOp::Max => s.max(d),
    }
}

fn stencil_op(op: StencilOp, old: u8, reference: u8) -> u8 {
    match op {
        StencilOp::Keep => old,
        StencilOp::Zero => 0,
        StencilOp::Replace => reference,
        StencilOp::IncrementClamp => old.saturating_add(1),
        StencilOp::DecrementClamp => old.saturating_sub(1),
        StencilOp::Invert => !old,
        StencilOp::IncrementWrap => old.wrapping_add(1),
        StencilOp::DecrementWrap => old.wrapping_sub(1),
    }
}

/// Clips a polygon to the side of a plane where `distance` is not negative.
fn clip(polygon: Vec<ClipVertex>, distance: impl Fn(&[f64; 4]) -> f64) -> Vec<ClipVertex> {
    let mut out = Vec::new();
    for i in 0..polygon.len() {
        let (a, b) = (&polygon[i], &polygon[(i + 1) % polygon.len()]);
        let (da, db) = (distance(&a.position), distance(&b.position));
        if da >= 0.0 {
            out.push(a.clone());
        }
        if (da >= 0.0) != (db >= 0.0) {
            let t = da / (da - db);
            let lerp = |x: &[f64], y: &[f64], t: f64| -> Vec<f64> {
                x.iter().zip(y).map(|(x, y)| x + (y - x) * t).collect()
            };
            // Values without perspective are interpolated along the edge as seen on the screen.
            let (wa, wb) = (a.position[3], b.position[3]);
            let s = t * wb / ((1.0 - t) * wa + t * wb);
            let p = lerp(&a.position, &b.position, t);
            out.push(ClipVertex {
                position: [p[0], p[1], p[2], p[3]],
                perspective: lerp(&a.perspective, &b.perspective, t),
                linear: lerp(&a.linear, &b.linear, s),
            });
        }
    }
    out
}

struct DrawState {
    vertex: std::rc::Rc<ShaderModule>,
    fragment: Option<std::rc::Rc<ShaderModule>>,
    blend: BlendDesc,
    depth_stencil: DepthStencilDesc,
    raster: RasterDesc,
    layout: VertexLayoutDesc,
    sets: [Option<Vec<(u32, Bound)>>; MAX_BINDING_SETS],
    viewport: [f64; 6],
    scissor: Option<ScissorRect>,
}

impl Recorder {
    fn gather(&self, call: &DrawCall<'_, Self>) -> Result<DrawState, Error> {
        let (vertex, fragment) = match self.obj(*call.program)? {
            Obj::Program(p) => (p.vertex.clone(), p.fragment.clone()),
            _ => return Err("the draw's program is not a program".into()),
        };
        let Obj::Blend(blend) = self.obj(*call.blend)? else {
            return Err("the draw's blend block is not one".into());
        };
        let Obj::DepthStencil(depth_stencil) = *self.obj(*call.depth_stencil)? else {
            return Err("the draw's depth-stencil block is not one".into());
        };
        let Obj::Raster(raster) = *self.obj(*call.raster)? else {
            return Err("the draw's raster block is not one".into());
        };
        let Obj::VertexLayout(layout) = self.obj(*call.vertex_layout)? else {
            return Err("the draw's vertex layout block is not one".into());
        };
        let mut sets: [Option<Vec<(u32, Bound)>>; MAX_BINDING_SETS] = Default::default();
        for (slot, set) in sets.iter_mut().zip(call.bindings) {
            if let Some(set) = set {
                match self.obj(*set)? {
                    Obj::BindingSet(s) => *slot = Some(s.clone()),
                    _ => return Err("a binding set that is not one".into()),
                }
            }
        }
        let vp = call.viewports.first().ok_or("a draw without a viewport")?;
        let f = |b: u32| f64::from(f32::from_bits(b));
        let viewport = [
            f(vp.x),
            f(vp.y),
            f(vp.width),
            f(vp.height),
            f(vp.min_depth),
            f(vp.max_depth),
        ];
        let scissor = if raster.scissor {
            Some(
                *call
                    .scissors
                    .first()
                    .ok_or("scissor on without a scissor")?,
            )
        } else {
            None
        };
        Ok(DrawState {
            vertex,
            fragment,
            blend: **blend,
            depth_stencil,
            raster,
            layout: **layout,
            sets,
            viewport,
            scissor,
        })
    }

    /// Checks the draw's own values against the rules and the features this back end declares.
    fn draw_rules(&self, call: &DrawCall<'_, Self>) -> Result<(), Error> {
        let has = self.caps().features;
        for v in call.viewports {
            if f32::from_bits(v.min_depth) > f32::from_bits(v.max_depth) {
                return Err("a viewport whose min_depth is above its max_depth".into());
            }
            if !has.contains(Features::FRACTIONAL_VIEWPORT)
                && [v.x, v.y, v.width, v.height]
                    .iter()
                    .any(|&b| f32::from_bits(b).fract() != 0.0)
            {
                return Err("a fractional viewport without FRACTIONAL_VIEWPORT".into());
            }
        }
        if call.stencil_reference[0] != call.stencil_reference[1]
            && !has.contains(Features::STENCIL_PER_FACE)
        {
            return Err("stencil references per face without STENCIL_PER_FACE".into());
        }
        let bases = match call.range {
            DrawRange::Vertices { first_instance, .. } => first_instance != 0,
            DrawRange::Indexed {
                base_vertex,
                first_instance,
                ..
            } => base_vertex != 0 || first_instance != 0,
            _ => false,
        };
        if bases && !has.contains(Features::BASE_VERTEX_INSTANCE) {
            return Err("a base vertex or first instance without BASE_VERTEX_INSTANCE".into());
        }
        if call
            .index
            .as_ref()
            .is_some_and(|i| i.format == IndexFormat::U8)
            && !has.contains(Features::INDEX_U8)
        {
            return Err("8-bit indices without INDEX_U8".into());
        }
        if call.immediates.len() > self.caps().limits.immediate_bytes as usize {
            return Err("immediates past the limit".into());
        }
        Ok(())
    }

    /// Checks the draw's buffers were made for how it uses them, and the usage-scope rule of a pass: a
    /// buffer any draw of the pass binds as writable storage is used no other way by any draw of the
    /// pass (texture subresources cannot be storage here).
    fn usage_scope(&mut self, call: &DrawCall<'_, Self>, state: &DrawState) -> Result<(), Error> {
        let mut uses: Vec<(crate::recorder::Id, bool)> = Vec::new();
        uses.extend(
            call.vertex_buffers
                .iter()
                .flatten()
                .map(|v| (*v.buffer, false)),
        );
        uses.extend(call.index.iter().map(|i| (*i.buffer, false)));
        if let DrawRange::Indirect { args, .. } | DrawRange::IndexedIndirect { args, .. } =
            call.range
        {
            uses.push((*args, false));
        }
        for (_, bound) in state.sets.iter().flatten().flatten() {
            if let Bound::Buffer {
                buffer, writable, ..
            } = bound
            {
                uses.push((*buffer, *writable));
            }
        }
        for v in call.vertex_buffers.iter().flatten() {
            self.buffer_for(*v.buffer, BufferUsage::VERTEX, "a vertex stream")?;
        }
        if let Some(i) = &call.index {
            self.buffer_for(*i.buffer, BufferUsage::INDEX, "an index binding")?;
        }
        if let DrawRange::Indirect { args, .. } | DrawRange::IndexedIndirect { args, .. } =
            call.range
        {
            self.buffer_for(*args, BufferUsage::INDIRECT, "indirect arguments")?;
        }
        let pass = self.pass.as_mut().ok_or("draw outside a pass")?;
        for &(buffer, writable) in &uses {
            let (mine, theirs) = if writable {
                (&mut pass.written, &pass.other)
            } else {
                (&mut pass.other, &pass.written)
            };
            if !mine.contains(&buffer) {
                mine.push(buffer);
            }
            if theirs.contains(&buffer) {
                return Err(format!(
                    "a pass uses buffer {} as writable storage and another way",
                    buffer.0
                ));
            }
        }
        Ok(())
    }

    /// The buffer bytes a draw reads or writes: its index range, the elements its vertex streams
    /// fetch and the ranges its binding sets bind.
    fn draw_reads(
        &self,
        call: &DrawCall<'_, Self>,
        state: &DrawState,
    ) -> Result<Vec<(crate::recorder::Id, u64, u64)>, Error> {
        let mut reads = Vec::new();
        for (_, bound) in state.sets.iter().flatten().flatten() {
            if let Bound::Buffer {
                buffer,
                offset,
                size,
                ..
            } = *bound
            {
                reads.push((buffer, offset, offset + size));
            }
        }
        if let (Some(i), DrawRange::Indexed { first, count, .. }) = (&call.index, &call.range) {
            let size = u64::from(i.format.size());
            let start = i.offset + u64::from(*first) * size;
            reads.push((*i.buffer, start, start + u64::from(*count) * size));
        }
        let (list, first_instance, instances) = self.vertex_list(call)?;
        let vertices = list.iter().flatten();
        let (lo, hi) = (vertices.clone().min(), vertices.max());
        for (n, stream) in state.layout.streams.iter().enumerate() {
            let (Some(stream), Some(Some(binding))) = (stream, call.vertex_buffers.get(n)) else {
                continue;
            };
            let (first, last) = match stream.step {
                StepMode::Vertex => match (lo, hi) {
                    (Some(&lo), Some(&hi)) => (lo, hi),
                    _ => continue,
                },
                StepMode::Instance(k) if instances > 0 => {
                    (first_instance, first_instance + (instances - 1) / k.max(1))
                }
                StepMode::Instance(_) => continue,
            };
            let mut ends = (u64::MAX, 0u64);
            for a in state.layout.attributes.iter().flatten() {
                if usize::from(a.stream) == n {
                    let (count, size, _) = vertex_format(a.format)?;
                    ends.0 = ends.0.min(u64::from(a.offset));
                    ends.1 = ends.1.max(u64::from(a.offset) + (count * size) as u64);
                }
            }
            if ends.1 > 0 {
                let stride = u64::from(stream.stride);
                reads.push((
                    *binding.buffer,
                    binding.offset + u64::from(first) * stride + ends.0,
                    binding.offset + u64::from(last) * stride + ends.1,
                ));
            }
        }
        Ok(reads)
    }

    /// Every (texture, subresource) the bound texture views cover.
    fn read_subresources(
        &self,
        sets: &[Option<Vec<(u32, Bound)>>; MAX_BINDING_SETS],
    ) -> Result<Vec<(crate::recorder::Id, usize)>, Error> {
        let mut out = Vec::new();
        for (_, bound) in sets.iter().flatten().flatten() {
            if let Bound::Texture(view) = bound {
                let v = self.view(*view)?;
                let desc = &self.texture(v.texture)?.desc;
                let layers = if desc.dimension == TextureDimension::D3 {
                    0..1
                } else {
                    v.desc.base_layer..v.desc.base_layer + v.desc.layer_count
                };
                for mip in v.desc.base_mip..v.desc.base_mip + v.desc.mip_count {
                    for layer in layers.clone() {
                        out.push((v.texture, subresource(desc, mip, layer)));
                    }
                }
            }
        }
        Ok(out)
    }

    /// The (fetch index, vertex index) of each vertex the range names, None where a strip restarts.
    fn vertex_list(
        &self,
        call: &DrawCall<'_, Self>,
    ) -> Result<(Vec<Option<u32>>, u32, u32), Error> {
        Ok(match call.range {
            DrawRange::Vertices {
                first,
                count,
                first_instance,
                instances,
            } => (
                (first..first + count).map(Some).collect(),
                first_instance,
                instances,
            ),
            DrawRange::Indexed {
                first,
                count,
                base_vertex,
                first_instance,
                instances,
            } => {
                let index = call
                    .index
                    .as_ref()
                    .ok_or("an indexed draw without indices")?;
                let bytes = &self.buffer(*index.buffer)?.bytes;
                let size = index.format.size() as usize;
                let mut list = Vec::new();
                for k in first..first + count {
                    let at = index.offset as usize + k as usize * size;
                    let b = bytes
                        .get(at..at + size)
                        .ok_or("an index past the buffer's end")?;
                    let v = b.iter().rev().fold(0u32, |a, &x| (a << 8) | u32::from(x));
                    if call.restart && v == index.format.restart_value() {
                        list.push(None);
                    } else {
                        list.push(Some((i64::from(v) + i64::from(base_vertex)) as u32));
                    }
                }
                (list, first_instance, instances)
            }
            DrawRange::Indirect { .. } | DrawRange::IndexedIndirect { .. } => {
                return Err("indirect draws, which this back end reports absent".into());
            }
        })
    }

    fn shade_vertex(
        &self,
        state: &DrawState,
        call: &DrawCall<'_, Self>,
        vertex: u32,
        instance: u32,
        first_instance: u32,
    ) -> Result<([f64; 4], Vec<Varying>), Error> {
        let m = &state.vertex.module;
        let ep = entry(&state.vertex)?;
        let fun = &m.entry_points[ep].function;
        let args = arguments(m, fun, |binding, ty| match *binding {
            Binding::BuiltIn(BuiltIn::VertexIndex) => Ok(Value::U32(vertex)),
            Binding::BuiltIn(BuiltIn::InstanceIndex) => Ok(Value::U32(instance)),
            Binding::BuiltIn(b) => Err(format!("vertex input {b:?}")),
            Binding::Location { location, .. } => {
                let a = state
                    .layout
                    .attributes
                    .iter()
                    .flatten()
                    .find(|a| a.location == location)
                    .ok_or_else(|| format!("no attribute at location {location}"))?;
                let stream = state.layout.streams[a.stream as usize]
                    .ok_or("an attribute of an unused stream")?;
                let bound = call
                    .vertex_buffers
                    .get(a.stream as usize)
                    .and_then(Option::as_ref)
                    .ok_or("an attribute of a stream with no buffer bound")?;
                let element = match stream.step {
                    StepMode::Vertex => vertex,
                    StepMode::Instance(0) => first_instance,
                    StepMode::Instance(n) => first_instance + (instance - first_instance) / n,
                };
                let at = bound.offset as usize
                    + element as usize * stream.stride as usize
                    + a.offset as usize;
                let bytes = self
                    .buffer(*bound.buffer)?
                    .bytes
                    .get(at..)
                    .ok_or("a vertex past the buffer's end")?;
                let numbers = decode_vertex(a.format, bytes)?;
                let (kind, n) = shape(m, ty)?;
                Ok(make_value(kind, &numbers[..n]))
            }
        })?;
        let writes = RefCell::default();
        let res = Bindings {
            rec: self,
            sets: &state.sets,
            writes: &writes,
        };
        let out = program::run(
            m,
            &state.vertex.info,
            ep,
            args,
            &res,
            &mut writes.borrow_mut(),
        )?
        .ok_or("a vertex program discarded")?;
        if !writes.borrow().is_empty() {
            return Err(
                "a vertex program writing storage, which this back end reports absent".into(),
            );
        }
        let mut position = None;
        let mut values = Vec::new();
        for (binding, ty, v) in outputs(m, fun, out)? {
            match binding {
                Binding::BuiltIn(BuiltIn::Position { .. }) => {
                    let n = v.numbers()?;
                    position = Some([n[0], n[1], n[2], n[3]]);
                }
                Binding::BuiltIn(b) => return Err(format!("vertex output {b:?}")),
                Binding::Location {
                    location,
                    interpolation,
                    ..
                } => {
                    let (kind, _) = shape(m, ty)?;
                    let interpolation = match (kind, interpolation) {
                        (ScalarKind::Float, Some(i)) => i,
                        (ScalarKind::Float, None) => Interpolation::Perspective,
                        _ => Interpolation::Flat,
                    };
                    values.push(Varying {
                        location,
                        interpolation,
                        numbers: v.numbers()?,
                    });
                }
            }
        }
        values.sort_by_key(|v| v.location);
        Ok((
            position.ok_or("a vertex program without a position")?,
            values,
        ))
    }

    pub(crate) fn draw_call(&mut self, call: &DrawCall<'_, Self>) -> Result<(), Error> {
        self.pass.as_ref().ok_or("draw outside a pass")?;
        let state = self.gather(call)?;
        self.draw_rules(call)?;
        self.usage_scope(call, &state)?;
        for (buffer, start, end) in self.draw_reads(call, &state)? {
            self.touch(buffer, start, end);
        }
        let pass = self.pass.as_ref().ok_or("draw outside a pass")?;
        match call.topology {
            Topology::TriangleList | Topology::TriangleStrip => {}
            Topology::TriangleFan | Topology::LineLoop => {
                return Err(
                    "fans and loops go through draw_fan_or_loop on a back end without FAN_AND_LOOP"
                        .into(),
                );
            }
            t => return Err(format!("{t:?} is not drawn by the recording back end yet")),
        }
        if call.restart && call.topology == Topology::TriangleList {
            return Err("restart on a list topology".into());
        }
        if state.blend.alpha_to_coverage {
            return Err("alpha to coverage is not drawn by the recording back end yet".into());
        }
        // Take the attachments out while the draw writes them.
        let mut colors: Vec<Option<Target>> = Vec::new();
        let color_views: Vec<_> = pass
            .colors
            .iter()
            .map(|c| c.map(|(v, s, _)| (v, s)))
            .collect();
        let depth_view = pass.depth;
        let (pw, ph, samples) = (pass.width, pass.height, pass.samples);
        let read = self.read_subresources(&state.sets)?;
        let take = |r: &mut Self, view, slice, read_only: bool| -> Result<Target, Error> {
            let (texture, sub, slice) = r.attachment(view, slice)?;
            let format = r.view(view)?.desc.format;
            let (width, height) = r.view_size(view)?;
            if read.contains(&(texture, sub)) && !read_only {
                return Err(
                    "a subresource bound for reading while it is a written attachment".into(),
                );
            }
            let Some(Obj::Texture(t)) = r.objects[texture.0 as usize].as_mut() else {
                return Err("an attachment that is not a texture".into());
            };
            let bytes = if read_only {
                t.subresources[sub].clone()
            } else {
                std::mem::take(&mut t.subresources[sub])
            };
            if bytes.is_empty() {
                return Err("an attachment attached twice".into());
            }
            Ok(Target {
                texture,
                sub,
                slice,
                format,
                width,
                height,
                bytes,
            })
        };
        let mut result = Ok(());
        for c in color_views {
            match c.map(|(v, s)| take(self, v, s, false)).transpose() {
                Ok(t) => colors.push(t),
                Err(e) => {
                    result = Err(e);
                    colors.push(None);
                }
            }
        }
        // A depth attachment whose every aspect is read-only may also be bound for reading.
        let depth_read_only =
            |(_, d, s): DepthOps| d.is_none_or(|a| a.read_only) && s.is_none_or(|a| a.read_only);
        let mut depth = match depth_view
            .map(|o| take(self, o.0, 0, depth_read_only(o)))
            .transpose()
        {
            Ok(d) => d,
            Err(e) => {
                result = Err(e);
                None
            }
        };
        let mut passed = 0u64;
        let writes = RefCell::default();
        if result.is_ok() {
            result = self.rasterise(
                call,
                &state,
                &mut colors,
                depth.as_mut(),
                depth_view,
                (pw, ph, samples),
                &mut passed,
                &writes,
            );
        }
        for (group, binding, at, bytes) in writes.into_inner() {
            let Some(Bound::Buffer { buffer, offset, .. }) = state.sets[group as usize]
                .as_ref()
                .and_then(|s| s.iter().find(|(b, _)| *b == binding))
                .map(|(_, r)| r.clone())
            else {
                continue;
            };
            if let Some(Obj::Buffer(b)) = self.objects[buffer.0 as usize].as_mut() {
                let at = (offset + u64::from(at)) as usize;
                b.bytes[at..at + bytes.len()].copy_from_slice(&bytes);
            }
        }
        for t in colors.into_iter().flatten().chain(depth) {
            if let Some(Obj::Texture(o)) = self.objects[t.texture.0 as usize].as_mut() {
                o.subresources[t.sub] = t.bytes;
            }
        }
        if let Some(Some((_, n))) = self.pass.as_mut().map(|p| p.query.as_mut()) {
            *n += passed;
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn rasterise(
        &self,
        call: &DrawCall<'_, Self>,
        state: &DrawState,
        colors: &mut [Option<Target>],
        mut depth: Option<&mut Target>,
        depth_ops: Option<DepthOps>,
        (pw, ph, samples): (u32, u32, u32),
        passed: &mut u64,
        writes: &RefCell<Vec<StorageWrite>>,
    ) -> Result<(), Error> {
        let (list, first_instance, instances) = self.vertex_list(call)?;
        let raster = state.raster;
        let ds = state.depth_stencil;
        let depth_read_only = depth_ops
            .and_then(|(_, d, _)| d)
            .is_some_and(|a| a.read_only);
        let stencil_read_only = depth_ops
            .and_then(|(_, _, s)| s)
            .is_some_and(|a| a.read_only);
        if depth_read_only && ds.depth_write && ds.depth_compare.is_some() {
            return Err("a draw writing depth into a read-only attachment".into());
        }
        let k: Vec<f64> = call
            .blend_constant
            .iter()
            .map(|&b| f64::from(f32::from_bits(b)))
            .collect();
        let k = [k[0], k[1], k[2], k[3]];
        let res = Bindings {
            rec: self,
            sets: &state.sets,
            writes,
        };
        for instance in first_instance..first_instance + instances {
            // Shade each vertex once per instance.
            let mut shaded: std::collections::HashMap<u32, ([f64; 4], Vec<Varying>)> =
                Default::default();
            let mut triangles: Vec<[u32; 3]> = Vec::new();
            let mut strip: Vec<u32> = Vec::new();
            for v in &list {
                let Some(v) = *v else {
                    strip.clear();
                    continue;
                };
                if let std::collections::hash_map::Entry::Vacant(e) = shaded.entry(v) {
                    e.insert(self.shade_vertex(state, call, v, instance, first_instance)?);
                }
                strip.push(v);
                match call.topology {
                    Topology::TriangleList if strip.len() == 3 => {
                        triangles.push([strip[0], strip[1], strip[2]]);
                        strip.clear();
                    }
                    Topology::TriangleStrip if strip.len() >= 3 => {
                        let i = strip.len() - 3;
                        // Vulkan's strip order: odd triangles are (i, i + 2, i + 1), or
                        // (i + 1, i, i + 2) with the last vertex provoking, which keeps the
                        // provoking vertex at i or i + 2 as every API defines it.
                        triangles.push(match (i & 1, raster.provoking) {
                            (0, _) => [strip[i], strip[i + 1], strip[i + 2]],
                            (_, ProvokingVertex::First) => [strip[i], strip[i + 2], strip[i + 1]],
                            (_, ProvokingVertex::Last) => [strip[i + 1], strip[i], strip[i + 2]],
                        });
                    }
                    _ => {}
                }
            }
            for tri in triangles {
                let verts: Vec<&([f64; 4], Vec<Varying>)> =
                    tri.iter().map(|v| &shaded[v]).collect();
                let provoking = match raster.provoking {
                    ProvokingVertex::First => 0,
                    ProvokingVertex::Last => 2,
                };
                let layout = ValueLayout {
                    entries: {
                        let (mut p, mut l) = (0, 0);
                        verts[0]
                            .1
                            .iter()
                            .map(|v| {
                                let at = match v.interpolation {
                                    Interpolation::Linear => {
                                        l += v.numbers.len();
                                        l - v.numbers.len()
                                    }
                                    Interpolation::Perspective => {
                                        p += v.numbers.len();
                                        p - v.numbers.len()
                                    }
                                    _ => 0,
                                };
                                (v.clone(), at)
                            })
                            .collect()
                    },
                };
                let flat: Vec<Varying> = verts[provoking].1.clone();
                let polygon: Vec<ClipVertex> = verts
                    .iter()
                    .map(|(pos, vals)| {
                        let pick = |i: Interpolation| -> Vec<f64> {
                            vals.iter()
                                .filter(|v| v.interpolation == i)
                                .flat_map(|v| v.numbers.clone())
                                .collect()
                        };
                        ClipVertex {
                            position: *pos,
                            perspective: pick(Interpolation::Perspective),
                            linear: pick(Interpolation::Linear),
                        }
                    })
                    .collect();
                let polygon = clip(polygon, |p| p[2]);
                let polygon = clip(polygon, |p| p[3] - p[2]);
                if polygon.len() < 3 {
                    continue;
                }
                let [vx, vy, vw, vh, vmin, vmax] = state.viewport;
                let screen: Vec<[f64; 4]> = polygon
                    .iter()
                    .map(|v| {
                        let inv = 1.0 / v.position[3];
                        let (x, y, z) = (
                            v.position[0] * inv,
                            v.position[1] * inv,
                            v.position[2] * inv,
                        );
                        [
                            vx + (x + 1.0) * 0.5 * vw,
                            vy + (1.0 - y) * 0.5 * vh,
                            vmin + z * (vmax - vmin),
                            inv,
                        ]
                    })
                    .collect();
                for i in 1..polygon.len() - 1 {
                    let idx = [0, i, i + 1];
                    self.triangle(
                        call,
                        state,
                        &res,
                        &idx.map(|j| screen[j]),
                        &idx.map(|j| &polygon[j]),
                        &layout,
                        &flat,
                        colors,
                        depth.as_deref_mut(),
                        depth_read_only,
                        stencil_read_only,
                        k,
                        (pw, ph, samples),
                        passed,
                    )?;
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn triangle(
        &self,
        call: &DrawCall<'_, Self>,
        state: &DrawState,
        res: &Bindings<'_>,
        s: &[[f64; 4]; 3],
        v: &[&ClipVertex; 3],
        layout: &ValueLayout,
        flat: &[Varying],
        colors: &mut [Option<Target>],
        mut depth: Option<&mut Target>,
        depth_read_only: bool,
        stencil_read_only: bool,
        k: [f64; 4],
        (pw, ph, samples): (u32, u32, u32),
        passed: &mut u64,
    ) -> Result<(), Error> {
        let raster = state.raster;
        let ds = state.depth_stencil;
        let snap = |x: f64| (x * 256.0).round() as i64;
        let mut p: Vec<(i64, i64)> = s.iter().map(|q| (snap(q[0]), snap(q[1]))).collect();
        let mut order = [0usize, 1, 2];
        let area = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[2].0 - p[0].0) * (p[1].1 - p[0].1);
        if area == 0 {
            return Ok(());
        }
        // Positive area in y-down framebuffer coordinates is clockwise on the screen.
        let counter_clockwise = area < 0;
        let front = match raster.front {
            FrontFace::CounterClockwise => counter_clockwise,
            FrontFace::Clockwise => !counter_clockwise,
        };
        match raster.cull {
            CullMode::Front if front => return Ok(()),
            CullMode::Back if !front => return Ok(()),
            _ => {}
        }
        if area < 0 {
            order = [0, 2, 1];
            p = order
                .iter()
                .map(|&i| (snap(s[i][0]), snap(s[i][1])))
                .collect();
        }
        let area = area.abs();
        let edge = |a: (i64, i64), b: (i64, i64), q: (i64, i64)| {
            (b.0 - a.0) * (q.1 - a.1) - (b.1 - a.1) * (q.0 - a.0)
        };
        let top_left = |a: (i64, i64), b: (i64, i64)| (a.1 == b.1 && b.0 > a.0) || b.1 < a.1;
        let edges = [(p[1], p[2]), (p[2], p[0]), (p[0], p[1])];
        let biased: Vec<bool> = edges.iter().map(|&(a, b)| top_left(a, b)).collect();
        // The pixels to visit: the triangle's box within the viewport, the scissor and the targets.
        let [vx, vy, vw, vh, vmin, vmax] = state.viewport;
        let mut x0 = vx.max(0.0).floor() as i64;
        let mut y0 = vy.max(0.0).floor() as i64;
        let mut x1 = ((vx + vw).ceil() as i64).min(i64::from(pw));
        let mut y1 = ((vy + vh).ceil() as i64).min(i64::from(ph));
        if let Some(sc) = state.scissor {
            x0 = x0.max(i64::from(sc.x));
            y0 = y0.max(i64::from(sc.y));
            x1 = x1.min(i64::from(sc.x + sc.width));
            y1 = y1.min(i64::from(sc.y + sc.height));
        }
        x0 = x0.max(p.iter().map(|q| q.0).min().unwrap_or(0) >> 8);
        y0 = y0.max(p.iter().map(|q| q.1).min().unwrap_or(0) >> 8);
        x1 = x1.min((p.iter().map(|q| q.0).max().unwrap_or(0) >> 8) + 1);
        y1 = y1.min((p.iter().map(|q| q.1).max().unwrap_or(0) >> 8) + 1);
        // Depth bias, by Direct3D's rule for the attachment's format.
        let (zs, xs, ys): (Vec<f64>, Vec<f64>, Vec<f64>) = (
            order.map(|i| s[i][2]).to_vec(),
            order.map(|i| s[i][0]).to_vec(),
            order.map(|i| s[i][1]).to_vec(),
        );
        let bias = {
            let f = |b: u32| f64::from(f32::from_bits(b));
            let (constant, slope, clamp) = (
                f(raster.depth_bias),
                f(raster.depth_bias_slope),
                f(raster.depth_bias_clamp),
            );
            if constant == 0.0 && slope == 0.0 {
                0.0
            } else {
                let det = (xs[1] - xs[0]) * (ys[2] - ys[0]) - (xs[2] - xs[0]) * (ys[1] - ys[0]);
                let dzdx =
                    ((zs[1] - zs[0]) * (ys[2] - ys[0]) - (zs[2] - zs[0]) * (ys[1] - ys[0])) / det;
                let dzdy =
                    ((xs[1] - xs[0]) * (zs[2] - zs[0]) - (xs[2] - xs[0]) * (zs[1] - zs[0])) / det;
                let r = match depth.as_ref().map(|d| d.format) {
                    Some(Format::Depth16Unorm) => 2f64.powi(-16),
                    Some(Format::Depth32Float | Format::Depth32FloatStencil8) => {
                        let m = zs.iter().fold(0.0f64, |a, z| a.max(z.abs())) as f32;
                        2f64.powi(m.to_bits().wrapping_shr(23) as i32 - 127 - 23)
                    }
                    _ => 2f64.powi(-24),
                };
                let b = constant * r + slope * dzdx.abs().max(dzdy.abs());
                if clamp > 0.0 {
                    b.min(clamp)
                } else if clamp < 0.0 {
                    b.max(clamp)
                } else {
                    b
                }
            }
        };
        let fragment = state.fragment.as_deref();
        let stencil_at = depth.as_ref().and_then(|d| texel::stencil_byte(d.format));
        // Where each sample lies in its pixel, in 1/256 pixel: the standard positions of Direct3D and
        // Vulkan, which WebGPU's four samples take.
        let positions: &[(i64, i64)] = match samples {
            1 => &[(128, 128)],
            4 => &[(96, 32), (224, 96), (32, 160), (160, 224)],
            n => {
                return Err(format!(
                    "{n} samples are not drawn by the recording back end"
                ));
            }
        };
        // A program reading the sample index, or a value interpolated at the sample, runs per sample.
        let mut per_sample = false;
        if let Some(fs) = fragment {
            let fun = &fs.module.entry_points[entry(fs)?].function;
            arguments(&fs.module, fun, |binding, _| {
                per_sample |= matches!(
                    binding,
                    Binding::BuiltIn(BuiltIn::SampleIndex)
                        | Binding::Location {
                            sampling: Some(naga::Sampling::Sample),
                            ..
                        }
                );
                Ok(Value::U32(0))
            })?;
        }
        if per_sample && !self.caps().features.contains(Features::SAMPLE_SHADING) {
            return Err("a program run per sample without SAMPLE_SHADING".into());
        }
        let inside = |q: (i64, i64)| {
            !edges.iter().zip(&biased).any(|(&(a, b), &tl)| {
                let e = edge(a, b, q);
                e < 0 || (e == 0 && !tl)
            })
        };
        // Weights per original vertex at a point.
        let bary_at = |q: (i64, i64)| {
            let mut bary = [0.0; 3];
            for (j, &i) in order.iter().enumerate() {
                let (a, b) = edges[j];
                bary[i] = edge(a, b, q) as f64 / area as f64;
            }
            bary
        };
        let depth_at = |bary: &[f64; 3]| (0..3).map(|i| bary[i] * s[i][2]).sum::<f64>() + bias;
        for py in y0.max(0)..y1 {
            for px in x0.max(0)..x1 {
                let at = |p: (i64, i64)| (px * 256 + p.0, py * 256 + p.1);
                let cover = positions
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| inside(at(**p)))
                    .fold(0u32, |m, (i, _)| m | 1 << i);
                if cover == 0 {
                    continue;
                }
                // Each run of the fragment program: the sample it runs at (the pixel's centre when
                // none) and the samples it covers.
                let runs: Vec<(Option<usize>, u32)> = if per_sample {
                    (0..positions.len())
                        .filter(|i| cover & 1 << i != 0)
                        .map(|i| (Some(i), 1 << i))
                        .collect()
                } else {
                    vec![(None, cover)]
                };
                for (sample, run) in runs {
                    let q = at(sample.map_or((128, 128), |i| positions[i]));
                    let bary = bary_at(q);
                    let inv_w: f64 = (0..3).map(|i| bary[i] * s[i][3]).sum();
                    let z_run = depth_at(&bary);
                    let interp = |values: &dyn Fn(&ClipVertex) -> &Vec<f64>,
                                  perspective: bool,
                                  at: usize|
                     -> f64 {
                        if perspective {
                            (0..3)
                                .map(|i| bary[i] * s[i][3] * values(v[i])[at])
                                .sum::<f64>()
                                / inv_w
                        } else {
                            (0..3).map(|i| bary[i] * values(v[i])[at]).sum()
                        }
                    };
                    // The fragment program.
                    let mut outs: Vec<(u32, Vec<f64>)> = Vec::new();
                    let mut mask = raster.sample_mask & run;
                    let mut frag_depth = None;
                    if let Some(fs) = fragment {
                        let m = &fs.module;
                        let ep = entry(fs)?;
                        let fun = &m.entry_points[ep].function;
                        let args = arguments(m, fun, |binding, ty| match *binding {
                            Binding::BuiltIn(BuiltIn::Position { .. }) => Ok(make_value(
                                ScalarKind::Float,
                                &[q.0 as f64 / 256.0, q.1 as f64 / 256.0, z_run, inv_w],
                            )),
                            Binding::BuiltIn(BuiltIn::FrontFacing) => Ok(Value::Bool(front)),
                            Binding::BuiltIn(BuiltIn::SampleIndex) => {
                                Ok(Value::U32(sample.unwrap_or(0) as u32))
                            }
                            Binding::BuiltIn(BuiltIn::SampleMask) => Ok(Value::U32(run)),
                            Binding::BuiltIn(b) => Err(format!("fragment input {b:?}")),
                            Binding::Location { location, .. } => {
                                let (kind, n) = shape(m, ty)?;
                                let (vary, at) = layout
                                    .entries
                                    .iter()
                                    .find(|(v, _)| v.location == location)
                                    .ok_or_else(|| {
                                        format!("no vertex output at location {location}")
                                    })?;
                                let numbers: Vec<f64> = match vary.interpolation {
                                    Interpolation::Perspective => (0..n)
                                        .map(|c| interp(&|cv| &cv.perspective, true, at + c))
                                        .collect(),
                                    Interpolation::Linear => (0..n)
                                        .map(|c| interp(&|cv| &cv.linear, false, at + c))
                                        .collect(),
                                    _ => flat
                                        .iter()
                                        .find(|f| f.location == location)
                                        .map(|f| f.numbers[..n].to_vec())
                                        .ok_or("a flat value missing")?,
                                };
                                Ok(make_value(kind, &numbers))
                            }
                        })?;
                        let out =
                            program::run(m, &fs.info, ep, args, res, &mut res.writes.borrow_mut())?;
                        let Some(out) = out else {
                            continue;
                        };
                        for (binding, _, value) in outputs(m, fun, out)? {
                            match binding {
                                Binding::Location {
                                    location,
                                    blend_src: None | Some(0),
                                    ..
                                } => outs.push((location, value.numbers()?)),
                                Binding::BuiltIn(BuiltIn::FragDepth) => {
                                    frag_depth = Some(f64::from(value.f32()?))
                                }
                                Binding::BuiltIn(BuiltIn::SampleMask) => {
                                    mask &= value.numbers()?[0] as u32
                                }
                                other => return Err(format!("fragment output {other:?}")),
                            }
                        }
                    }
                    // Each covered sample is tested and written on its own, with its own depth.
                    for (j, &p) in positions.iter().enumerate() {
                        if mask & 1 << j == 0 {
                            continue;
                        }
                        let z = frag_depth
                            .unwrap_or_else(|| depth_at(&bary_at(at(p))))
                            .clamp(vmin.min(vmax), vmax.max(vmin));
                        let j = j as u32;
                        // Depth and stencil.
                        let mut depth_pass = true;
                        let mut stencil_pass = true;
                        if let Some(d) = depth.as_deref_mut() {
                            let format = d.format;
                            let t = d.texel(px as u32, py as u32, j);
                            if let Some(cmp) = ds.depth_compare
                                && !matches!(format, Format::Stencil8)
                            {
                                let stored = texel::decode_depth(format, t);
                                depth_pass = compare(cmp, texel_depth(format, z), stored);
                            }
                            if let (Some((f, b)), Some(at)) = (ds.stencil, stencil_at) {
                                let face = usize::from(!front);
                                let rule = if front { f } else { b };
                                let reference = call.stencil_reference[face];
                                let read = ds.stencil_read_mask[face];
                                let old = t[at];
                                stencil_pass = compare(
                                    rule.compare,
                                    f64::from(reference & read),
                                    f64::from(old & read),
                                );
                                let op = match (stencil_pass, depth_pass) {
                                    (false, _) => rule.fail,
                                    (true, false) => rule.depth_fail,
                                    (true, true) => rule.pass,
                                };
                                let new = stencil_op(op, old, reference);
                                let write = ds.stencil_write_mask[face];
                                if !stencil_read_only {
                                    t[at] = (old & !write) | (new & write);
                                }
                            }
                            if depth_pass
                                && stencil_pass
                                && ds.depth_write
                                && ds.depth_compare.is_some()
                                && !depth_read_only
                            {
                                texel::encode_depth(format, z, t);
                            }
                        }
                        if !(depth_pass && stencil_pass) {
                            continue;
                        }
                        *passed += 1;
                        // Colour.
                        for (i, target) in colors.iter_mut().enumerate() {
                            let Some(target) = target else { continue };
                            let Some((_, out)) = outs.iter().find(|(l, _)| *l as usize == i) else {
                                continue;
                            };
                            let rule = if state.blend.same_for_all {
                                state.blend.targets[0]
                            } else {
                                state.blend.targets[i]
                            };
                            let format = target.format;
                            let t = target.texel(px as u32, py as u32, j);
                            let dst = texel::decode(format, t);
                            let mut src = [0.0, 0.0, 0.0, 1.0];
                            for (c, v) in out.iter().enumerate().take(4) {
                                src[c] = *v;
                            }
                            let mut result = src;
                            if let Some((color, alpha)) = rule.blend {
                                if !self.caps().formats.get(format).contains(FormatUse::BLEND) {
                                    return Err(format!(
                                        "blending into {format:?}, which this back end does not blend"
                                    ));
                                }
                                // Normalised targets clamp what is blended into them, the constant
                                // too.
                                let (src, k) = if texel::normalised(format) {
                                    let low = if signed(format) { -1.0 } else { 0.0 };
                                    (src.map(|x| x.clamp(low, 1.0)), k.map(|x| x.clamp(low, 1.0)))
                                } else {
                                    (src, k)
                                };
                                for c in 0..4 {
                                    let comp = if c == 3 { alpha } else { color };
                                    result[c] = if matches!(comp.op, BlendOp::Min | BlendOp::Max) {
                                        blend_op(comp.op, src[c], dst[c])
                                    } else {
                                        let fs = blend_factor(comp.src, c, src, dst, k)?;
                                        let fd = blend_factor(comp.dst, c, src, dst, k)?;
                                        blend_op(comp.op, src[c] * fs, dst[c] * fd)
                                    };
                                }
                            }
                            for c in 0..4 {
                                if !rule.writes.bits() & (1 << c) != 0 {
                                    result[c] = dst[c];
                                }
                            }
                            texel::encode(format, result, t);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Whether a normalised format is signed.
fn signed(format: Format) -> bool {
    matches!(
        format,
        Format::R8Snorm
            | Format::Rg8Snorm
            | Format::Rgba8Snorm
            | Format::R16Snorm
            | Format::Rg16Snorm
            | Format::Rgba16Snorm
    )
}

/// A fragment's depth as the attachment stores it, so equal compares see stored precision.
fn texel_depth(format: Format, z: f64) -> f64 {
    let mut bytes = [0u8; 8];
    texel::encode_depth(format, z, &mut bytes);
    texel::decode_depth(format, &bytes)
}
