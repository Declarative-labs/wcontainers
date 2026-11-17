//! The rig's eight-viewport scene (crates/wcontainers-graphics-test/src/d3d11/scenes/raster.rs),
//! four of whose viewports start half a pixel down, drawn once per viewport with the first instance
//! as the viewport number in place of its geometry shader; or with the viewports moved to whole
//! pixels (`rounded`). Compared with Direct3D 11's `raster-viewports.png` natively by
//! tests/raster.rs and in Chrome by web/check.

use wcontainers_gpu::{
    Backend, BufferUsage, ColorAttachment, DrawCall, DrawRange, Extent, Format, LoadOp, Origin,
    PassDesc, RasterDesc, ReadSource, ScissorRect, StepMode, StoreOp, TextureRegion, TextureUsage,
    Topology, VertexBinding, VertexFormat, Viewport,
};
use wcontainers_gpu_webgpu::WebGpu;

use crate::{draw, scene};

/// The picture's size.
pub const SIZE: (u32, u32) = (256, 128);

/// raster.hlsl without its geometry shader: the draw's first instance is the viewport number, and
/// the colour is scaled per viewport as the geometry shader scales it.
const VIEWPORTS_WGSL: &str = r#"
struct VOut { @builtin(position) pos: vec4f, @location(0) color: vec4f }
@vertex fn vs(@location(0) pos: vec3f, @location(1) color: vec4f, @builtin(instance_index) v: u32) -> VOut {
    return VOut(vec4f(pos, 1.0), vec4f(color.rgb * (0.3 + 0.1 * f32(v)), 1.0));
}
@fragment fn ps(v: VOut) -> @location(0) vec4f { return v.color; }
"#;

/// The rig's `hue`.
fn hue(i: u32, n: u32) -> [f32; 3] {
    let h = i as f32 / n as f32 * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    match h as u32 {
        0 => [1.0, x, 0.0],
        1 => [x, 1.0, 0.0],
        2 => [0.0, 1.0, x],
        3 => [0.0, x, 1.0],
        4 => [x, 0.0, 1.0],
        _ => [1.0, 0.0, x],
    }
}

/// The rig's `spread`: a fan of three triangles and a white square.
fn spread() -> Vec<f32> {
    let points = [
        [0.0f32, 0.0],
        [0.8, -0.6],
        [0.7, 0.6],
        [-0.3, 0.85],
        [-0.85, -0.2],
    ];
    let mut v: Vec<[f32; 7]> = Vec::new();
    for k in 1..4 {
        for p in [points[0], points[k], points[k + 1]] {
            let [r, g, b] = hue(v.len() as u32, 9);
            v.push([p[0], p[1], 0.5, r, g, b, 1.0]);
        }
    }
    let (x0, y0, x1, y1) = (-0.95f32, -0.95f32, -0.5f32, -0.5f32);
    let white = |x: f32, y: f32| [x, y, 0.5, 1.0, 1.0, 1.0, 1.0];
    v.extend([
        white(x0, y0),
        white(x0, y1),
        white(x1, y1),
        white(x0, y0),
        white(x1, y1),
        white(x1, y0),
    ]);
    v.concat()
}

/// Draws the scene and starts reading it back, rows packed RGBA.
pub fn draw(b: &mut WebGpu, rounded: bool) -> <WebGpu as Backend>::Readback {
    let (w, h) = SIZE;
    let (target, view) = draw::texture_2d(
        b,
        Format::Rgba8Unorm,
        w,
        h,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    )
    .unwrap();
    let program = draw::program(b, VIEWPORTS_WGSL).unwrap();
    let blend = b.create_blend(&draw::opaque()).unwrap();
    let depth_stencil = b.create_depth_stencil(&draw::no_depth()).unwrap();
    let raster = b
        .create_raster(&RasterDesc {
            scissor: true,
            ..draw::solid()
        })
        .unwrap();
    let vertex_layout = b
        .create_vertex_layout(&draw::layout(
            &[(28, StepMode::Vertex)],
            &[
                (0, 0, VertexFormat::Float32x3),
                (0, 12, VertexFormat::Float32x4),
            ],
        ))
        .unwrap();
    let vertices: Vec<u8> = spread().iter().flat_map(|f| f.to_le_bytes()).collect();
    let count = (vertices.len() / 28) as u32;
    let vertices = draw::buffer(b, &vertices, BufferUsage::VERTEX).unwrap();
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear(scene::CLEAR.map(f32::to_bits)),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    for i in 0..8u32 {
        // The rig's `viewports` and `scissors`.
        let shrink = 3.0 * i as f32;
        let (cx, cy) = ((i % 4) as f32 * 64.0, (i / 4) as f32 * 64.0);
        let viewport = [Viewport {
            x: (cx + shrink).to_bits(),
            y: if rounded {
                (cy + shrink / 2.0).floor()
            } else {
                cy + shrink / 2.0
            }
            .to_bits(),
            width: (64.0 - 2.0 * shrink).to_bits(),
            height: (64.0 - shrink).to_bits(),
            min_depth: 0,
            max_depth: 1.0f32.to_bits(),
        }];
        let (x, y) = (64 * (i % 4), 64 * (i / 4));
        let scissor = [ScissorRect {
            x: x + 2 * i,
            y: y + i,
            width: 64 - i - 2 * i,
            height: 60 - 3 * i - i,
        }];
        b.draw(&DrawCall {
            program: &program,
            blend: &blend,
            depth_stencil: &depth_stencil,
            raster: &raster,
            vertex_layout: &vertex_layout,
            bindings: [None; 4],
            vertex_buffers: &[Some(VertexBinding {
                buffer: &vertices,
                offset: 0,
            })],
            index: None,
            topology: Topology::TriangleList,
            restart: false,
            viewports: &viewport,
            scissors: &scissor,
            blend_constant: [0; 4],
            stencil_reference: [0; 2],
            immediates: &[],
            range: DrawRange::Vertices {
                first: 0,
                count,
                first_instance: i,
                instances: 1,
            },
        });
    }
    b.end_pass();
    b.read_back(&ReadSource::Texture {
        region: TextureRegion {
            texture: &target,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        },
        extent: Extent {
            width: w,
            height: h,
            depth_or_layers: 1,
        },
    })
    .unwrap()
}
