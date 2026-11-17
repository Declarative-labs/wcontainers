//! The front on the recording back end: the rig's first three Direct3D 11 scenes made as the scene
//! programs make them (their DXBC from the rig's corpus), checked by the calls recorded; and the
//! creation checks, lifetimes and maps.

use std::rc::Rc;

use wcontainers_direct3d_11::desc::*;
use wcontainers_direct3d_11::object::{Buffer, Resource, ShaderView, Texture, View};
use wcontainers_direct3d_11::record::Recorder;
use wcontainers_direct3d_11::{Device, Error, PIXEL, Removal, VERTEX, shader};
use wcontainers_gpu::{Backend, LossReason, SlowPath, SourceStage};

type Dev = Device<Recorder>;

fn corpus(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../wcontainers-graphics-test/shaders/dxbc/{name}.dxbc",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn device() -> Dev {
    Device::new(Recorder::new(None), &[level::L11_1, level::L11_0]).unwrap()
}

fn log_from(dev: &Dev, from: usize) -> Vec<String> {
    dev.backend.log[from..].to_vec()
}

fn find(log: &[String], prefix: &str) -> usize {
    log.iter()
        .position(|l| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("no call starting {prefix:?} in {log:#?}"))
}

fn tex2d(width: u32, height: u32, format: u32, bind: u32) -> TextureDesc {
    TextureDesc {
        width,
        height,
        depth: 1,
        mip_levels: 1,
        array_size: 1,
        format,
        sample_count: 1,
        usage: usage::DEFAULT,
        bind_flags: bind,
        ..Default::default()
    }
}

fn immutable(dev: &mut Dev, bytes: &[u8], bind_flags: u32) -> Rc<Buffer<Recorder>> {
    let desc = BufferDesc {
        byte_width: bytes.len() as u32,
        usage: usage::IMMUTABLE,
        bind_flags,
        ..Default::default()
    };
    dev.create_buffer(&desc, Some(bytes)).unwrap()
}

fn element(semantic: &str, format: u32, offset: u32) -> InputElement {
    InputElement {
        semantic: semantic.into(),
        semantic_index: 0,
        format,
        slot: 0,
        offset,
        per_instance: false,
        step_rate: 0,
    }
}

const RGBA8: u32 = 28;
const RG32F: u32 = 16;
const RGB32F: u32 = 6;
const RGBA32F: u32 = 2;
const D32F: u32 = 40;

/// What the rig's `Gpu::begin` does: clear, bind, full viewport.
fn begin(dev: &mut Dev, rtv: &Rc<View<Recorder>>, dsv: Option<&Rc<View<Recorder>>>, size: f32) {
    dev.clear_render_target_view(rtv, [0.1, 0.2, 0.3, 1.0]);
    if let Some(d) = dsv {
        dev.clear_depth_stencil_view(d, clear::DEPTH | clear::STENCIL, 1.0, 0);
    }
    dev.om_set_render_targets(&[Some(rtv.clone())], dsv.cloned())
        .unwrap();
    dev.rs_set_viewports(&[Viewport {
        width: size,
        height: size,
        max_depth: 1.0,
        ..Default::default()
    }]);
}

fn program(dev: &mut Dev, vs: &str, ps: &str, elements: &[InputElement]) {
    let (vsb, psb) = (corpus(vs), corpus(ps));
    let v = dev.create_shader(&vsb, SourceStage::Vertex).unwrap();
    let p = dev.create_shader(&psb, SourceStage::Pixel).unwrap();
    let layout = dev.create_input_layout(elements, &vsb).unwrap();
    dev.vs_set_shader(Some(v));
    dev.ps_set_shader(Some(p));
    dev.ia_set_input_layout(Some(layout));
}

/// What the rig's `Gpu::read` does: copy into a staging texture and map it for reading.
fn read(dev: &mut Dev, target: &Rc<Texture<Recorder>>) -> Vec<u8> {
    let mut desc = target.desc;
    desc.usage = usage::STAGING;
    desc.bind_flags = 0;
    desc.cpu_access_flags = cpu::READ;
    let staging = dev.create_texture(&desc, &[]).unwrap();
    let (dst, src) = (
        Resource::Texture(staging),
        Resource::Texture(target.clone()),
    );
    dev.copy_subresource_region(&dst, 0, [0; 3], &src, 0, None)
        .unwrap();
    let mapped = dev.map(&dst, 0, map::READ, 0).unwrap();
    assert_eq!(mapped.row_pitch, desc.width * 4);
    let bytes = unsafe {
        std::slice::from_raw_parts(mapped.data, (mapped.row_pitch * desc.height) as usize)
    }
    .to_vec();
    dev.unmap(&dst, 0);
    bytes
}

fn solid(dev: &mut Dev) {
    let r = dev
        .create_rasterizer_state(&RasterizerDesc {
            fill: 3,
            cull: 1,
            depth_clip: true,
            ..Default::default()
        })
        .unwrap();
    dev.rs_set_state(Some(r));
}

#[test]
fn shader_stages_are_read_from_the_container() {
    let dir = format!(
        "{}/../wcontainers-graphics-test/shaders/dxbc",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let kind = name.split('.').nth(1).unwrap();
        let want = match &kind[..2] {
            "vs" => SourceStage::Vertex,
            "ps" => SourceStage::Pixel,
            "gs" => SourceStage::Geometry,
            "hs" => SourceStage::Hull,
            "ds" => SourceStage::Domain,
            "cs" => SourceStage::Compute,
            _ => panic!("{name}"),
        };
        assert_eq!(
            shader::stage(&std::fs::read(&path).unwrap()),
            Ok(want),
            "{name}"
        );
        count += 1;
    }
    assert!(count > 50);
    let mut cut = corpus("triangle.vs");
    cut.truncate(40);
    assert!(matches!(shader::stage(&cut), Err(Error::InvalidArg(_))));
}

#[test]
fn feature_level_is_the_first_asked_for_that_is_served() {
    let dev = device();
    assert_eq!(dev.feature_level(), level::L11_0);
    let dev = Device::new(Recorder::new(None), &[]).unwrap();
    assert_eq!(dev.feature_level(), level::L11_0);
    assert_eq!(
        Device::new(Recorder::new(None), &[level::L11_1]).err(),
        Some(Error::Unsupported)
    );
}

#[test]
fn triangle_scene() {
    let mut dev = device();
    let target = dev
        .create_texture(&tex2d(256, 256, RGBA8, bind::RENDER_TARGET), &[])
        .unwrap();
    let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
    let start = dev.backend.log.len();
    begin(&mut dev, &rtv, None, 256.0);
    program(
        &mut dev,
        "triangle.vs",
        "triangle.ps",
        &[element("POSITION", RG32F, 0), element("COLOR", RGB32F, 8)],
    );
    let vertices = immutable(&mut dev, &[0; 60], bind::VERTEX_BUFFER);
    dev.ia_set_vertex_buffers(0, &[Some((vertices, 20, 0))])
        .unwrap();
    solid(&mut dev);
    dev.ia_set_primitive_topology(topology::TRIANGLELIST);
    dev.draw(3, 0);
    let pixels = read(&mut dev, &target);
    assert_eq!(pixels.len(), 256 * 256 * 4);
    let log = log_from(&dev, start);
    let layout = find(&log, "create_vertex_layout");
    assert!(
        log[layout].ends_with(
            "stream 0 stride 20 Vertex attribute 0 at 0+0 Float32x2 attribute 1 at 0+8 Float32x3"
        ),
        "{}",
        log[layout]
    );
    let pass = find(&log, "begin_pass");
    let clear = [0.1f32, 0.2, 0.3, 1.0].map(f32::to_bits);
    assert!(
        log[pass].contains(&format!("Clear({clear:?})")),
        "{}",
        log[pass]
    );
    let draw = find(&log, "draw ");
    assert!(
        log[draw].contains("TriangleList restart false"),
        "{}",
        log[draw]
    );
    assert!(log[draw].contains("Vertices { first: 0, count: 3, first_instance: 0, instances: 1 }"));
    assert!(log[draw].contains("viewports [[0.0, 0.0, 256.0, 256.0]]"));
    assert!(
        pass < draw
            && draw < find(&log, "end_pass")
            && find(&log, "end_pass") < find(&log, "copy_texture")
    );
    assert!(find(&log, "copy_texture") < find(&log, "read_back"));
}

