//! The rig's Direct3D 12 triangle scene through the device on the WebGPU back end, on a real GPU,
//! compared with real Direct3D 12's picture at the rig's tolerance. Needs a GPU: run it through
//! `tools/graphics-rig/gpu-job`. `WEBGPU_API` picks wgpu's API (dx12, vulkan, metal).

#[path = "support/triangle.rs"]
mod triangle;

use wcontainers_direct3d_12::Device;
use wcontainers_direct3d_12::state::{
    CULL_MODE_NONE, FILL_MODE_SOLID, Raster, TOPOLOGY_TYPE_TRIANGLE,
};
use wcontainers_directx_graphics_infrastructure::dxgi;
use wcontainers_gpu::{Backend, DeviceStatus, Features, SlowPath};
use wcontainers_gpu_webgpu::WebGpu;

/// The rig's shaders passing a three-component position and a colour.
const COLOR_VS: &[u8] =
    include_bytes!("../../wcontainers-graphics-test/shaders/dxil/common.vs_color.dxil");
const COLOR_PS: &[u8] =
    include_bytes!("../../wcontainers-graphics-test/shaders/dxil/common.ps_color.dxil");

fn webgpu() -> WebGpu {
    webgpu_without(wgpu::Features::empty())
}

/// The back end on a device made without `hidden`, as a browser lacking those features gives.
fn webgpu_without(hidden: wgpu::Features) -> WebGpu {
    let backends = match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    };
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends;
    desc.flags |= WebGpu::INSTANCE_FLAGS;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("a GPU adapter");
    let info = adapter.get_info();
    assert_ne!(
        info.device_type,
        wgpu::DeviceType::Cpu,
        "{} is not a GPU",
        info.name
    );
    eprintln!("adapter: {} ({:?})", info.name, info.backend);
    let mut desc = WebGpu::device_descriptor(&adapter);
    desc.required_features.remove(hidden);
    let (device, queue) = pollster::block_on(adapter.request_device(&desc)).unwrap();
    WebGpu::new(&adapter, device, queue)
}

/// The rig's allowance for `name` among the Direct3D 12 references: the largest channel difference
/// and how many pixels may exceed it.
fn tolerance(name: &str) -> (u8, usize) {
    let text = include_str!("../../wcontainers-graphics-test/references/d3d12/tolerances.txt");
    let line = text
        .lines()
        .find(|l| l.split_whitespace().next() == Some(name))
        .expect("a tolerance");
    let mut words = line.split_whitespace().skip(1);
    let mut next = || words.next().unwrap().parse().unwrap();
    (next() as u8, next())
}

fn reference(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../wcontainers-graphics-test/references/d3d12/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(
        (info.color_type, info.bit_depth),
        (png::ColorType::Rgba, png::BitDepth::Eight),
        "{path}"
    );
    buf.truncate(info.buffer_size());
    buf
}

/// The picture `scene` draws on `device`.
fn picture(device: &mut Device<WebGpu>, scene: &triangle::Scene) -> Vec<u8> {
    let queue = device.create_queue().unwrap();
    let fence = device.create_fence(0);
    if let Err(e) = device.execute(queue, &scene.lists) {
        let status = device.backend().status();
        let reason = device.removed();
        panic!(
            "{e:?} ({reason:?}): {status:?}, {:?}",
            device.take_warnings()
        );
    }
    device.queue_signal(queue, fence, 1).unwrap();
    assert!(device.wait(fence, 1));
    assert_eq!(device.backend().status(), DeviceStatus::Ok);
    let mapped = device.map(scene.readback).unwrap();
    // The readback heap's bytes, finished into its memory when the fence was reached.
    let bytes = unsafe { std::slice::from_raw_parts(mapped, scene.total as usize) };
    let f = scene.footprint;
    let row = (f.width * 4) as usize;
    (0..f.height as usize)
        .flat_map(|y| &bytes[y * f.row_pitch as usize..][..row])
        .copied()
        .collect()
}

