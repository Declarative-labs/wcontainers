//! How 24-bit depth is stored (the probe of plan 4.11), and a constant, slope-scaled and clamped
//! depth bias on `depth24plus-stencil8` against real Direct3D 11's picture of the rig's depth-bias
//! scene.
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs. Its own harness reports the bias picture as
//! ignored, not passed, where the device lacks `DEPTH24_UNORM`.

#[path = "../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

use libtest_mimic::{Completion, Failed, Trial};
use wcontainers_gpu::{
    AspectOps, Backend, BufferUsage, ColorAttachment, CompareFn, DepthAttachment, DepthStencilDesc,
    DeviceStatus, DrawCall, DrawRange, Extent, Features, Format, LoadOp, Origin, PassDesc,
    RasterDesc, ReadSource, StepMode, StoreOp, TextureRegion, TextureUsage, Topology,
    VertexBinding, VertexFormat, Viewport,
};
use wcontainers_gpu_webgpu::{Depth24, WebGpu};

fn main() {
    let args = libtest_mimic::Arguments::from_args();
    let trials = vec![
        Trial::test("the_depth24_probe_finds_how_depth24plus_is_stored", || {
            the_depth24_probe_finds_how_depth24plus_is_stored();
            Ok(())
        }),
        Trial::ignorable_test(
            "depth_bias_on_24_bit_depth_matches_direct3d",
            depth_bias_on_24_bit_depth_matches_direct3d,
        ),
    ];
    libtest_mimic::run(&args, trials).exit();
}

fn device() -> WebGpu {
    let backends = match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    };
    let (b, adapter) = pollster::block_on(draw::device(backends)).expect("a GPU adapter");
    eprintln!("adapter: {adapter}");
    b
}

use draw::COLORED_WGSL;

/// The rig's `quad`: two triangles from (x0, y0) to (x1, y1), depth z0 on the left edge and z1 on
/// the right.
#[allow(clippy::too_many_arguments)]
fn quad(x0: f32, y0: f32, x1: f32, y1: f32, z0: f32, z1: f32, color: [f32; 4]) -> Vec<f32> {
    let v = |x: f32, y: f32| {
        let z = if x == x0 { z0 } else { z1 };
        [x, y, z, color[0], color[1], color[2], color[3]]
    };
    [
        v(x0, y0),
        v(x0, y1),
        v(x1, y1),
        v(x0, y0),
        v(x1, y1),
        v(x1, y0),
    ]
    .concat()
}

fn reference(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../wcontainers-graphics-test/references/d3d11/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    buf
}

/// The probe's bias moved the depth up, and the device declares `DEPTH24_UNORM` exactly when it moved
/// it by one 24-bit step, Direct3D's rule. Another amount is not refused: the bit stays clear and
/// fronts bias in the program (wgpu's Vulkan on the RTX 3080 moves it by two steps).
fn the_depth24_probe_finds_how_depth24plus_is_stored() {
    let b = device();
    let found = b.depth24();
    eprintln!("depth24plus: {found:?}");
    let (Depth24::Unorm(v) | Depth24::Float(v) | Depth24::Other(v)) = found;
    assert!(v > 0.1 && v < 0.1001, "{found:?}");
    assert_eq!(
        b.caps().features.contains(Features::DEPTH24_UNORM),
        matches!(found, Depth24::Unorm(_))
    );
}

