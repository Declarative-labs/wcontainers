//! Writing one texel with `Discard`, and the small full-target draws the bind group tests use.
//! Shared by the crate's unit tests, tests/features.rs, tests/direct3d.rs and the browser page.

use super::draw;
use wcontainers_gpu::{
    Gpu, draw::ColorAttachment, draw::DrawCall, draw::DrawRange, draw::LoadOp, draw::PassDesc,
    draw::StoreOp, draw::Viewport, format::Format, resource::Extent, resource::Origin,
    resource::ReadSource, resource::TextureRegion, resource::TextureUsage, resource::WriteMode,
    state::BlendDesc, state::Topology,
};
use wcontainers_gpu_webgpu::WebGpu;

/// A full-target triangle whose colour is the constant buffer's first vector.
pub const CONSTANT_COLOR_WGSL: &str = r#"
struct C { color: vec4f }
@group(1) @binding(0) var<uniform> c: C;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn ps() -> @location(0) vec4f { return c.color; }
"#;

/// What every draw here shares but its set and viewport.
pub struct Kit {
    pub program: <WebGpu as Gpu>::Program,
    pub blend: <WebGpu as Gpu>::Blend,
    pub depth_stencil: <WebGpu as Gpu>::DepthStencil,
    pub raster: <WebGpu as Gpu>::Raster,
    pub vertex_layout: <WebGpu as Gpu>::VertexLayout,
    pub target: <WebGpu as Gpu>::Texture,
    pub view: <WebGpu as Gpu>::TextureView,
    pub width: u32,
}

pub fn kit(b: &mut WebGpu, format: Format, width: u32, blend: &BlendDesc) -> Kit {
    let (target, view) = draw::texture_2d(
        b,
        format,
        width,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    )
    .unwrap();
    Kit {
        program: draw::program(b, CONSTANT_COLOR_WGSL).unwrap(),
        blend: b.create_blend(blend).unwrap(),
        depth_stencil: b.create_depth_stencil(&draw::no_depth()).unwrap(),
        raster: b.create_raster(&draw::solid()).unwrap(),
        vertex_layout: b.create_vertex_layout(&draw::layout(&[], &[])).unwrap(),
        target,
        view,
        width,
    }
}

pub fn begin(b: &mut WebGpu, k: &Kit, clear: [f32; 4]) {
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &k.view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear(clear.map(f32::to_bits)),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
}

/// Draws the triangle with `set` into pixel `x` alone, or the whole target.
pub fn draw_at(b: &mut WebGpu, k: &Kit, set: &<WebGpu as Gpu>::BindingSet, x: Option<u32>) {
    let (x, width) = x.map_or((0.0, k.width as f32), |x| (x as f32, 1.0));
    let viewport = [Viewport {
        x: x.to_bits(),
        y: 0,
        width: width.to_bits(),
        height: 1.0f32.to_bits(),
        min_depth: 0,
        max_depth: 1.0f32.to_bits(),
    }];
    b.draw(&DrawCall {
        program: &k.program,
        blend: &k.blend,
        depth_stencil: &k.depth_stencil,
        raster: &k.raster,
        vertex_layout: &k.vertex_layout,
        bindings: [None, Some(set), None, None],
        vertex_buffers: &[],
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: &viewport,
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
}

/// Ends the pass, reads the target back and waits for it.
pub fn finish(b: &mut WebGpu, k: &Kit, texel: u32) -> Vec<u8> {
    b.end_pass();
    let rb = b
        .read_back(&ReadSource::Texture {
            region: TextureRegion {
                texture: &k.target,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width: k.width,
                height: 1,
                depth_or_layers: 1,
            },
        })
        .unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut out = vec![0; (k.width * texel) as usize];
    b.finish_read(rb, &mut out);
    out
}

/// A full-target triangle whose red is texel (0, 0) of a texture it only loads from.
pub const LOAD_WGSL: &str = r#"
@group(1) @binding(32) var t: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn ps() -> @location(0) vec4f {
    return vec4f(textureLoad(t, vec2i(0), 0).r, 0.0, 0.0, 1.0);
}
"#;

/// Writes one texel of row 0 with `Discard`.
pub fn one_texel(b: &mut WebGpu, texture: &<WebGpu as Gpu>::Texture, x: u32, rgba: [u8; 4]) {
    b.write_texture(
        &TextureRegion {
            texture,
            mip: 0,
            layer: 0,
            origin: Origin { x, y: 0, z: 0 },
            stencil: false,
        },
        Extent {
            width: 1,
            height: 1,
            depth_or_layers: 1,
        },
        &rgba,
        4,
        1,
        WriteMode::Discard,
    );
}