#[test]
fn quads_scene() {
    let mut dev = device();
    let target = dev
        .create_texture(&tex2d(256, 256, RGBA8, bind::RENDER_TARGET), &[])
        .unwrap();
    let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
    let depth = dev
        .create_texture(&tex2d(256, 256, D32F, bind::DEPTH_STENCIL), &[])
        .unwrap();
    let dsv = dev.create_view(&depth, None, bind::DEPTH_STENCIL).unwrap();
    begin(&mut dev, &rtv, Some(&dsv), 256.0);
    program(
        &mut dev,
        "quads.vs",
        "quads.ps",
        &[
            element("POSITION", RGB32F, 0),
            element("TEXCOORD", RG32F, 12),
            element("COLOR", RGBA32F, 20),
        ],
    );
    let vertices = immutable(&mut dev, &[0; 36 * 8], bind::VERTEX_BUFFER);
    dev.ia_set_vertex_buffers(0, &[Some((vertices, 36, 0))])
        .unwrap();
    let indices = immutable(&mut dev, &[0; 24], bind::INDEX_BUFFER);
    let texels: Vec<u8> = (0..16 * 16 * 4).map(|i| i as u8).collect();
    let mut desc = tex2d(16, 16, RGBA8, bind::SHADER_RESOURCE);
    desc.usage = usage::IMMUTABLE;
    let texture = dev
        .create_texture(
            &desc,
            &[SubresourceData {
                bytes: &texels,
                row_pitch: 64,
                slice_pitch: 0,
            }],
        )
        .unwrap();
    let view = dev
        .create_view(&texture, None, bind::SHADER_RESOURCE)
        .unwrap();
    let sampler = dev
        .create_sampler_state(&SamplerDesc {
            filter: 0x15,
            address: [1; 3],
            mip_lod_bias: 0.0,
            max_anisotropy: 1,
            comparison: 1,
            border: [0.0; 4],
            min_lod: 0.0,
            max_lod: f32::MAX,
        })
        .unwrap();
    let mut blend = BlendDesc::default_state();
    blend.targets[0] = TargetBlend {
        enable: true,
        src: 5,
        dst: 6,
        op: 1,
        src_alpha: 2,
        dst_alpha: 6,
        op_alpha: 1,
        write_mask: 0xf,
    };
    let blend = dev.create_blend_state(&blend).unwrap();
    let ds = dev
        .create_depth_stencil_state(&DepthStencilDesc {
            depth_enable: true,
            depth_write: true,
            depth_func: 2,
            ..Default::default()
        })
        .unwrap();
    let start = dev.backend.log.len();
    solid(&mut dev);
    dev.ia_set_primitive_topology(topology::TRIANGLELIST);
    dev.ia_set_index_buffer(Some(indices), 57, 0).unwrap();
    let srv = Rc::new(ShaderView::Texture(view.clone()));
    dev.set_shader_resources(PIXEL, 0, &[Some(srv)]).unwrap();
    dev.set_samplers(PIXEL, 0, &[Some(sampler.clone())])
        .unwrap();
    dev.om_set_blend_state(Some(blend), None, u32::MAX);
    dev.om_set_depth_stencil_state(Some(ds), 0);
    dev.draw_indexed(12, 0, 0);
    let log = log_from(&dev, start);
    let set = find(&log, "create_binding_set");
    let (view_id, sampler_id) = (view.gpu.handle, sampler.gpu.handle);
    // Two entries, in whatever order the reflection lists them.
    assert!(
        log[set].contains(&format!(" 32=texture {view_id}"))
            && log[set].contains(&format!(" 16=sampler {sampler_id}"))
            && log[set].matches('=').count() == 2,
        "{}",
        log[set]
    );
    let pass = find(&log, "begin_pass");
    assert!(
        log[pass].contains("depth view")
            && log[pass].contains(&format!("Some(Clear({}))", 1.0f32.to_bits())),
        "{}",
        log[pass]
    );
    let draw = find(&log, "draw ");
    assert!(
        log[draw].contains("U16)) TriangleList restart false"),
        "{}",
        log[draw]
    );
    assert!(
        log[draw].contains("Indexed { first: 0, count: 12, base_vertex: 0"),
        "{}",
        log[draw]
    );
    assert!(
        log[draw].contains("sets [None, Some("),
        "pixel set bound in set 1: {}",
        log[draw]
    );
    // The texture's initial data reached the back end whole.
    let read_back = {
        let mut d = desc;
        d.usage = usage::STAGING;
        d.bind_flags = 0;
        d.cpu_access_flags = cpu::READ;
        let s = dev.create_texture(&d, &[]).unwrap();
        let s = Resource::Texture(s);
        dev.copy_resource(&s, &Resource::Texture(texture)).unwrap();
        let m = dev.map(&s, 0, map::READ, 0).unwrap();
        let bytes = unsafe { std::slice::from_raw_parts(m.data, 16 * 16 * 4) }.to_vec();
        dev.unmap(&s, 0);
        bytes
    };
    assert_eq!(read_back, texels);
}

#[test]
fn topology_scene() {
    let mut dev = device();
    let target = dev
        .create_texture(&tex2d(256, 256, RGBA8, bind::RENDER_TARGET), &[])
        .unwrap();
    let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
    begin(&mut dev, &rtv, None, 256.0);
    program(
        &mut dev,
        "common.vs_color",
        "common.ps_color",
        &[
            element("POSITION", RGB32F, 0),
            element("COLOR", RGBA32F, 12),
        ],
    );
    let vertices = immutable(&mut dev, &[0; 28 * 100], bind::VERTEX_BUFFER);
    dev.ia_set_vertex_buffers(0, &[Some((vertices, 28, 0))])
        .unwrap();
    solid(&mut dev);
    let start = dev.backend.log.len();
    let draws = [
        (topology::POINTLIST, 8),
        (topology::LINELIST, 8),
        (topology::LINESTRIP, 8),
        (topology::TRIANGLELIST, 6),
        (topology::TRIANGLESTRIP, 8),
        (topology::LINELIST_ADJ, 8),
        (topology::LINESTRIP_ADJ, 8),
        (topology::TRIANGLELIST_ADJ, 12),
        (topology::TRIANGLESTRIP_ADJ, 12),
    ];
    let mut first = 0;
    for (t, n) in draws {
        dev.ia_set_primitive_topology(t);
        dev.draw(n, first);
        first += n;
    }
    let cut16: Vec<u8> = [0u16, 1, 2, 3, 0xffff, 4, 5, 6, 7]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let indices = immutable(&mut dev, &cut16, bind::INDEX_BUFFER);
    dev.ia_set_primitive_topology(topology::TRIANGLESTRIP);
    dev.ia_set_index_buffer(Some(indices), 57, 0).unwrap();
    dev.draw_indexed(9, 0, first as i32);
    let log = log_from(&dev, start);
    let draws: Vec<&String> = log.iter().filter(|l| l.starts_with("draw ")).collect();
    assert_eq!(draws.len(), 10);
    for (line, kind) in draws.iter().zip([
        "PointList",
        "LineList",
        "LineStrip",
        "TriangleList",
        "TriangleStrip",
        "LineList",
        "LineList",
        "TriangleList",
        "TriangleList",
        "TriangleStrip",
    ]) {
        assert!(line.contains(&format!(" {kind} restart")), "{line}");
    }
    // Adjacency: drawn from generated 32-bit indices, the adjacent vertices dropped.
    assert!(
        draws[5].contains("Indexed { first: 0, count: 4, base_vertex: 38"),
        "{}",
        draws[5]
    );
    assert!(
        draws[6].contains("count: 10, base_vertex: 46"),
        "{}",
        draws[6]
    );
    assert!(
        draws[7].contains("count: 6, base_vertex: 54"),
        "{}",
        draws[7]
    );
    assert!(
        draws[8].contains("count: 12, base_vertex: 66"),
        "{}",
        draws[8]
    );
    assert!(draws[5].contains("U32))"), "{}", draws[5]);
    assert_eq!(
        dev.backend.slow_paths().count(SlowPath::AdjacencyDropped),
        4
    );
    assert!(
        draws[9].contains("U16)) TriangleStrip restart true"),
        "{}",
        draws[9]
    );
    // One pass for every draw: nothing between them ends it.
    assert_eq!(
        log.iter().filter(|l| l.starts_with("begin_pass")).count(),
        1
    );
}

