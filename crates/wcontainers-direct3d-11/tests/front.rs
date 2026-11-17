//! The front on the recording back end: the rig's first three Direct3D 11 scenes made as the scene
//! programs make them (their DXBC from the rig's corpus), checked by the calls recorded; and the
//! creation checks, lifetimes and maps.

use std::rc::Rc;

use wcontainers_direct3d_11::desc::*;
use wcontainers_direct3d_11::object::{Buffer, Resource, Texture2D, View};
use wcontainers_direct3d_11::record::Recorder;
use wcontainers_direct3d_11::{Device, Error, PIXEL, VERTEX, standin};
use wcontainers_gpu::{Backend, SlowPath, SourceStage};

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

fn tex2d(width: u32, height: u32, format: u32, bind: u32) -> Texture2DDesc {
    Texture2DDesc {
        width,
        height,
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
fn read(dev: &mut Dev, target: &Rc<Texture2D<Recorder>>) -> Vec<u8> {
    let mut desc = target.desc;
    desc.usage = usage::STAGING;
    desc.bind_flags = 0;
    desc.cpu_access_flags = cpu::READ;
    let staging = dev.create_texture_2d(&desc, &[]).unwrap();
    let (dst, src) = (
        Resource::Texture2D(staging),
        Resource::Texture2D(target.clone()),
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
fn stand_in_keys_name_the_corpus_shaders() {
    let mut wrong = Vec::new();
    for (source, key) in standin::sources() {
        let actual: String = standin::key(&corpus(source))
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if actual != key {
            wrong.push(format!("{source}: {actual}"));
        }
    }
    assert!(wrong.is_empty(), "keys differ:\n{}", wrong.join("\n"));
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
        .create_texture_2d(&tex2d(256, 256, RGBA8, bind::RENDER_TARGET), &[])
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
        .create_texture_2d(&tex2d(256, 256, RGBA8, bind::RENDER_TARGET), &[])
        .unwrap();
    let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
    let depth = dev
        .create_texture_2d(&tex2d(256, 256, D32F, bind::DEPTH_STENCIL), &[])
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
        .create_texture_2d(
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
    dev.set_shader_resources(PIXEL, 0, &[Some(view.clone())])
        .unwrap();
    dev.set_samplers(PIXEL, 0, &[Some(sampler.clone())])
        .unwrap();
    dev.om_set_blend_state(Some(blend), None, u32::MAX);
    dev.om_set_depth_stencil_state(Some(ds), 0);
    dev.draw_indexed(12, 0, 0);
    let log = log_from(&dev, start);
    let set = find(&log, "create_binding_set");
    let (view_id, sampler_id) = (view.gpu.handle, sampler.gpu.handle);
    assert!(
        log[set].ends_with(&format!(" 32=texture {view_id} 16=sampler {sampler_id}")),
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
        let s = dev.create_texture_2d(&d, &[]).unwrap();
        let s = Resource::Texture2D(s);
        dev.copy_resource(&s, &Resource::Texture2D(texture))
            .unwrap();
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
        .create_texture_2d(&tex2d(256, 256, RGBA8, bind::RENDER_TARGET), &[])
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
    let id = buffer.gpu.handle;
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
        dev.create_texture_2d(&tex2d(16385, 1, RGBA8, 0), &[]),
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
        dev.create_shader(&corpus("blend.vs"), SourceStage::Vertex),
        Err(Error::NotImplemented(_))
    ));
    let missing = dev.create_input_layout(&[element("POSITION", RG32F, 0)], &corpus("triangle.vs"));
    assert!(matches!(missing, Err(Error::InvalidArg(_))));
    let target = dev
        .create_texture_2d(&tex2d(4, 4, RGBA8, bind::SHADER_RESOURCE), &[])
        .unwrap();
    assert!(matches!(
        dev.create_view(&target, None, bind::RENDER_TARGET),
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