#[test]
fn the_triangle_scene_matches_direct3d_12() {
    let mut device = Device::new(webgpu());
    let scene = triangle::record(&mut device);
    let picture = picture(&mut device, &scene);
    let expected = reference("triangle.png");
    let (allowed, beyond) = tolerance("triangle.png");
    assert_eq!(picture.len(), expected.len(), "the picture's size");
    let diffs: Vec<u8> = picture
        .iter()
        .zip(&expected)
        .map(|(a, b)| a.abs_diff(*b))
        .collect();
    let largest = diffs.iter().copied().max().unwrap_or(0);
    let pixels = diffs
        .chunks(4)
        .filter(|p| p.iter().any(|d| *d > allowed))
        .count();
    eprintln!("largest channel difference {largest}, pixels beyond {allowed}: {pixels}");
    assert!(
        pixels <= beyond,
        "{pixels} pixels differ by more than {allowed}"
    );
    eprintln!("warnings: {:?}", device.take_warnings());
}

/// Depth clamped in the vertex program where the back end lacks its own (`DepthClampInShader`,
/// with no depth target) draws what the back end's clamp draws: a triangle beyond the far plane
/// and one before the near plane, which clipping would drop.
#[test]
fn depth_clamped_in_the_programs_draws_as_the_back_ends_clamp() {
    // Position, then colour.
    let triangles: [[f32; 7]; 6] = [
        [-0.9, -0.8, 1.5, 1.0, 0.0, 0.0, 1.0],
        [-0.1, -0.8, 1.5, 1.0, 0.0, 0.0, 1.0],
        [-0.5, 0.8, 1.5, 1.0, 0.0, 0.0, 1.0],
        [0.1, -0.8, -0.5, 0.0, 1.0, 0.0, 1.0],
        [0.9, -0.8, -0.5, 0.0, 1.0, 0.0, 1.0],
        [0.5, 0.8, -0.5, 0.0, 1.0, 0.0, 1.0],
    ];
    let data: Vec<u8> = triangles
        .iter()
        .flatten()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let float3 = size_of::<[f32; 3]>() as u32;
    let stride = size_of::<[f32; 7]>() as u32;
    let pictures = [wgpu::Features::empty(), wgpu::Features::DEPTH_CLIP_CONTROL].map(|hidden| {
        let mut device = Device::new(webgpu_without(hidden));
        assert_eq!(
            device.backend().caps().has(Features::DEPTH_CLAMP),
            hidden.is_empty(),
            "the adapter's own depth clamping"
        );
        let inputs = vec![
            triangle::element("POSITION", dxgi::R32G32B32_FLOAT, 0),
            triangle::element("COLOR", dxgi::R32G32B32A32_FLOAT, float3),
        ];
        let raster = Raster {
            fill: FILL_MODE_SOLID,
            cull: CULL_MODE_NONE,
            depth_clip: false,
            ..Default::default()
        };
        let pso = triangle::pipeline_with(
            &mut device,
            COLOR_VS,
            COLOR_PS,
            inputs,
            raster,
            TOPOLOGY_TYPE_TRIANGLE,
            dxgi::UNKNOWN,
        )
        .unwrap();
        let scene = triangle::record_with(&mut device, pso, &data, stride, data.len() as u64);
        let picture = picture(&mut device, &scene);
        let in_programs = device
            .backend()
            .slow_paths()
            .count(SlowPath::DepthClampInShader);
        assert_eq!(in_programs > 0, !hidden.is_empty(), "the slow path's count");
        eprintln!("warnings: {:?}", device.take_warnings());
        picture
    });
    // Both triangles are drawn: their centres are not the clear colour.
    let size = triangle::SIZE as usize;
    let clear = triangle::CLEAR.map(|c| (c * f32::from(u8::MAX)).round() as u8);
    for x in [size / 4, size * 3 / 4] {
        let at = (size / 2 * size + x) * clear.len();
        assert_ne!(
            pictures[0][at..at + clear.len()],
            clear,
            "the triangle at {x}"
        );
    }
    let differing = pictures[0]
        .chunks(clear.len())
        .zip(pictures[1].chunks(clear.len()))
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(differing, 0, "pixels differing between the two clamps");
}