#[test]
fn released_objects_are_destroyed_at_the_next_call() {
    let mut dev = device();
    let buffer = immutable(&mut dev, &[1; 16], bind::VERTEX_BUFFER);
    let id = buffer.handle();
    dev.ia_set_vertex_buffers(0, &[Some((buffer.clone(), 16, 0))])
        .unwrap();
    drop(buffer);
    dev.flush();
    assert!(
        !dev.backend
            .log
            .iter()
            .any(|l| l == &format!("destroy Buffer({id})")),
        "still bound"
    );
    dev.ia_set_vertex_buffers(0, &[None]).unwrap();
    dev.flush();
    assert!(
        dev.backend
            .log
            .iter()
            .any(|l| l == &format!("destroy Buffer({id})"))
    );
}

#[test]
fn creation_checks_refuse_what_direct3d_refuses() {
    let mut dev = device();
    let cb = |size| BufferDesc {
        byte_width: size,
        usage: usage::DEFAULT,
        bind_flags: bind::CONSTANT_BUFFER,
        ..Default::default()
    };
    assert!(matches!(
        dev.create_buffer(&cb(17), None),
        Err(Error::InvalidArg(_))
    ));
    assert!(dev.create_buffer(&cb(16), None).is_ok());
    let mut immutable = cb(16);
    immutable.usage = usage::IMMUTABLE;
    assert!(matches!(
        dev.create_buffer(&immutable, None),
        Err(Error::InvalidArg(_))
    ));
    assert!(matches!(
        dev.create_texture(&tex2d(16385, 1, RGBA8, 0), &[]),
        Err(Error::InvalidArg(_))
    ));
    assert!(matches!(
        dev.create_shader(b"not a shader at all, thirty-two+", SourceStage::Vertex),
        Err(Error::InvalidArg(_))
    ));
    assert!(matches!(
        dev.create_shader(&corpus("triangle.ps"), SourceStage::Vertex),
        Err(Error::InvalidArg(_))
    ));
    assert!(matches!(
        dev.create_shader(&corpus("geometry.gs_points"), SourceStage::Vertex),
        Err(Error::InvalidArg(_))
    ));
    // Valid, but read a value the converter does not provide yet.
    assert!(matches!(
        dev.create_shader(&corpus("geometry.ps_id"), SourceStage::Pixel),
        Err(Error::NotImplemented("PrimitiveId"))
    ));
    let missing = dev.create_input_layout(&[element("POSITION", RG32F, 0)], &corpus("triangle.vs"));
    assert!(matches!(missing, Err(Error::InvalidArg(_))));
    let target = dev
        .create_texture(&tex2d(4, 4, RGBA8, bind::SHADER_RESOURCE), &[])
        .unwrap();
    assert!(matches!(
        dev.create_view(&target, None, bind::RENDER_TARGET),
        Err(Error::InvalidArg(_))
    ));
    // Targets past the first are ignored unless they blend independently, as the quads scene relies on.
    let mut blend = BlendDesc::default();
    blend.targets[0] = BlendDesc::default_state().targets[0];
    assert!(dev.create_blend_state(&blend).is_ok());
    // Independent targets that do not blend may hold zero factors (Wine's test_independent_blend
    // and test_color_mask, on real Direct3D); one that blends with them is refused.
    blend.independent = true;
    assert!(dev.create_blend_state(&blend).is_ok());
    blend.targets[1].enable = true;
    assert!(matches!(
        dev.create_blend_state(&blend),
        Err(Error::InvalidArg(_))
    ));
}

#[test]
fn dynamic_constants_upload_whole_on_unmap() {
    let mut dev = device();
    let desc = BufferDesc {
        byte_width: 64,
        usage: usage::DYNAMIC,
        bind_flags: bind::CONSTANT_BUFFER,
        cpu_access_flags: cpu::WRITE,
        ..Default::default()
    };
    let b = Resource::Buffer(dev.create_buffer(&desc, None).unwrap());
    let m = dev.map(&b, 0, map::WRITE_DISCARD, 0).unwrap();
    unsafe { std::ptr::write_bytes(m.data, 7, 64) };
    dev.unmap(&b, 0);
    assert!(
        dev.backend
            .log
            .last()
            .unwrap()
            .ends_with("at 0 64 bytes Discard")
    );
    assert_eq!(
        dev.backend.slow_paths().count(SlowPath::MappedUploadWhole),
        1
    );
    assert!(matches!(
        dev.map(&b, 0, map::READ, 0),
        Err(Error::InvalidArg(_))
    ));
    dev.set_constant_buffers(VERTEX, 0, &[None]).unwrap();
}

/// The id of the object a log line starting `prefix` made.
fn made(log: &[String], prefix: &str) -> String {
    log[find(log, prefix)]
        .split(' ')
        .nth(1)
        .expect("an id")
        .to_string()
}

#[test]
fn empty_slots_read_zeros_and_sample_with_the_default_state() {
    let mut dev = device();
    let target = dev
        .create_texture(&tex2d(16, 16, RGBA8, bind::RENDER_TARGET), &[])
        .unwrap();
    let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
    begin(&mut dev, &rtv, None, 16.0);
    let mut color = element("COLOR", RGBA32F, 0);
    color.slot = 1;
    let elements = [
        element("POSITION", RGB32F, 0),
        element("TEXCOORD", RG32F, 12),
        color,
    ];
    program(&mut dev, "quads.vs", "quads.ps", &elements);
    let vertices = immutable(&mut dev, &[0; 20 * 3], bind::VERTEX_BUFFER);
    dev.ia_set_vertex_buffers(0, &[Some((vertices, 20, 0))])
        .unwrap();
    dev.ia_set_primitive_topology(topology::TRIANGLELIST);
    // Vertex slot 1, the pixel shader's texture slot 0 and its sampler slot 0 are empty.
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    let zero = made(&log, "create_buffer");
    assert!(
        log[find(&log, "create_buffer")].contains("size 65536"),
        "{log:#?}"
    );
    find(&log, &format!("clear_buffer {zero} "));
    let texture = made(&log, "create_texture");
    assert!(log[find(&log, "create_texture")].contains("Rgba8Unorm 1x1x1"));
    find(&log, &format!("write_texture {texture} "));
    let view = made(&log, "create_view");
    // Direct3D's default sampler: linear filtering, clamped.
    let sampler = made(&log, "create_sampler");
    assert!(
        log[find(&log, "create_sampler")].contains("Linear Linear Linear [Clamp, Clamp, Clamp]"),
        "{log:#?}"
    );
    let set = &log[find(&log, "create_binding_set")];
    assert!(set.contains(&format!("=texture {view}")), "{set}");
    assert!(set.contains(&format!("=sampler {sampler}")), "{set}");
    assert!(log[find(&log, "create_vertex_layout")].contains("stream 1 stride 0"));
    let draw = &log[find(&log, "draw ")];
    assert!(draw.contains(&format!("Some(({zero}, 0))]")), "{draw}");

    // A second draw makes nothing new.
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    let log = log_from(&dev, from);
    assert!(!log.iter().any(|l| l.starts_with("create_")), "{log:#?}");
}