/// The rig's depth-bias scene (crates/wcontainers-graphics-test/src/d3d11/scenes/depth.rs): a
/// sloped blue quad, then a yellow one over it with each bias (none; constant -3; slope -1.5;
/// constant -3000, slope -1.5, clamp -0.00002) into `depth24plus-stencil8`. Where `DEPTH24_UNORM` is
/// absent a front biases in the program instead, which is not this back end's route to test, so the
/// test is ignored there.
fn depth_bias_on_24_bit_depth_matches_direct3d() -> Result<Completion, Failed> {
    let mut b = device();
    if !b.caps().features.contains(Features::DEPTH24_UNORM) {
        return Ok(Completion::ignored_with(
            "DEPTH24_UNORM is absent here: fronts bias in the program",
        ));
    }
    let (w, h) = (256, 64);
    let (target, target_view) = draw::texture_2d(
        &mut b,
        Format::Rgba8Unorm,
        w,
        h,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    )
    .unwrap();
    let (_depth, depth_view) = draw::texture_2d(
        &mut b,
        Format::Depth24PlusStencil8,
        w,
        h,
        TextureUsage::RENDER,
    )
    .unwrap();
    let program = draw::program(&mut b, COLORED_WGSL).unwrap();
    let blend = b.create_blend(&draw::opaque()).unwrap();
    let depth_stencil = b
        .create_depth_stencil(&DepthStencilDesc {
            depth_compare: Some(CompareFn::Less),
            depth_write: true,
            ..draw::no_depth()
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
    let blue = quad(-0.8, -0.8, 0.8, 0.8, 0.2, 0.8, [0.2, 0.4, 0.95, 1.0]);
    let yellow = quad(-0.8, -0.8, 0.8, 0.8, 0.2, 0.8, [0.95, 0.85, 0.2, 1.0]);
    let bytes = |v: &[f32]| v.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>();
    let blue = draw::buffer(&mut b, &bytes(&blue), BufferUsage::VERTEX).unwrap();
    let yellow = draw::buffer(&mut b, &bytes(&yellow), BufferUsage::VERTEX).unwrap();
    let biases: [(f32, f32, f32); 4] = [
        (0.0, 0.0, 0.0),
        (-3.0, 0.0, 0.0),
        (0.0, -1.5, 0.0),
        (-3000.0, -1.5, -0.00002),
    ];
    let rasters: Vec<_> = biases
        .iter()
        .map(|(bias, slope, clamp)| {
            b.create_raster(&RasterDesc {
                depth_bias: bias.to_bits(),
                depth_bias_slope: slope.to_bits(),
                depth_bias_clamp: clamp.to_bits(),
                ..draw::solid()
            })
            .unwrap()
        })
        .collect();
    let plain = b.create_raster(&draw::solid()).unwrap();
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: Some(DepthAttachment {
            view: &depth_view,
            depth: Some(AspectOps {
                load: LoadOp::Clear(1.0f32.to_bits()),
                store: StoreOp::Store,
                read_only: false,
            }),
            stencil: Some(AspectOps {
                load: LoadOp::Clear(0),
                store: StoreOp::Store,
                read_only: false,
            }),
        }),
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &target_view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear(scene::CLEAR.map(f32::to_bits)),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    for (i, raster) in rasters.iter().enumerate() {
        let viewport = [Viewport {
            x: (i as f32 * 64.0).to_bits(),
            y: 0,
            width: 64.0f32.to_bits(),
            height: 64.0f32.to_bits(),
            min_depth: 0,
            max_depth: 1.0f32.to_bits(),
        }];
        for (vertices, raster) in [(&blue, &plain), (&yellow, raster)] {
            b.draw(&DrawCall {
                program: &program,
                blend: &blend,
                depth_stencil: &depth_stencil,
                raster,
                vertex_layout: &vertex_layout,
                bindings: [None; 4],
                vertex_buffers: &[Some(VertexBinding {
                    buffer: vertices,
                    offset: 0,
                })],
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
                    count: 6,
                    first_instance: 0,
                    instances: 1,
                },
            });
        }
    }
    b.end_pass();
    let rb = b
        .read_back(&ReadSource::Texture {
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
        .unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut got = vec![0; (w * h * 4) as usize];
    b.finish_read(rb, &mut got);
    assert_eq!(b.status(), DeviceStatus::Ok);
    let want = reference("depth-bias.png");
    // The rig's tolerance for this picture is 0 (references/d3d11/tolerances.txt).
    let wrong: Vec<_> = (0..(w * h) as usize)
        .filter(|p| got[p * 4..p * 4 + 4] != want[p * 4..p * 4 + 4])
        .map(|p| (p as u32 % w, p as u32 / w))
        .collect();
    assert!(
        wrong.is_empty(),
        "{} pixels differ, first {:?}",
        wrong.len(),
        wrong.first()
    );
    Ok(Completion::Completed)
}
