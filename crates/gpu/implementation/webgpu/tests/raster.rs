//! Viewports at fractional positions (`FRACTIONAL_VIEWPORT`, plan 4.11) against real Direct3D 11's
//! picture of the rig's eight-viewport scene (crates/graphics-test/src/d3d11/scenes/
//! raster.rs): four of the eight viewports start half a pixel down. Wireframe filling
//! (`POLYGON_MODE`) against the same scenes' fill picture.
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs.

#[path = "../../../../graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

mod allowance;

#[path = "../examples/scenes/viewports.rs"]
mod viewports;

use libtest_mimic::{Completion, Failed, Trial};
use wcontainers_gpu::{
    Gpu, caps::Features, draw::ColorAttachment, draw::DrawCall, draw::DrawRange, draw::LoadOp,
    draw::PassDesc, draw::StoreOp, draw::VertexBinding, draw::Viewport, error::DeviceStatus,
    error::GpuError, format::Format, resource::BufferUsage, resource::Extent, resource::Origin,
    resource::ReadSource, resource::TextureRegion, resource::TextureUsage, state::CullMode,
    state::FillMode, state::RasterDesc, state::StepMode, state::Topology, state::VertexFormat,
};
use wcontainers_gpu_webgpu::WebGpu;

fn main() {
    let args = libtest_mimic::Arguments::from_args();
    let trials = vec![
        Trial::test("fractional_viewports_match_direct3d", || {
            fractional_viewports_match_direct3d();
            Ok(())
        }),
        Trial::ignorable_test(
            "wireframe_fill_matches_direct3d",
            wireframe_fill_matches_direct3d,
        ),
        Trial::ignorable_test("point_fill_draws_the_corners", point_fill_draws_the_corners),
    ];
    libtest_mimic::run(&args, trials).exit();
}

/// Whether the device is wgpu's Vulkan, whose lines follow Vulkan's rasterisation rules, not
/// Direct3D's.
fn vulkan() -> bool {
    std::env::var("WEBGPU_API").as_deref() == Ok("vulkan")
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

fn fractional_viewports_match_direct3d() {
    let mut b = device();
    assert!(b.caps().features.contains(Features::FRACTIONAL_VIEWPORT));
    for rounded in [false, true] {
        let (w, h) = viewports::SIZE;
        let rb = viewports::draw(&mut b, rounded);
        b.submit();
        b.wait_read(&rb);
        let mut got = vec![0; (w * h * 4) as usize];
        b.finish_read(rb, &mut got);
        assert_eq!(b.status(), DeviceStatus::Ok);
        let path = format!(
            "{}/../../../graphics-test/references/d3d11/raster-viewports.png",
            env!("CARGO_MANIFEST_DIR")
        );
        let decoder =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut want = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut want).unwrap();
        want.truncate(info.buffer_size());
        if rounded {
            // The same viewports moved to whole pixels must not match: the half pixel is seen.
            let (tolerance, pixels) = allowance::allowance("raster-viewports.png");
            assert!(
                allowance::beyond(&got, &want, tolerance).0 > pixels,
                "rounded viewports matched too"
            );
        } else {
            assert!(
                allowance::within(&got, &want, "raster-viewports.png"),
                "the viewports differ from Direct3D's"
            );
        }
    }
}

/// The rig's `fan` and `windings`: three triangles of a fan coloured by corner, then a clockwise
/// red triangle and an anticlockwise green one, each vertex position then colour.
fn fan_and_windings() -> Vec<[f32; 7]> {
    let hue = |i: u32, n: u32| {
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
    };
    let points = [
        [0.0f32, 0.0],
        [0.8, -0.6],
        [0.7, 0.6],
        [-0.3, 0.85],
        [-0.85, -0.2],
    ];
    let mut v = Vec::new();
    for k in 1..4 {
        for p in [points[0], points[k], points[k + 1]] {
            let [r, g, b] = hue(v.len() as u32, 9);
            v.push([p[0], p[1], 0.5, r, g, b, 1.0]);
        }
    }
    let red = |x: f32, y: f32| [x, y, 0.5, 0.9, 0.2, 0.2, 1.0];
    let green = |x: f32, y: f32| [x, y, 0.5, 0.2, 0.9, 0.3, 1.0];
    v.extend([
        red(-0.5, 0.8),
        red(-0.1, -0.8),
        red(-0.9, -0.8),
        green(0.5, 0.8),
        green(0.1, -0.8),
        green(0.9, -0.8),
    ]);
    v
}

use draw::COLORED_WGSL;