#[test]
fn copies_between_groups_are_invalid_and_within_one_copy_bits() {
    let mut dev = device();
    let source = Resource::Texture(
        dev.create_texture(&tex2d(4, 4, RGBA8, bind::SHADER_RESOURCE), &[])
            .unwrap(),
    );
    let staging = |dev: &mut Dev, format| {
        let mut d = tex2d(4, 4, format, 0);
        d.usage = usage::STAGING;
        d.cpu_access_flags = cpu::READ;
        Resource::Texture(dev.create_texture(&d, &[]).unwrap())
    };
    // R8G8B8A8_UNORM_SRGB and _UINT share the group; R32_FLOAT does not; R32G32_UINT and BC1 are
    // Direct3D 10.1's conversion, not served.
    for format in [29, 30] {
        let dst = staging(&mut dev, format);
        dev.copy_resource(&dst, &source).unwrap();
        dev.copy_subresource_region(&dst, 0, [0; 3], &source, 0, None)
            .unwrap();
    }
    let dst = staging(&mut dev, 41);
    assert!(matches!(
        dev.copy_resource(&dst, &source),
        Err(Error::InvalidArg(_))
    ));
    assert!(matches!(
        dev.copy_subresource_region(&dst, 0, [0; 3], &source, 0, None),
        Err(Error::InvalidArg(_))
    ));
    let wide = Resource::Texture(
        dev.create_texture(&tex2d(1, 1, 17, bind::SHADER_RESOURCE), &[])
            .unwrap(),
    );
    let bc1 = staging(&mut dev, 71);
    assert!(matches!(
        dev.copy_subresource_region(&bc1, 0, [0; 3], &wide, 0, None),
        Err(Error::NotImplemented(_))
    ));
    assert!(dev.removed.is_none());
}

#[test]
fn depth_stencil_copies_are_whole_and_of_both_aspects() {
    let mut dev = device();
    let ds = |dev: &mut Dev| {
        Resource::Texture(
            dev.create_texture(&tex2d(4, 4, 45, bind::DEPTH_STENCIL), &[])
                .unwrap(),
        )
    };
    let (a, b) = (ds(&mut dev), ds(&mut dev));
    let from = dev.backend.log.len();
    dev.copy_resource(&b, &a).unwrap();
    dev.copy_subresource_region(&b, 0, [0; 3], &a, 0, None)
        .unwrap();
    // A part of a depth-stencil subresource is dropped, as Direct3D drops it.
    let part = Box3 {
        left: 0,
        top: 0,
        front: 0,
        right: 2,
        bottom: 2,
        back: 1,
    };
    dev.copy_subresource_region(&b, 0, [0; 3], &a, 0, Some(&part))
        .unwrap();
    dev.copy_subresource_region(&b, 0, [1, 0, 0], &a, 0, None)
        .unwrap();
    let copies: Vec<_> = log_from(&dev, from)
        .into_iter()
        .filter(|l| l.starts_with("copy_texture "))
        .collect();
    assert_eq!(copies.len(), 2, "{copies:#?}");
    assert!(copies.iter().all(|l| l.ends_with(" 4x4")), "{copies:#?}");
    assert!(dev.removed.is_none());
}

/// Per draw, the vertex helper set it binds and the bases (base vertex, first instance) that set's
/// block holds; none for a draw without one.
fn draw_bases(dev: &Dev) -> Vec<Option<(String, i32, u32)>> {
    let log = &dev.backend.log;
    let bases = |line: &str| -> Option<(String, i32, u32)> {
        let sets = line.split_once("sets [")?.1.split_once(']')?.0;
        let set = sets
            .split(", ")
            .nth(3)?
            .strip_prefix("Some(")?
            .strip_suffix(')')?;
        let prefix = format!("create_binding_set {set} 0=uniform ");
        let made = log.iter().find_map(|m| m.strip_prefix(prefix.as_str()))?;
        let (buffer, rest) = made.split_once('+')?;
        let (offset, size) = rest.split_once(':')?;
        assert_eq!(size, "16", "{made}");
        let bytes = dev.backend.buffer_bytes(buffer.parse().ok()?)?;
        let at: usize = offset.parse().ok()?;
        let word =
            |i: usize| u32::from_le_bytes(bytes[at + 4 * i..at + 4 * i + 4].try_into().unwrap());
        Some((set.to_string(), word(0) as i32, word(1)))
    };
    log.iter()
        .filter(|l| l.starts_with("draw "))
        .map(|l| bases(l))
        .collect()
}

#[test]
fn vertex_ids_read_the_draws_bases_from_the_helper_constants() {
    let mut dev = device();
    let target = dev
        .create_texture(&tex2d(16, 16, RGBA8, bind::RENDER_TARGET), &[])
        .unwrap();
    let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
    begin(&mut dev, &rtv, None, 16.0);
    let vs = dev
        .create_shader(&corpus("common.vs_full"), SourceStage::Vertex)
        .unwrap();
    dev.vs_set_shader(Some(vs));
    dev.ia_set_primitive_topology(topology::TRIANGLELIST);
    let indices = immutable(&mut dev, &[0; 12], bind::INDEX_BUFFER);
    dev.ia_set_index_buffer(Some(indices), 57, 0).unwrap();
    // SV_VertexID and SV_InstanceID leave out the start and base locations the common form's
    // indices include; the front writes them into the helper block when they change.
    dev.draw(3, 0);
    dev.draw(3, 3);
    dev.draw_instanced(3, 2, 3, 0);
    dev.draw_instanced(3, 1, 0, 4);
    dev.draw_indexed(3, 0, -2);
    dev.draw_indexed_instanced(3, 1, 3, 5, 6);
    assert!(dev.removed.is_none());
    let bases = draw_bases(&dev);
    let values: Vec<_> = bases
        .iter()
        .map(|b| b.as_ref().map(|b| (b.1, b.2)))
        .collect();
    assert_eq!(
        values,
        [
            Some((0, 0)),
            Some((3, 0)),
            Some((3, 0)),
            Some((0, 4)),
            Some((-2, 0)),
            Some((5, 6)),
        ]
    );
    // Equal bases keep their block: one write per change.
    let sets: Vec<_> = bases.iter().flatten().map(|b| &b.0).collect();
    assert_eq!(sets[1], sets[2]);
    assert_ne!(sets[0], sets[1]);
    assert_eq!(
        dev.backend.slow_paths().count(SlowPath::DrawBaseInHelpers),
        5
    );

    // Indirect arguments are read back where the back end cannot take the bases from them.
    let words: Vec<u8> = [3u32, 1, 7, 2, 0, 0, 0, 0]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    let desc = BufferDesc {
        byte_width: 32,
        usage: usage::DEFAULT,
        misc_flags: MISC_DRAWINDIRECT_ARGS,
        ..Default::default()
    };
    let args = dev.create_buffer(&desc, Some(&words)).unwrap();
    dev.draw_indirect(&args, 0, false).unwrap();
    assert!(dev.removed.is_none());
    assert_eq!(
        draw_bases(&dev)
            .last()
            .unwrap()
            .as_ref()
            .map(|b| (b.1, b.2)),
        Some((7, 2))
    );
    assert!(
        dev.backend
            .log
            .last()
            .unwrap()
            .contains("Vertices { first: 7, count: 3, first_instance: 2, instances: 1 }"),
        "{:?}",
        dev.backend.log.last()
    );
    assert_eq!(
        dev.backend.slow_paths().count(SlowPath::IndirectReadBack),
        1
    );
    // A buffer not made for arguments, an offset off a word or arguments past the end are invalid
    // and draw nothing.
    let draws = draw_bases(&dev).len();
    let plain = immutable(&mut dev, &words, bind::VERTEX_BUFFER);
    assert!(matches!(
        dev.draw_indirect(&plain, 0, false),
        Err(Error::InvalidArg(_))
    ));
    assert!(dev.draw_indirect(&args, 2, false).is_err());
    assert!(dev.draw_indirect(&args, 16, true).is_err());
    assert_eq!(draw_bases(&dev).len(), draws);
    assert!(dev.removed.is_none());
}