/// HLSL compiled with the system's compiler (`D3DCompile`) for shader model `target`.
#[cfg(windows)]
fn compiled(source: &str, target: windows::core::PCSTR) -> Vec<u8> {
    use windows::Win32::Graphics::Direct3D::{Fxc::D3DCompile, ID3DInclude};
    let mut code = None;
    let entry = std::ffi::CString::new(wcontainers_shaders::ENTRY_POINT).unwrap();
    // The source and entry outlive the call; the blob is copied out before it is released.
    unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            None,
            None,
            None::<&ID3DInclude>,
            windows::core::PCSTR(entry.as_ptr().cast()),
            target,
            0,
            0,
            &mut code,
            None,
        )
        .unwrap();
        let code = code.unwrap();
        std::slice::from_raw_parts(code.GetBufferPointer().cast::<u8>(), code.GetBufferSize())
            .to_vec()
    }
}

/// HLSL of the depth clamp scene's routes: a vertex shader passing position and colour, a pixel
/// shader writing the colour, a geometry shader passing each triangle on, vertex shaders writing a
/// cull distance that culls nothing, a clip distance cutting off the left, or both, and hull and
/// domain shaders passing each triangle on once.
#[cfg(windows)]
mod clamp_routes {
    pub const VS: &str = "struct O { float4 p : SV_Position; float4 c : COLOR; };
         O main(float4 p : POSITION, float4 c : COLOR) { O o; o.p = p; o.c = c; return o; }";
    pub const PS: &str =
        "float4 main(float4 p : SV_Position, float4 c : COLOR) : SV_Target { return c; }";
    pub const GS: &str = "struct V { float4 p : SV_Position; float4 c : COLOR; };
         [maxvertexcount(3)] void main(triangle V i[3], inout TriangleStream<V> s) {
             for (uint k = 0; k < 3; k++) { s.Append(i[k]); } }";
    pub const CULLED_VS: &str =
        "struct O { float4 p : SV_Position; float4 c : COLOR; float d : SV_CullDistance; };
         O main(float4 p : POSITION, float4 c : COLOR) { O o; o.p = p; o.c = c; o.d = 1.0; return o; }";
    pub const CLIPPED_VS: &str =
        "struct O { float4 p : SV_Position; float4 c : COLOR; float d : SV_ClipDistance; };
         O main(float4 p : POSITION, float4 c : COLOR) {
             O o; o.p = p; o.c = c; o.d = p.x + 0.25 * p.w; return o; }";
    pub const CLIPPED_CULLED_VS: &str = "struct O { float4 p : SV_Position; float4 c : COLOR;
             float d : SV_ClipDistance; float e : SV_CullDistance; };
         O main(float4 p : POSITION, float4 c : COLOR) {
             O o; o.p = p; o.c = c; o.d = p.x + 0.25 * p.w; o.e = 1.0; return o; }";
    pub const HS: &str = "struct V { float4 p : SV_Position; float4 c : COLOR; };
         struct F { float e[3] : SV_TessFactor; float i : SV_InsideTessFactor; };
         F constants() { F f; f.e[0] = 1.0; f.e[1] = 1.0; f.e[2] = 1.0; f.i = 1.0; return f; }
         [domain(\"tri\")] [partitioning(\"integer\")] [outputtopology(\"triangle_cw\")]
         [outputcontrolpoints(3)] [patchconstantfunc(\"constants\")]
         V main(InputPatch<V, 3> p, uint i : SV_OutputControlPointID) { return p[i]; }";
    pub const DS: &str = "struct V { float4 p : SV_Position; float4 c : COLOR; };
         struct F { float e[3] : SV_TessFactor; float i : SV_InsideTessFactor; };
         [domain(\"tri\")] V main(F f, float3 b : SV_DomainLocation, const OutputPatch<V, 3> p) {
             V o; o.p = p[0].p * b.x + p[1].p * b.y + p[2].p * b.z; o.c = p[0].c; return o; }";
}

/// The shaders of one route the depth clamp scene's triangle is drawn through (empty where none).
#[cfg(windows)]
struct ClampRoute {
    name: &'static str,
    vs: Vec<u8>,
    hs: Vec<u8>,
    ds: Vec<u8>,
    gs: Vec<u8>,
    ps: Vec<u8>,
    /// Whether its vertex shader writes a clip distance.
    clipped: bool,
}

/// Depth clamped in the programs into a depth target (`DepthClampInShader`'s pixel half), with
/// perspective (w from 1 to 4) and two viewport depth ranges, draws what the back end's own clamp
/// draws: a triangle reaching past the far plane and before the near one, drawn directly, without a
/// pixel shader (the pixel program for its variant writes the depth), through a geometry shader,
/// through the cull route and through tessellation, then a quad at depth 0.9 tested against it with
/// `LESS_EQUAL`; with a clip distance, clipped in the programs where the back end's clip distances
/// are hidden too (`ClipDistanceInShader`), without a pixel shader and through the cull route.
#[cfg(windows)]
#[test]
fn depth_clamped_in_the_programs_into_a_depth_target_draws_as_the_back_ends_clamp() {
    use windows::core::s;
    let vs = compiled(clamp_routes::VS, s!("vs_5_0"));
    let ps = compiled(clamp_routes::PS, s!("ps_5_0"));
    let route =
        |name: &'static str, vs: &[u8], hs: &[u8], ds: &[u8], gs: &[u8], ps: &[u8]| ClampRoute {
            name,
            vs: vs.to_vec(),
            hs: hs.to_vec(),
            ds: ds.to_vec(),
            gs: gs.to_vec(),
            ps: ps.to_vec(),
            clipped: name.starts_with("clipped"),
        };
    let routes = [
        route("direct", &vs, &[], &[], &[], &ps),
        route("without a pixel shader", &vs, &[], &[], &[], &[]),
        route(
            "geometry shader",
            &vs,
            &[],
            &[],
            &compiled(clamp_routes::GS, s!("gs_5_0")),
            &ps,
        ),
        route(
            "cull route",
            &compiled(clamp_routes::CULLED_VS, s!("vs_5_0")),
            &[],
            &[],
            &[],
            &ps,
        ),
        route(
            "clipped without a pixel shader",
            &compiled(clamp_routes::CLIPPED_VS, s!("vs_5_0")),
            &[],
            &[],
            &[],
            &[],
        ),
        route(
            "clipped cull route",
            &compiled(clamp_routes::CLIPPED_CULLED_VS, s!("vs_5_0")),
            &[],
            &[],
            &[],
            &ps,
        ),
        route(
            "tessellation",
            &vs,
            &compiled(clamp_routes::HS, s!("hs_5_0")),
            &compiled(clamp_routes::DS, s!("ds_5_0")),
            &[],
            &ps,
        ),
    ];
    let size = triangle::SIZE;
    for route in &routes {
        for range in [[0.0f32, 1.0], [0.25, 0.75]] {
            let hidden = wgpu::Features::DEPTH_CLIP_CONTROL | wgpu::Features::CLIP_DISTANCES;
            let pictures = [wgpu::Features::empty(), hidden]
                .map(|hidden| clamp_scene(route, &vs, &ps, range, hidden));
            let count = |p: &[u8], c: [u8; 4]| p.chunks(c.len()).filter(|x| *x == c).count();
            let red = count(&pictures[0], [u8::MAX, 0, 0, u8::MAX]);
            let green = count(&pictures[0], [0, u8::MAX, 0, u8::MAX]);
            let differing = pictures[0]
                .chunks(size_of::<u32>())
                .zip(pictures[1].chunks(size_of::<u32>()))
                .filter(|(a, b)| a != b)
                .count();
            eprintln!(
                "{} range {range:?}: red {red}, green {green}, differing {differing}",
                route.name
            );
            // The triangle's clamped parts in front of the quad (red, or the clear colour without
            // a pixel shader) and the quad over the rest both show.
            let pixels = (size * size) as usize;
            assert!(green > 0 && green < pixels, "{}: green {green}", route.name);
            assert_eq!(red > 0, !route.ps.is_empty(), "{}: red {red}", route.name);
            assert_eq!(
                differing, 0,
                "{}: pixels differing between the two clamps",
                route.name
            );
        }
    }
}

/// The depth clamp scene's picture: `route` draws the triangle with depth clamped, then the
/// pipeline of `vs` and `ps` draws the quad, on a device without `hidden`, the viewport's depth
/// range `range`.
#[cfg(windows)]
fn clamp_scene(
    route: &ClampRoute,
    vs: &[u8],
    ps: &[u8],
    range: [f32; 2],
    hidden: wgpu::Features,
) -> Vec<u8> {
    use wcontainers_direct3d_12::GraphicsPipelineDesc;
    use wcontainers_direct3d_12::list::*;
    use wcontainers_direct3d_12::resource::*;
    use wcontainers_direct3d_12::root::{
        FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT, RootSignatureDesc, VERSION_1_0, serialize,
    };
    use wcontainers_direct3d_12::state::*;
    use wcontainers_gpu::ColorWrites;
    use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_3_CONTROL_POINT_PATCHLIST;

    // A vertex at (x, y, z) / w, then its colour.
    let vertex = |x: f32, y: f32, z: f32, w: f32, c: [f32; 4]| {
        [x * w, y * w, z * w, w, c[0], c[1], c[2], c[3]]
    };
    let red = [1.0, 0.0, 0.0, 1.0];
    let green = [0.0, 1.0, 0.0, 1.0];
    let triangle_vertices = [
        vertex(-0.95, -0.95, 0.2, 1.0, red),
        vertex(0.95, -0.95, 1.5, 4.0, red),
        vertex(0.0, 0.95, -0.4, 2.0, red),
    ];
    let quad_vertices = [
        vertex(-1.0, -1.0, 0.9, 1.0, green),
        vertex(1.0, -1.0, 0.9, 1.0, green),
        vertex(-1.0, 1.0, 0.9, 1.0, green),
        vertex(1.0, -1.0, 0.9, 1.0, green),
        vertex(1.0, 1.0, 0.9, 1.0, green),
        vertex(-1.0, 1.0, 0.9, 1.0, green),
    ];
    let data: Vec<u8> = triangle_vertices
        .iter()
        .chain(&quad_vertices)
        .flatten()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let stride = size_of::<[f32; 8]>() as u32;
    let size = triangle::SIZE;
    let mut device = Device::new(webgpu_without(hidden));
    let root = device
        .create_root_signature(&serialize(&RootSignatureDesc {
            version: VERSION_1_0,
            parameters: vec![],
            samplers: vec![],
            flags: FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        }))
        .unwrap();
    let mut blend = Blend::default();
    blend.targets[0].write_mask = ColorWrites::all().bits();
    let tessellated = !route.hs.is_empty();
    let pipeline =
        |device: &mut Device<WebGpu>, vs: &[u8], hs: &[u8], ds: &[u8], gs: &[u8], ps: &[u8]| {
            device.create_graphics_pipeline(&GraphicsPipelineDesc {
                root: Some(root.clone()),
                vs,
                hs,
                ds,
                gs,
                ps,
                blend,
                sample_mask: u32::MAX,
                raster: Raster {
                    fill: FILL_MODE_SOLID,
                    cull: CULL_MODE_NONE,
                    depth_clip: false,
                    ..Default::default()
                },
                depth_stencil: DepthStencil {
                    depth_enable: true,
                    depth_write_all: true,
                    depth_func: COMPARISON_LESS_EQUAL,
                    ..Default::default()
                },
                inputs: vec![
                    triangle::element("POSITION", dxgi::R32G32B32A32_FLOAT, 0),
                    triangle::element(
                        "COLOR",
                        dxgi::R32G32B32A32_FLOAT,
                        size_of::<[f32; 4]>() as u32,
                    ),
                ],
                topology_type: match hs.is_empty() {
                    true => TOPOLOGY_TYPE_TRIANGLE,
                    false => TOPOLOGY_TYPE_PATCH,
                },
                dsv_format: dxgi::D32_FLOAT,
                rtv_formats: vec![triangle::RGBA8],
                samples: 1,
                ..Default::default()
            })
        };
    let routed = pipeline(
        &mut device,
        &route.vs,
        &route.hs,
        &route.ds,
        &route.gs,
        &route.ps,
    )
    .unwrap_or_else(|e| panic!("{}: {e:?}", route.name));
    let quad = pipeline(&mut device, vs, &[], &[], &[], ps).unwrap();
    let texture = |format, flags| ResourceDesc {
        dimension: DIMENSION_TEXTURE2D,
        width: size.into(),
        height: size,
        depth_or_array_size: 1,
        mip_levels: 1,
        format,
        sample_count: 1,
        flags,
        ..Default::default()
    };
    let color_desc = texture(triangle::RGBA8, FLAG_RENDER_TARGET);
    let target = device
        .create_committed(HeapType::Default, &color_desc)
        .unwrap();
    let depth = device
        .create_committed(
            HeapType::Default,
            &texture(dxgi::D32_FLOAT, FLAG_DEPTH_STENCIL),
        )
        .unwrap();
    let vertex_buffer = device
        .create_committed(HeapType::Upload, &triangle::buffer(data.len() as u64))
        .unwrap();
    let address = device.address(vertex_buffer);
    let mapped = device.map(vertex_buffer).unwrap();
    // The upload heap's memory stays where it is for the resource's life.
    unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), mapped, data.len()) };
    let view = |resource, format| TargetView {
        resource,
        format,
        mip: 0,
        first_layer: 0,
        layers: 1,
    };
    let rtv = Descriptor::RenderTarget(view(target, triangle::RGBA8));
    let dsv = Descriptor::DepthStencil {
        view: view(depth, dxgi::D32_FLOAT),
        read_only: 0,
    };
    let mut list = CommandList::new();
    list.record(Command::RootSignature {
        compute: false,
        root: Some(root.clone()),
    });
    list.record(Command::Pipeline(routed));
    list.record(Command::ClearTarget {
        view: rtv,
        color: triangle::CLEAR,
        rects: vec![],
    });
    list.record(Command::ClearDepth {
        view: dsv,
        depth: Some(1.0),
        stencil: None,
        rects: vec![],
    });
    list.record(Command::Targets {
        colors: vec![rtv],
        depth: Some(dsv),
    });
    list.record(Command::Viewports(vec![ViewportF {
        width: size as f32,
        height: size as f32,
        min_depth: range[0],
        max_depth: range[1],
        ..Default::default()
    }]));
    list.record(Command::Scissors(vec![[0, 0, size as i32, size as i32]]));
    list.record(Command::Topology(match tessellated {
        true => D3D_PRIMITIVE_TOPOLOGY_3_CONTROL_POINT_PATCHLIST.0 as u32,
        false => TOPOLOGY_TRIANGLELIST,
    }));
    list.record(Command::VertexBuffers {
        start: 0,
        views: vec![VertexBufferView {
            address,
            size: data.len() as u32,
            stride,
        }],
    });
    list.record(Command::Draw {
        vertices: triangle_vertices.len() as u32,
        instances: 1,
        first_vertex: 0,
        first_instance: 0,
    });
    list.record(Command::Pipeline(quad));
    list.record(Command::Topology(TOPOLOGY_TRIANGLELIST));
    list.record(Command::Draw {
        vertices: quad_vertices.len() as u32,
        instances: 1,
        first_vertex: triangle_vertices.len() as u32,
        first_instance: 0,
    });
    list.close().unwrap();
    let (f, total) = color_desc.footprints(0, 1, 0).unwrap();
    let readback = device
        .create_committed(HeapType::Readback, &triangle::buffer(total))
        .unwrap();
    let mut copy = CommandList::new();
    copy.record(Command::CopyTexture {
        dst: CopyLocation::Footprint {
            resource: readback,
            footprint: f[0].footprint,
        },
        at: [0; 3],
        src: CopyLocation::Subresource {
            resource: target,
            index: 0,
        },
        area: None,
    });
    copy.close().unwrap();
    let scene = triangle::Scene {
        lists: vec![
            list.commands().unwrap().clone(),
            copy.commands().unwrap().clone(),
        ],
        readback,
        footprint: f[0].footprint,
        total,
    };
    let picture = picture(&mut device, &scene);
    let in_programs = device
        .backend()
        .slow_paths()
        .count(SlowPath::DepthClampInShader);
    assert_eq!(
        in_programs > 0,
        !hidden.is_empty(),
        "{}: the slow path's count",
        route.name
    );
    let clipped = device
        .backend()
        .slow_paths()
        .count(SlowPath::ClipDistanceInShader);
    assert_eq!(
        clipped > 0,
        route.clipped && !hidden.is_empty(),
        "{}: the clip distances' slow path count",
        route.name
    );
    eprintln!("{}: warnings: {:?}", route.name, device.take_warnings());
    picture
}