/// The rig's fill picture: four 64-pixel cells, wireframe without culling, culling back faces and
/// front faces, then solid.
/// A known failure on wgpu's Vulkan, reported as ignored with its reason: Vulkan draws polygon
/// edges by its own line rules. The solid cell is still held to Direct3D's picture there.
fn wireframe_fill_matches_direct3d() -> Result<Completion, Failed> {
    let mut b = device();
    // wgpu fills polygons with lines on every native API; points only on some (not Direct3D 12).
    assert!(b.caps().features.contains(Features::POLYGON_MODE));
    eprintln!(
        "POLYGON_MODE_POINT: {}",
        b.caps().features.contains(Features::POLYGON_MODE_POINT)
    );
    if !b.caps().features.contains(Features::POLYGON_MODE_POINT) {
        let point = RasterDesc {
            fill: FillMode::Point,
            ..draw::solid()
        };
        assert_eq!(
            b.create_raster(&point).unwrap_err(),
            GpuError::Absent(Features::POLYGON_MODE_POINT)
        );
    }
    let fills = [
        (FillMode::Line, CullMode::None),
        (FillMode::Line, CullMode::Back),
        (FillMode::Line, CullMode::Front),
        (FillMode::Solid, CullMode::None),
    ]
    .map(|(fill, cull)| RasterDesc {
        fill,
        cull,
        ..draw::solid()
    });
    let got = cells(&mut b, &fills, &fan_and_windings(), 64);
    assert_eq!(b.status(), DeviceStatus::Ok);
    let path = format!(
        "{}/../../../graphics-test/references/d3d11/raster-fill.png",
        env!("CARGO_MANIFEST_DIR")
    );
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut want = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut want).unwrap();
    want.truncate(info.buffer_size());
    let (differ, largest) = allowance::beyond(&got, &want, 0);
    eprintln!("wireframe fill: {differ} pixels differ from Direct3D's, by up to {largest}");
    if allowance::within(&got, &want, "raster-fill.png") {
        return Ok(Completion::Completed);
    }
    let solid = |p: &[u8]| -> Vec<u8> {
        p.chunks(256 * 4)
            .flat_map(|row| row[192 * 4..].to_vec())
            .collect()
    };
    if vulkan() && allowance::within(&solid(&got), &solid(&want), "raster-fill.png") {
        return Ok(Completion::ignored_with(format!(
            "known failure on Vulkan, whose line rules differ from Direct3D's: {differ} pixels \
             of the wireframe cells differ, by up to {largest}"
        )));
    }
    Err("the wireframe fill differs from Direct3D's".into())
}

/// `vertices` (position, colour) drawn as a triangle list once per raster block, each into its
/// own square cell of a row as high as the picture: the picture, rows packed RGBA.
fn cells(b: &mut WebGpu, fills: &[RasterDesc], vertices: &[[f32; 7]], h: u32) -> Vec<u8> {
    let w = h * fills.len() as u32;
    let (target, view) = draw::texture_2d(
        b,
        Format::Rgba8Unorm,
        w,
        h,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    )
    .unwrap();
    let program = draw::program(b, COLORED_WGSL).unwrap();
    let blend = b.create_blend(&draw::opaque()).unwrap();
    let depth_stencil = b.create_depth_stencil(&draw::no_depth()).unwrap();
    let vertex_layout = b
        .create_vertex_layout(&draw::layout(
            &[(28, StepMode::Vertex)],
            &[
                (0, 0, VertexFormat::Float32x3),
                (0, 12, VertexFormat::Float32x4),
            ],
        ))
        .unwrap();
    let vertices: Vec<u8> = vertices
        .concat()
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let count = (vertices.len() / 28) as u32;
    let vertices = draw::buffer(b, &vertices, BufferUsage::VERTEX).unwrap();
    let rasters: Vec<_> = fills.iter().map(|f| b.create_raster(f).unwrap()).collect();
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
    for (i, raster) in rasters.iter().enumerate() {
        b.draw(&DrawCall {
            program: &program,
            blend: &blend,
            depth_stencil: &depth_stencil,
            raster,
            vertex_layout: &vertex_layout,
            bindings: [None; 4],
            vertex_buffers: &[Some(VertexBinding {
                buffer: &vertices,
                offset: 0,
            })],
            index: None,
            topology: Topology::TriangleList,
            restart: false,
            viewports: &[Viewport {
                x: (h as f32 * i as f32).to_bits(),
                y: 0,
                width: (h as f32).to_bits(),
                height: (h as f32).to_bits(),
                min_depth: 0,
                max_depth: 1.0f32.to_bits(),
            }],
            scissors: &[],
            blend_constant: [0; 4],
            stencil_reference: [0; 2],
            immediates: &[],
            range: DrawRange::Vertices {
                first: 0,
                count,
                first_instance: 0,
                instances: 1,
            },
        });
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
    got
}

/// Point filling draws a point at each corner of a triangle: on a device declaring it, exactly
/// the three pixels holding the corners are drawn. Elsewhere reported ignored, with its reason.
fn point_fill_draws_the_corners() -> Result<Completion, Failed> {
    let mut b = device();
    if !b.caps().features.contains(Features::POLYGON_MODE_POINT) {
        return Ok(Completion::ignored_with(
            "POLYGON_MODE_POINT is absent on this device (its refusal is checked beside the \
             wireframe)",
        ));
    }
    let size = 16;
    let at = |x: u32, y: u32| {
        // The centre of pixel (x, y), in clip space.
        let c = |p: u32| (p as f32 + 0.5) / size as f32 * 2.0 - 1.0;
        [c(x), -c(y), 0.5, 1.0, 0.0, 0.0, 1.0]
    };
    let corners = [(2, 3), (12, 5), (6, 13)];
    let point = RasterDesc {
        fill: FillMode::Point,
        ..draw::solid()
    };
    let got = cells(&mut b, &[point], &corners.map(|(x, y)| at(x, y)), size);
    let drawn: Vec<(u32, u32)> = (0..size * size)
        .filter(|i| got[*i as usize * 4..][..4] == [255, 0, 0, 255])
        .map(|i| (i % size, i / size))
        .collect();
    let mut want = corners.to_vec();
    want.sort_by_key(|&(x, y)| (y, x));
    if drawn != want {
        return Err(format!("point filling drew {drawn:?}, not the corners {want:?}").into());
    }
    Ok(Completion::Completed)
}