#[test]
fn lines_by_the_quadrilateral_or_alpha_rule_are_not_served() {
    for (multisample, antialiased_lines) in [(true, false), (false, true)] {
        let mut dev = device();
        let target = dev
            .create_texture(&tex2d(16, 16, RGBA8, bind::RENDER_TARGET), &[])
            .unwrap();
        let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
        begin(&mut dev, &rtv, None, 16.0);
        let elements = [
            element("POSITION", RGB32F, 0),
            element("COLOR", RGBA32F, 12),
        ];
        program(&mut dev, "common.vs_color", "common.ps_color", &elements);
        let vertices = immutable(&mut dev, &[0; 28 * 4], bind::VERTEX_BUFFER);
        dev.ia_set_vertex_buffers(0, &[Some((vertices, 28, 0))])
            .unwrap();
        let state = dev
            .create_rasterizer_state(&RasterizerDesc {
                fill: 3,
                cull: 1,
                depth_clip: true,
                multisample,
                antialiased_lines,
                ..Default::default()
            })
            .unwrap();
        dev.rs_set_state(Some(state));
        // Triangles are drawn as usual; lines are not.
        dev.ia_set_primitive_topology(topology::TRIANGLELIST);
        dev.draw(3, 0);
        assert!(dev.removed.is_none());
        dev.ia_set_primitive_topology(topology::LINELIST);
        dev.draw(2, 0);
        assert!(dev.removed.is_some());
        assert_eq!(
            dev.backend
                .log
                .iter()
                .filter(|l| l.starts_with("draw "))
                .count(),
            1
        );
    }
}

/// A target, cleared and bound, with a program of the corpus and a vertex buffer of `stride` bytes.
fn ready(dev: &mut Dev, vs: &str, ps: &str, elements: &[InputElement], stride: u32) {
    let target = dev
        .create_texture(&tex2d(16, 16, RGBA8, bind::RENDER_TARGET), &[])
        .unwrap();
    let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
    begin(dev, &rtv, None, 16.0);
    program(dev, vs, ps, elements);
    let vertices = immutable(dev, &vec![0; stride as usize * 3], bind::VERTEX_BUFFER);
    dev.ia_set_vertex_buffers(0, &[Some((vertices, stride, 0))])
        .unwrap();
    dev.ia_set_primitive_topology(topology::TRIANGLELIST);
}

#[test]
fn a_new_pair_is_built_ahead_with_the_states_its_vertex_shader_was_drawn_with() {
    let mut dev = device();
    let elements = [
        element("POSITION", RGB32F, 0),
        element("COLOR", RGBA32F, 12),
    ];
    ready(
        &mut dev,
        "common.vs_color",
        "common.ps_color",
        &elements,
        28,
    );
    dev.draw(3, 0);
    // The first pair met no drawn state: nothing was built ahead.
    assert_eq!(dev.prepared_use(), (0, 0));
    let from = dev.backend.log.len();
    let ps = dev
        .create_shader(&corpus("common.ps_color"), SourceStage::Pixel)
        .unwrap();
    let prepared: Vec<_> = dev.backend.log[from..]
        .iter()
        .filter(|l| l.starts_with("prepare "))
        .cloned()
        .collect();
    assert_eq!(prepared.len(), 1, "{:#?}", &dev.backend.log[from..]);
    assert_eq!(dev.prepared_use(), (0, 1));
    dev.ps_set_shader(Some(ps));
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    assert_eq!(dev.prepared_use(), (1, 0));
    // The draw used the program and states prepared.
    let draw = dev
        .backend
        .log
        .iter()
        .rev()
        .find(|l| l.starts_with("draw "))
        .unwrap();
    let head = |l: &str| l.split(" layout ").next().unwrap().to_string();
    assert_eq!(
        head(draw).trim_start_matches("draw "),
        head(&prepared[0]).trim_start_matches("prepare ")
    );
}

#[test]
fn staging_depth_with_stencil_moves_both_aspects_in_direct3ds_layout() {
    // D24_UNORM_S8_UINT (four bytes a texel) and D32_FLOAT_S8X24_UINT (eight).
    for (format, texel) in [(45u32, 4usize), (20, 8)] {
        let mut dev = device();
        let desc = TextureDesc {
            usage: usage::STAGING,
            bind_flags: 0,
            cpu_access_flags: cpu::READ | cpu::WRITE,
            ..tex2d(2, 2, format, 0)
        };
        let a = Resource::Texture(dev.create_texture(&desc, &[]).unwrap());
        let b = Resource::Texture(dev.create_texture(&desc, &[]).unwrap());
        // Every texel distinct in depth and stencil (the unused bytes of D32_FLOAT_S8X24 zero).
        let bytes: Vec<u8> = (0..4u8)
            .flat_map(|i| {
                let mut t = vec![0u8; texel];
                t[..4].copy_from_slice(&[0x10 + i, 0x20 + i, 0x30 + i, 0x40 + i]);
                if texel == 8 {
                    t[4] = 0x50 + i;
                }
                t
            })
            .collect();
        let mapped = dev.map(&a, 0, map::WRITE, 0).unwrap();
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), mapped.data, bytes.len()) };
        let from = dev.backend.log.len();
        dev.unmap(&a, 0);
        let writes = dev.backend.log[from..]
            .iter()
            .filter(|l| l.starts_with("write_texture "))
            .count();
        assert_eq!(writes, 2, "one write per aspect");
        dev.copy_resource(&b, &a).unwrap();
        let mapped = dev.map(&b, 0, map::READ, 0).unwrap();
        assert_eq!(mapped.row_pitch as usize, 2 * texel);
        let back = unsafe { std::slice::from_raw_parts(mapped.data, bytes.len()) }.to_vec();
        dev.unmap(&b, 0);
        assert_eq!(back, bytes, "format {format}");
        assert!(dev.removed.is_none());
    }
}

#[test]
fn an_input_layout_without_elements_serves_a_vertex_shader_without_inputs() {
    let mut dev = device();
    // common.vs_full reads only SV_VertexID; common.vs_color reads a position and a colour.
    assert!(
        dev.create_input_layout(&[], &corpus("common.vs_full"))
            .is_ok()
    );
    assert!(matches!(
        dev.create_input_layout(&[], &corpus("common.vs_color")),
        Err(Error::InvalidArg(_))
    ));
}

#[test]
fn a_back_end_loss_reaches_the_program_where_it_waits() {
    let mut dev = device();
    let desc = BufferDesc {
        byte_width: 16,
        usage: usage::STAGING,
        cpu_access_flags: cpu::READ,
        ..Default::default()
    };
    let staging = Resource::Buffer(dev.create_buffer(&desc, None).unwrap());
    assert!(dev.map(&staging, 0, map::READ, 0).is_ok());
    dev.unmap(&staging, 0);
    assert_eq!(dev.poll_removed(), None);
    dev.backend.lost = Some(LossReason::Error);
    assert!(matches!(
        dev.map(&staging, 0, map::READ, 0),
        Err(Error::DeviceRemoved)
    ));
    assert_eq!(dev.removed, Some(Removal::Lost(LossReason::Error)));
    assert!(matches!(
        dev.create_buffer(&desc, None),
        Err(Error::DeviceRemoved)
    ));
}

#[test]
fn slow_path_warnings_are_handed_out_once() {
    let mut dev = device();
    let desc = BufferDesc {
        byte_width: 64,
        usage: usage::DYNAMIC,
        bind_flags: bind::CONSTANT_BUFFER,
        cpu_access_flags: cpu::WRITE,
        ..Default::default()
    };
    let b = Resource::Buffer(dev.create_buffer(&desc, None).unwrap());
    for _ in 0..2 {
        dev.map(&b, 0, map::WRITE_DISCARD, 0).unwrap();
        dev.unmap(&b, 0);
    }
    assert_eq!(dev.take_warnings(), [SlowPath::MappedUploadWhole.reason()]);
    dev.map(&b, 0, map::WRITE_DISCARD, 0).unwrap();
    dev.unmap(&b, 0);
    assert!(dev.take_warnings().is_empty());
}

#[test]
fn a_constant_buffer_smaller_than_its_shader_declares_grows() {
    let mut dev = device();
    // The instanced scene's vertex shader indexes its constants by instance; the program binds a buffer of
    // one register, which Direct3D reads as zeros past its end.
    ready(
        &mut dev,
        "instanced.vs",
        "instanced.ps",
        &[element("POSITION", RG32F, 0)],
        8,
    );
    let vs = dev
        .create_shader(&corpus("instanced.vs"), SourceStage::Vertex)
        .unwrap();
    let declared = vs.constant_sizes[0].1;
    assert!(declared > 16, "{declared}");
    let desc = BufferDesc {
        byte_width: 16,
        usage: usage::DYNAMIC,
        bind_flags: bind::CONSTANT_BUFFER,
        cpu_access_flags: cpu::WRITE,
        ..Default::default()
    };
    let cb = dev.create_buffer(&desc, None).unwrap();
    dev.set_constant_buffers(VERTEX, 0, &[Some(cb.clone())])
        .unwrap();
    let from = dev.backend.log.len();
    dev.draw_instanced(3, 4, 0, 0);
    dev.draw_instanced(3, 4, 0, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    let made = find(&log, "create_buffer ");
    assert!(
        log[made].contains(&format!(" size {declared} ")),
        "{log:#?}"
    );
    assert!(
        log.iter()
            .any(|l| l.starts_with("create_binding_set") && l.contains(&format!(":{declared}"))),
        "{log:#?}"
    );
    assert_eq!(log.iter().filter(|l| l.starts_with("draw ")).count(), 2);
    assert_eq!(
        dev.backend
            .slow_paths()
            .count(SlowPath::ConstantBufferGrown),
        1
    );
    // Its whole writes cover the grown bytes, so those past the program's stay zero.
    let r = Resource::Buffer(cb);
    dev.map(&r, 0, map::WRITE_DISCARD, 0).unwrap();
    dev.unmap(&r, 0);
    assert!(
        dev.backend
            .log
            .last()
            .unwrap()
            .ends_with(&format!("at 0 {declared} bytes Discard"))
    );
}

#[test]
fn formats_direct3d_defines_but_the_front_does_not_serve_are_not_implemented() {
    let mut dev = device();
    // R32G32B32_TYPELESS and _FLOAT are valid; 0 and 500 are no format.
    for typeless in [5, 6] {
        assert!(matches!(
            dev.create_texture(&tex2d(4, 4, typeless, bind::SHADER_RESOURCE), &[]),
            Err(Error::NotImplemented(_))
        ));
    }
    for bad in [0, 500] {
        assert!(matches!(
            dev.create_texture(&tex2d(4, 4, bad, bind::SHADER_RESOURCE), &[]),
            Err(Error::InvalidArg(_))
        ));
    }
}

#[test]
fn dual_source_blending_is_not_served() {
    let mut dev = device();
    let elements = [
        element("POSITION", RGB32F, 0),
        element("COLOR", RGBA32F, 12),
    ];
    ready(
        &mut dev,
        "common.vs_color",
        "common.ps_color",
        &elements,
        28,
    );
    let mut blend = BlendDesc::default_state();
    blend.targets[0].enable = true;
    // D3D11_BLEND_SRC1_COLOR.
    blend.targets[0].src = 16;
    let blend = dev.create_blend_state(&blend).unwrap();
    dev.om_set_blend_state(Some(blend), None, u32::MAX);
    dev.draw(3, 0);
    assert!(dev.removed.is_some());
    assert!(!dev.backend.log.iter().any(|l| l.starts_with("draw ")));
}

#[test]
fn staging_read_backs_start_at_the_copy_and_maps_may_not_wait() {
    let mut dev = device();
    dev.backend.reads_need_submit = true;
    let source = dev
        .create_texture(
            &tex2d(4, 4, RGBA8, bind::SHADER_RESOURCE),
            &[SubresourceData {
                bytes: &[7; 64],
                row_pitch: 16,
                slice_pitch: 0,
            }],
        )
        .unwrap();
    let staging = |dev: &mut Dev, access: u32| {
        let mut d = tex2d(4, 4, RGBA8, 0);
        d.usage = usage::STAGING;
        d.cpu_access_flags = access;
        Resource::Texture(dev.create_texture(&d, &[]).unwrap())
    };
    let src = Resource::Texture(source);
    let readable = staging(&mut dev, cpu::READ);
    let start = dev.backend.log.len();
    dev.copy_subresource_region(&readable, 0, [0; 3], &src, 0, None)
        .unwrap();
    let log = log_from(&dev, start);
    assert!(
        find(&log, "read_back") > find(&log, "copy_texture"),
        "{log:#?}"
    );
    // Not done yet: a map that may not wait is told so, and the work is submitted.
    let start = dev.backend.log.len();
    let r = dev.map(&readable, 0, map::READ, map::DO_NOT_WAIT);
    assert!(matches!(r, Err(Error::StillDrawing)));
    find(&log_from(&dev, start), "submit");
    let mapped = dev.map(&readable, 0, map::READ, map::DO_NOT_WAIT).unwrap();
    assert_eq!(unsafe { *mapped.data }, 7);
    dev.unmap(&readable, 0);
    // The shadow is current: mapping again reads nothing back.
    let start = dev.backend.log.len();
    dev.map(&readable, 0, map::READ, 0).unwrap();
    dev.unmap(&readable, 0);
    assert!(
        !log_from(&dev, start)
            .iter()
            .any(|l| l.starts_with("read_back"))
    );
    assert_eq!(dev.backend.slow_paths().count(SlowPath::ReadBackWait), 0);

    // A write-only map reads back only what the GPU wrote since the shadow was filled.
    let writable = staging(&mut dev, cpu::WRITE);
    let start = dev.backend.log.len();
    dev.map(&writable, 0, map::WRITE, 0).unwrap();
    dev.unmap(&writable, 0);
    assert!(
        !log_from(&dev, start)
            .iter()
            .any(|l| l.starts_with("read_back"))
    );
    assert_eq!(
        dev.backend.slow_paths().count(SlowPath::StagingRefreshed),
        0
    );
    dev.copy_subresource_region(&writable, 0, [0; 3], &src, 0, None)
        .unwrap();
    let mapped = dev.map(&writable, 0, map::WRITE, 0).unwrap();
    assert_eq!(unsafe { *mapped.data }, 7, "the GPU's bytes are kept");
    dev.unmap(&writable, 0);
    assert_eq!(
        dev.backend.slow_paths().count(SlowPath::StagingRefreshed),
        1
    );
    assert_eq!(dev.backend.slow_paths().count(SlowPath::ReadBackWait), 1);
}

#[test]
fn binding_the_same_objects_again_makes_no_new_binding_set() {
    let mut dev = device();
    ready(
        &mut dev,
        "instanced.vs",
        "instanced.ps",
        &[element("POSITION", RG32F, 0)],
        8,
    );
    let vs = dev
        .create_shader(&corpus("instanced.vs"), SourceStage::Vertex)
        .unwrap();
    let desc = BufferDesc {
        byte_width: vs.constant_sizes[0].1 as u32,
        usage: usage::DEFAULT,
        bind_flags: bind::CONSTANT_BUFFER,
        ..Default::default()
    };
    let cb = dev.create_buffer(&desc, None).unwrap();
    let sets = |dev: &mut Dev, rebind: Option<&Rc<Buffer<Recorder>>>| {
        let from = dev.backend.log.len();
        for _ in 0..3 {
            if let Some(b) = rebind {
                dev.set_constant_buffers(VERTEX, 0, &[Some(b.clone())])
                    .unwrap();
            }
            dev.draw_instanced(3, 4, 0, 0);
        }
        log_from(dev, from)
            .iter()
            .filter(|l| l.starts_with("create_binding_set"))
            .count()
    };
    assert_eq!(sets(&mut dev, Some(&cb)), 1, "the first draw makes it");
    assert_eq!(sets(&mut dev, Some(&cb)), 0, "rebinding the same buffer");
    let other = dev.create_buffer(&desc, None).unwrap();
    assert_eq!(sets(&mut dev, Some(&other)), 1, "a different buffer");
    assert!(dev.removed.is_none());
}

#[test]
fn a_view_of_a_target_is_unbound_and_cannot_be_bound() {
    let mut dev = device();
    let mut d = tex2d(8, 8, RGBA8, bind::RENDER_TARGET | bind::SHADER_RESOURCE);
    d.mip_levels = 2;
    let t = dev.create_texture(&d, &[]).unwrap();
    let srv = dev.create_view(&t, None, bind::SHADER_RESOURCE).unwrap();
    let srv = Rc::new(ShaderView::Texture(srv));
    let rtv = dev.create_view(&t, None, bind::RENDER_TARGET).unwrap();
    dev.set_shader_resources(PIXEL, 3, &[Some(srv.clone())])
        .unwrap();
    assert!(dev.shader_resource_bound(PIXEL, 3));
    dev.om_set_render_targets(&[Some(rtv)], None).unwrap();
    assert!(!dev.shader_resource_bound(PIXEL, 3));
    dev.set_shader_resources(PIXEL, 3, &[Some(srv)]).unwrap();
    assert!(!dev.shader_resource_bound(PIXEL, 3));
}

#[test]
fn typeless_textures_are_viewed_through_members_of_their_group() {
    let mut dev = device();
    let view = |format| ViewDesc {
        format,
        dimension: ViewDim::Texture2D,
        first_mip: 0,
        mip_count: 1,
        first_layer: 0,
        layer_count: 1,
    };
    // R24G8_TYPELESS: D24_UNORM_S8_UINT for depth, R24_UNORM_X8_TYPELESS and X24_TYPELESS_G8_UINT
    // to read depth and stencil; a D format is for depth-stencil views only.
    let depth = dev
        .create_texture(
            &tex2d(4, 4, 44, bind::DEPTH_STENCIL | bind::SHADER_RESOURCE),
            &[],
        )
        .unwrap();
    assert_eq!(depth.format, wcontainers_gpu::Format::Depth24PlusStencil8);
    dev.create_view(&depth, Some(&view(45)), bind::DEPTH_STENCIL)
        .unwrap();
    for (format, stencil) in [(46, false), (47, true)] {
        let from = dev.backend.log.len();
        dev.create_view(&depth, Some(&view(format)), bind::SHADER_RESOURCE)
            .unwrap();
        let made = log_from(&dev, from).join("\n");
        assert_eq!(made.ends_with(" stencil"), stencil, "{made}");
    }
    for (format, kind) in [
        (45, bind::SHADER_RESOURCE),
        (46, bind::DEPTH_STENCIL),
        (44, bind::SHADER_RESOURCE),
        (28, bind::SHADER_RESOURCE),
    ] {
        assert!(
            matches!(
                dev.create_view(&depth, Some(&view(format)), kind),
                Err(Error::InvalidArg(_))
            ),
            "{format}"
        );
    }
    // R32_TYPELESS bound for depth is stored as depth and read as R32_FLOAT.
    let r32 = dev
        .create_texture(
            &tex2d(4, 4, 39, bind::DEPTH_STENCIL | bind::SHADER_RESOURCE),
            &[],
        )
        .unwrap();
    assert_eq!(r32.format, wcontainers_gpu::Format::Depth32Float);
    dev.create_view(&r32, Some(&view(40)), bind::DEPTH_STENCIL)
        .unwrap();
    dev.create_view(&r32, Some(&view(41)), bind::SHADER_RESOURCE)
        .unwrap();
    // R8G8B8A8_TYPELESS: UNORM and UNORM_SRGB views; UINT would reinterpret the bits.
    let colour = dev
        .create_texture(
            &tex2d(4, 4, 27, bind::RENDER_TARGET | bind::SHADER_RESOURCE),
            &[],
        )
        .unwrap();
    dev.create_view(&colour, Some(&view(29)), bind::RENDER_TARGET)
        .unwrap();
    dev.create_view(&colour, Some(&view(28)), bind::SHADER_RESOURCE)
        .unwrap();
    assert!(matches!(
        dev.create_view(&colour, Some(&view(30)), bind::SHADER_RESOURCE),
        Err(Error::NotImplemented(_))
    ));
    assert!(matches!(
        dev.create_view(&colour, None, bind::SHADER_RESOURCE),
        Err(Error::InvalidArg(_))
    ));
    assert!(dev.removed.is_none());
}

#[test]
fn a_draw_goes_through_the_first_viewport_and_scissor_rectangle() {
    let mut dev = device();
    let elements = [
        element("POSITION", RGB32F, 0),
        element("COLOR", RGBA32F, 12),
    ];
    ready(
        &mut dev,
        "common.vs_color",
        "common.ps_color",
        &elements,
        28,
    );
    let half = Viewport {
        width: 32.0,
        height: 32.0,
        max_depth: 1.0,
        ..Default::default()
    };
    dev.rs_set_viewports(&[half, Viewport { x: 32.0, ..half }]);
    dev.rs_set_scissor_rects(&[[0, 0, 16, 16], [16, 16, 64, 64]]);
    let draws = |dev: &Dev, from: usize| -> Vec<String> {
        log_from(dev, from)
            .into_iter()
            .filter(|l| l.starts_with("draw "))
            .collect()
    };
    // Without the scissor test, no rectangle.
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    let d = draws(&dev, from);
    assert!(
        d[0].contains("viewports [[0.0, 0.0, 32.0, 32.0]] scissors []"),
        "{d:#?}"
    );
    // With it, the first rectangle; with none set, one that passes nothing.
    let r = dev
        .create_rasterizer_state(&RasterizerDesc {
            fill: 3,
            cull: 1,
            depth_clip: true,
            scissor: true,
            ..Default::default()
        })
        .unwrap();
    dev.rs_set_state(Some(r));
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    dev.rs_set_scissor_rects(&[]);
    dev.draw(3, 0);
    let d = draws(&dev, from);
    assert!(d[0].contains("scissors [[0, 0, 16, 16]]"), "{d:#?}");
    assert!(d[1].contains("scissors [[0, 0, 0, 0]]"), "{d:#?}");
    assert!(dev.removed.is_none());
}

const R32F: u32 = 41;

/// A full-target draw of `ps`, from the rig's samplers scene, reading an R32_FLOAT texture at t0.
fn sampling(dev: &mut Dev, ps: &str) {
    let target = dev
        .create_texture(&tex2d(16, 16, RGBA8, bind::RENDER_TARGET), &[])
        .unwrap();
    let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
    begin(dev, &rtv, None, 16.0);
    let v = dev
        .create_shader(&corpus("common.vs_full"), SourceStage::Vertex)
        .unwrap();
    let p = dev.create_shader(&corpus(ps), SourceStage::Pixel).unwrap();
    dev.vs_set_shader(Some(v));
    dev.ps_set_shader(Some(p));
    dev.ia_set_primitive_topology(topology::TRIANGLELIST);
    let texture = dev
        .create_texture(&tex2d(4, 4, R32F, bind::SHADER_RESOURCE), &[])
        .unwrap();
    let view = dev
        .create_view(&texture, None, bind::SHADER_RESOURCE)
        .unwrap();
    dev.set_shader_resources(PIXEL, 0, &[Some(Rc::new(ShaderView::Texture(view)))])
        .unwrap();
}

#[test]
fn samplers_are_bound_comparing_as_the_program_reads_them() {
    let mut dev = device();
    sampling(&mut dev, "samplers.ps_compare");
    // D3D11_COMPARISON_LESS, on a sampler whose filter does not compare.
    let plain = SamplerDesc {
        comparison: 2,
        ..SamplerDesc::default_state()
    };
    let plain = dev.create_sampler_state(&plain).unwrap();
    // The program compares through s1: the sampler is read through a twin that compares with its
    // function, made once.
    dev.set_samplers(PIXEL, 1, &[Some(plain.clone())]).unwrap();
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    let made_samplers: Vec<_> = log
        .iter()
        .filter(|l| l.starts_with("create_sampler"))
        .collect();
    assert_eq!(made_samplers.len(), 1, "{log:#?}");
    assert!(made_samplers[0].ends_with("compare Some(Less)"), "{log:#?}");
    let twin = made(&log, "create_sampler");
    let set = &log[find(&log, "create_binding_set")];
    assert!(set.contains(&format!("=sampler {twin}")), "{set}");

    // A comparing sampler is bound as it is.
    let comparing = SamplerDesc {
        filter: 0x95,
        comparison: 2,
        ..SamplerDesc::default_state()
    };
    let comparing = dev.create_sampler_state(&comparing).unwrap();
    dev.set_samplers(PIXEL, 1, &[Some(comparing.clone())])
        .unwrap();
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    let log = log_from(&dev, from);
    assert!(
        !log.iter().any(|l| l.starts_with("create_sampler")),
        "{log:#?}"
    );
    let set = &log[find(&log, "create_binding_set")];
    assert!(
        set.contains(&format!("=sampler {}", comparing.gpu.handle)),
        "{set}"
    );

    // A program that does not compare reads the comparing sampler through a twin that does not.
    sampling(&mut dev, "samplers.ps_sample");
    dev.set_samplers(PIXEL, 0, &[Some(comparing)]).unwrap();
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    assert!(
        log[find(&log, "create_sampler")].ends_with("compare None"),
        "{log:#?}"
    );
}

#[test]
fn a_texture_the_back_end_cannot_filter_is_not_sampled_linearly() {
    let mut dev = device();
    let uses = &mut dev.backend.caps.formats.uses[wcontainers_gpu::Format::R32Float.index()];
    uses.remove(wcontainers_gpu::FormatUse::FILTER);
    sampling(&mut dev, "samplers.ps_sample");
    // Point sampling needs no filtering.
    let point = SamplerDesc {
        filter: 0,
        ..SamplerDesc::default_state()
    };
    let point = dev.create_sampler_state(&point).unwrap();
    dev.set_samplers(PIXEL, 0, &[Some(point)]).unwrap();
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    assert_eq!(
        dev.backend
            .log
            .iter()
            .filter(|l| l.starts_with("draw "))
            .count(),
        1
    );
    // An empty slot reads Direct3D's default sampler, which filters linearly: the program would
    // have to filter, which the converter does not build yet.
    dev.set_samplers(PIXEL, 0, &[None]).unwrap();
    dev.draw(3, 0);
    assert!(dev.removed.is_some());
    assert_eq!(
        dev.backend
            .log
            .iter()
            .filter(|l| l.starts_with("draw "))
            .count(),
        1
    );
}

#[test]
fn a_pass_without_targets_is_as_large_as_the_viewports_it_draws_through() {
    let mut dev = device();
    let elements = [
        element("POSITION", RGB32F, 0),
        element("COLOR", RGBA32F, 12),
    ];
    ready(
        &mut dev,
        "common.vs_color",
        "common.ps_color",
        &elements,
        28,
    );
    dev.om_set_render_targets(&[], None).unwrap();
    let viewport = |dev: &mut Dev, width: f32, height: f32| {
        dev.rs_set_viewports(&[Viewport {
            width,
            height,
            max_depth: 1.0,
            ..Default::default()
        }]);
    };
    viewport(&mut dev, 40.0, 24.5);
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    // A smaller viewport draws in the same pass; a larger one opens another.
    viewport(&mut dev, 10.0, 10.0);
    dev.draw(3, 0);
    viewport(&mut dev, 64.0, 8.0);
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let passes: Vec<_> = log_from(&dev, from)
        .into_iter()
        .filter(|l| l.starts_with("begin_pass"))
        .collect();
    assert_eq!(
        passes,
        [
            "begin_pass empty 40x25 samples 1",
            "begin_pass empty 64x8 samples 1"
        ]
    );
}

#[test]
fn constants_rewritten_between_draws_make_no_binding_set_and_split_no_pass() {
    let mut dev = device();
    let elements = [
        element("POSITION", RGB32F, 0),
        element("COLOR", RGBA32F, 12),
    ];
    ready(
        &mut dev,
        "common.vs_color",
        "dynamic.ps_tint",
        &elements,
        28,
    );
    let dynamic = BufferDesc {
        byte_width: 16,
        usage: usage::DYNAMIC,
        bind_flags: bind::CONSTANT_BUFFER,
        cpu_access_flags: cpu::WRITE,
        ..Default::default()
    };
    let dynamic = dev.create_buffer(&dynamic, None).unwrap();
    let default = BufferDesc {
        byte_width: 16,
        usage: usage::DEFAULT,
        bind_flags: bind::CONSTANT_BUFFER,
        ..Default::default()
    };
    let default = dev.create_buffer(&default, Some(&[0; 16])).unwrap();
    let from = dev.backend.log.len();
    // Map(WRITE_DISCARD) before each draw.
    dev.set_constant_buffers(PIXEL, 0, &[Some(dynamic.clone())])
        .unwrap();
    let r = Resource::Buffer(dynamic);
    for i in 0..3 {
        let m = dev.map(&r, 0, map::WRITE_DISCARD, 0).unwrap();
        unsafe { std::ptr::write_bytes(m.data, i, 16) };
        dev.unmap(&r, 0);
        dev.draw(3, 0);
    }
    // UpdateSubresource, whole and in part, before each draw.
    dev.set_constant_buffers(PIXEL, 0, &[Some(default.clone())])
        .unwrap();
    let d = Resource::Buffer(default);
    dev.update_subresource(&d, 0, None, &[1; 16], 0, 0).unwrap();
    dev.draw(3, 0);
    let part = Box3 {
        left: 4,
        right: 8,
        bottom: 1,
        back: 1,
        ..Default::default()
    };
    dev.update_subresource(&d, 0, Some(&part), &[2; 4], 0, 0)
        .unwrap();
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    let count = |p: &str| log.iter().filter(|l| l.starts_with(p)).count();
    assert_eq!(count("draw "), 5, "{log:#?}");
    assert_eq!(count("begin_pass"), 1, "{log:#?}");
    assert_eq!(count("end_pass"), 0, "{log:#?}");
    // One binding set for each buffer bound, none for a draw.
    assert_eq!(count("create_binding_set"), 2, "{log:#?}");
    let writes: Vec<_> = log
        .iter()
        .filter(|l| l.starts_with("write_buffer"))
        .collect();
    assert_eq!(writes.len(), 5, "{log:#?}");
    assert!(
        writes[..4]
            .iter()
            .all(|w| w.ends_with("at 0 16 bytes Discard")),
        "{writes:#?}"
    );
    assert!(writes[4].ends_with("at 4 4 bytes Ordered"), "{writes:#?}");
}
