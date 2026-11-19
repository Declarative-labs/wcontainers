//! The front on the recording back end: the rig's first three Direct3D 11 scenes made as the scene
//! programs make them (their DXBC from the rig's corpus), checked by the calls recorded; and the
//! creation checks, lifetimes and maps.

use std::rc::Rc;

use wcontainers_direct3d_11::desc::*;
use wcontainers_direct3d_11::object::{Buffer, Resource, ShaderView, Texture, View};
use wcontainers_direct3d_11::record::Recorder;
use wcontainers_direct3d_11::{
    DOMAIN, Device, Error, GEOMETRY, HULL, PIXEL, Removal, VERTEX, shader,
};
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
fn nv12_is_two_planes_viewed_apart_and_written_copied_and_mapped_as_joined_rows() {
    const NV12: u32 = 103;
    let mut dev = device();
    let bind = bind::SHADER_RESOURCE | bind::RENDER_TARGET;
    for (w, h) in [(7, 4), (8, 3)] {
        assert!(matches!(
            dev.create_texture(&tex2d(w, h, NV12, bind), &[]),
            Err(Error::InvalidArg(_))
        ));
    }
    let (w, h) = (8u32, 4u32);
    let bytes: Vec<u8> = (0..w * h * 3 / 2).map(|i| (i * 7 + 1) as u8).collect();
    let init = [SubresourceData {
        bytes: &bytes,
        row_pitch: w,
        slice_pitch: 0,
    }];
    let texture = dev.create_texture(&tex2d(w, h, NV12, bind), &init).unwrap();
    let view = |format| ViewDesc {
        format,
        dimension: ViewDim::Texture2D,
        first_mip: 0,
        mip_count: 1,
        first_layer: 0,
        layer_count: 1,
        read_only: 0,
    };
    // Views name one plane by its format; the default and NV12's own are invalid.
    for desc in [None, Some(view(NV12)), Some(view(63))] {
        assert!(matches!(
            dev.create_view(&texture, desc.as_ref(), bind::SHADER_RESOURCE),
            Err(Error::InvalidArg(_))
        ));
    }
    let luma = dev
        .create_view(&texture, Some(&view(61)), bind::SHADER_RESOURCE)
        .unwrap();
    assert!(Rc::ptr_eq(&luma.texture, &texture));
    let chroma = dev
        .create_view(&texture, Some(&view(49)), bind::RENDER_TARGET)
        .unwrap();
    let plane = texture.chroma.as_ref().unwrap();
    assert!(Rc::ptr_eq(&chroma.texture, plane));
    assert_eq!((plane.desc.width, plane.desc.height), (w / 2, h / 2));

    let mut staging_desc = tex2d(w, h, NV12, 0);
    staging_desc.usage = usage::STAGING;
    staging_desc.cpu_access_flags = cpu::READ;
    let staging = Resource::Texture(dev.create_texture(&staging_desc, &[]).unwrap());
    let nv12 = Resource::Texture(texture.clone());
    let read = |dev: &mut Dev| {
        dev.copy_resource(&staging, &nv12).unwrap();
        let mapped = dev.map(&staging, 0, map::READ, 0).unwrap();
        assert_eq!(mapped.row_pitch, w);
        assert_eq!(mapped.depth_pitch, w * h * 3 / 2);
        let got = unsafe { std::slice::from_raw_parts(mapped.data, bytes.len()) }.to_vec();
        dev.unmap(&staging, 0);
        got
    };
    assert_eq!(read(&mut dev), bytes);
    // An odd box writes nothing; an even one writes its luma and the chroma under it.
    let source = [0xa0u8; 4 * 2 * 3 / 2];
    let odd = Box3 {
        left: 1,
        top: 2,
        front: 0,
        right: 5,
        bottom: 4,
        back: 1,
    };
    dev.update_subresource(&nv12, 0, Some(&odd), &source, 4, 0)
        .unwrap();
    assert_eq!(read(&mut dev), bytes);
    let even = Box3 {
        left: 2,
        right: 6,
        ..odd
    };
    dev.update_subresource(&nv12, 0, Some(&even), &source, 4, 0)
        .unwrap();
    let mut want = bytes.clone();
    for y in 2..4 {
        want[(y * w + 2) as usize..(y * w + 6) as usize].fill(0xa0);
    }
    // Chroma row 1 (under luma rows 2 and 3), pairs 1 and 2.
    let at = (w * h + w + 2) as usize;
    want[at..at + 4].fill(0xa0);
    assert_eq!(read(&mut dev), want);
    assert!(dev.removed.is_none());
}

#[test]
fn yuy2_is_a_half_width_texture_of_pixel_pairs_viewed_as_rgba8() {
    const YUY2: u32 = 107;
    let mut dev = device();
    let bind = bind::SHADER_RESOURCE | bind::RENDER_TARGET;
    assert!(matches!(
        dev.create_texture(&tex2d(7, 4, YUY2, bind), &[]),
        Err(Error::InvalidArg(_))
    ));
    let (w, h) = (8u32, 4u32);
    let bytes: Vec<u8> = (0..w * h * 2).map(|i| (i * 7 + 1) as u8).collect();
    let init = [SubresourceData {
        bytes: &bytes,
        row_pitch: w * 2,
        slice_pitch: 0,
    }];
    let texture = dev.create_texture(&tex2d(w, h, YUY2, bind), &init).unwrap();
    assert_eq!(texture.desc.width, w);
    assert_eq!(texture.gpu_desc.width, w / 2);
    let view = |format| ViewDesc {
        format,
        dimension: ViewDim::Texture2D,
        first_mip: 0,
        mip_count: 1,
        first_layer: 0,
        layer_count: 1,
        read_only: 0,
    };
    // R8G8B8A8_UNORM and _UINT only.
    for desc in [None, Some(view(YUY2)), Some(view(29)), Some(view(31))] {
        assert!(matches!(
            dev.create_view(&texture, desc.as_ref(), bind::SHADER_RESOURCE),
            Err(Error::InvalidArg(_))
        ));
    }
    for format in [28, 30] {
        dev.create_view(&texture, Some(&view(format)), bind::SHADER_RESOURCE)
            .unwrap();
    }
    let mut staging_desc = tex2d(w, h, YUY2, 0);
    staging_desc.usage = usage::STAGING;
    staging_desc.cpu_access_flags = cpu::READ;
    let staging = Resource::Texture(dev.create_texture(&staging_desc, &[]).unwrap());
    let yuy2 = Resource::Texture(texture.clone());
    let read = |dev: &mut Dev| {
        dev.copy_resource(&staging, &yuy2).unwrap();
        let mapped = dev.map(&staging, 0, map::READ, 0).unwrap();
        assert_eq!(mapped.row_pitch, w * 2);
        let got = unsafe { std::slice::from_raw_parts(mapped.data, bytes.len()) }.to_vec();
        dev.unmap(&staging, 0);
        got
    };
    assert_eq!(read(&mut dev), bytes);
    // Boxes are in pixels: one at an odd pixel writes nothing, an even one its pairs.
    let source = [0xa0u8; 4 * 2 * 2];
    let odd = Box3 {
        left: 1,
        top: 2,
        front: 0,
        right: 5,
        bottom: 4,
        back: 1,
    };
    dev.update_subresource(&yuy2, 0, Some(&odd), &source, 8, 0)
        .unwrap();
    assert_eq!(read(&mut dev), bytes);
    let even = Box3 {
        left: 2,
        right: 6,
        ..odd
    };
    dev.update_subresource(&yuy2, 0, Some(&even), &source, 8, 0)
        .unwrap();
    let mut want = bytes.clone();
    for y in 2..4 {
        want[(y * w * 2 + 4) as usize..(y * w * 2 + 12) as usize].fill(0xa0);
    }
    assert_eq!(read(&mut dev), want);
    assert!(dev.removed.is_none());
}

#[test]
fn a_target_view_of_a_buffer_draws_into_a_row_of_its_elements_copied_into_the_buffer() {
    let mut dev = device();
    let buffer = dev
        .create_buffer(
            &BufferDesc {
                byte_width: 1024,
                usage: usage::DEFAULT,
                bind_flags: bind::RENDER_TARGET,
                ..Default::default()
            },
            None,
        )
        .unwrap();
    // Past the buffer's end, of a typeless format, and of a buffer not bound as a target.
    for (format, first, count) in [(RGBA32F, 1, 64), (1, 0, 64), (RGBA32F, 0, 0)] {
        assert!(matches!(
            dev.create_buffer_target_view(&buffer, format, first, count),
            Err(Error::InvalidArg(_))
        ));
    }
    let vertices = immutable(&mut dev, &[0; 60], bind::VERTEX_BUFFER);
    assert!(matches!(
        dev.create_buffer_target_view(&vertices, RGBA32F, 0, 4),
        Err(Error::InvalidArg(_))
    ));
    let rtv = dev
        .create_buffer_target_view(&buffer, RGBA32F, 16, 48)
        .unwrap();
    let start = dev.backend.log.len();
    begin(&mut dev, &rtv, None, 48.0);
    program(
        &mut dev,
        "triangle.vs",
        "triangle.ps",
        &[element("POSITION", RG32F, 0), element("COLOR", RGB32F, 8)],
    );
    dev.ia_set_vertex_buffers(0, &[Some((vertices, 20, 0))])
        .unwrap();
    dev.ia_set_primitive_topology(topology::TRIANGLELIST);
    dev.draw(3, 0);
    dev.flush();
    assert!(dev.removed.is_none());
    let log = log_from(&dev, start);
    // Cleared, so nothing is copied in; what the pass drew is copied out from element 16's byte.
    assert!(
        !log.iter().any(|l| l.starts_with("copy_buffer_to_texture")),
        "{log:#?}"
    );
    let out = find(&log, "copy_texture_to_buffer");
    assert!(find(&log, "draw ") < out, "{log:#?}");
    assert!(log[out].ends_with("48x1"), "{}", log[out]);
    // Drawn again without a clear, the elements are copied in first.
    let start = dev.backend.log.len();
    dev.om_set_render_targets(&[None], None).unwrap();
    dev.om_set_render_targets(&[Some(rtv)], None).unwrap();
    dev.draw(3, 0);
    dev.flush();
    let log = log_from(&dev, start);
    assert!(
        find(&log, "copy_buffer_to_texture") < find(&log, "draw "),
        "{log:#?}"
    );
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
    // Valid, but read a value the back end does not provide (the recording back end has no
    // primitive index), and the stage helper's route is not built.
    assert!(matches!(
        dev.create_shader(&corpus("geometry.ps_id"), SourceStage::Pixel),
        Err(Error::NotImplemented(
            "the primitive index without the back end's (PrimitiveIdAsGeometry)"
        ))
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
    // Direct3D 10.1's conversion, the texel copying into the block.
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
    let from = dev.backend.log.len();
    dev.copy_subresource_region(&bc1, 0, [0; 3], &wide, 0, None)
        .unwrap();
    let log = log_from(&dev, from);
    find(&log, "copy_texture");
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

#[test]
fn a_read_only_depth_view_keeps_views_of_its_aspect_bound_and_its_draws_write_nothing_there() {
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
    let target = dev
        .create_texture(&tex2d(16, 16, RGBA8, bind::RENDER_TARGET), &[])
        .unwrap();
    let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
    // R24G8_TYPELESS viewed as D24_UNORM_S8_UINT (45) for depth and R24_UNORM_X8_TYPELESS (46)
    // to read it.
    let depth = dev
        .create_texture(
            &tex2d(16, 16, 44, bind::DEPTH_STENCIL | bind::SHADER_RESOURCE),
            &[],
        )
        .unwrap();
    let view = |format, read_only| ViewDesc {
        format,
        dimension: ViewDim::Texture2D,
        first_mip: 0,
        mip_count: 1,
        first_layer: 0,
        layer_count: 1,
        read_only,
    };
    let srv = dev
        .create_view(&depth, Some(&view(46, 0)), bind::SHADER_RESOURCE)
        .unwrap();
    let srv = Rc::new(ShaderView::Texture(srv));
    for (read_only, kept) in [
        (0, false),
        (DSV_READ_ONLY_DEPTH, true),
        (DSV_READ_ONLY_STENCIL, false),
    ] {
        let dsv = dev
            .create_view(&depth, Some(&view(45, read_only)), bind::DEPTH_STENCIL)
            .unwrap();
        dev.om_set_render_targets(&[Some(rtv.clone())], None)
            .unwrap();
        dev.set_shader_resources(PIXEL, 0, &[Some(srv.clone())])
            .unwrap();
        dev.om_set_render_targets(&[Some(rtv.clone())], Some(dsv))
            .unwrap();
        assert_eq!(
            dev.shader_resource_bound(PIXEL, 0),
            kept,
            "flags {read_only}"
        );
    }
    // The default state writes depth; through the depth-only read-only view it draws with a twin
    // writing none, in a pass whose depth is only read.
    let dsv = dev
        .create_view(
            &depth,
            Some(&view(45, DSV_READ_ONLY_DEPTH)),
            bind::DEPTH_STENCIL,
        )
        .unwrap();
    dev.om_set_render_targets(&[Some(rtv)], Some(dsv)).unwrap();
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    assert!(
        log.iter()
            .any(|l| l.starts_with("create_depth_stencil") && l.contains("write false")),
        "{log:#?}"
    );
    assert!(
        log.iter().any(|l| l.starts_with("begin_pass")
            && l.contains(" depth read-only")
            && !l.contains("stencil read-only")),
        "{log:#?}"
    );
    // Feature level 11_0 is needed, and only the two flags exist.
    assert!(matches!(
        dev.create_view(&depth, Some(&view(45, 4)), bind::DEPTH_STENCIL),
        Err(Error::InvalidArg(_))
    ));
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
    // R1_UNORM and SAMPLER_FEEDBACK_MIN_MIP_OPAQUE are valid; 0 and 500 are no format.
    for unserved in [66, 189] {
        assert!(matches!(
            dev.create_texture(&tex2d(4, 4, unserved, bind::SHADER_RESOURCE), &[]),
            Err(Error::NotImplemented(_))
        ));
    }
    // R32G32B32_TYPELESS and _FLOAT are stored as the back end's three-channel format, which the
    // recording back end holds.
    for served in [5, 6] {
        assert!(
            dev.create_texture(&tex2d(4, 4, served, bind::SHADER_RESOURCE), &[])
                .is_ok()
        );
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
    // The vertex program reads SV_InstanceID: its helper block's set is made once as well.
    assert_eq!(
        sets(&mut dev, Some(&cb)),
        2,
        "the first draw makes the stage's set and the helper block's"
    );
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
        read_only: 0,
    };
    // R24G8_TYPELESS: D24_UNORM_S8_UINT for depth, R24_UNORM_X8_TYPELESS and X24_TYPELESS_G8_UINT
    // to read depth and stencil; a D format is for depth-stencil views only. Stencil is read
    // through a stencil view, moved to the second component by the programs reading it.
    let depth = dev
        .create_texture(
            &tex2d(4, 4, 44, bind::DEPTH_STENCIL | bind::SHADER_RESOURCE),
            &[],
        )
        .unwrap();
    assert_eq!(depth.format, wcontainers_gpu::Format::Depth24PlusStencil8);
    dev.create_view(&depth, Some(&view(45)), bind::DEPTH_STENCIL)
        .unwrap();
    let from = dev.backend.log.len();
    dev.create_view(&depth, Some(&view(46)), bind::SHADER_RESOURCE)
        .unwrap();
    let made = log_from(&dev, from).join("\n");
    assert!(!made.ends_with(" stencil"), "{made}");
    let from = dev.backend.log.len();
    let stencil = dev
        .create_view(&depth, Some(&view(47)), bind::SHADER_RESOURCE)
        .unwrap();
    assert!(stencil.reads_stencil());
    let made = log_from(&dev, from).join("\n");
    assert!(made.ends_with(" stencil"), "{made}");
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
    // R8G8B8A8_TYPELESS: UNORM and UNORM_SRGB views of the texture itself; UINT views read and
    // draw the bits through a twin texture of their format.
    let colour = dev
        .create_texture(
            &tex2d(4, 4, 27, bind::RENDER_TARGET | bind::SHADER_RESOURCE),
            &[],
        )
        .unwrap();
    let srgb = dev
        .create_view(&colour, Some(&view(29)), bind::RENDER_TARGET)
        .unwrap();
    assert!(srgb.twin.is_none());
    dev.create_view(&colour, Some(&view(28)), bind::SHADER_RESOURCE)
        .unwrap();
    let uint = dev
        .create_view(&colour, Some(&view(30)), bind::SHADER_RESOURCE)
        .unwrap();
    assert!(uint.twin.is_some());
    assert_eq!(uint.format, wcontainers_gpu::Format::Rgba8Uint);
    let target = dev
        .create_view(&colour, Some(&view(30)), bind::RENDER_TARGET)
        .unwrap();
    assert!(target.twin.is_some());
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

/// The f32 at `at` of the uniform block a binding set made in `log` binds at `binding`.
fn helper_f32(dev: &Dev, log: &[String], binding: u32, at: usize) -> Option<f32> {
    let key = format!(" {binding}=uniform ");
    let line = log
        .iter()
        .find(|l| l.starts_with("create_binding_set") && l.contains(&key))?;
    let range = line.split(&key).nth(1)?.split(' ').next()?;
    let (buffer, rest) = range.split_once('+')?;
    let offset: usize = rest.split_once(':')?.0.parse().ok()?;
    let bytes = dev.backend.buffer_bytes(buffer.parse().ok()?)?;
    let word = bytes[offset + at..offset + at + 4].try_into().ok()?;
    Some(f32::from_le_bytes(word))
}

#[test]
fn a_sampler_with_a_level_of_detail_bias_is_read_through_a_program_that_adds_it() {
    let mut dev = device();
    sampling(&mut dev, "samplers.ps_sample");
    let biased = SamplerDesc {
        mip_lod_bias: 1.5,
        ..SamplerDesc::default_state()
    };
    let biased = dev.create_sampler_state(&biased).unwrap();
    dev.set_samplers(PIXEL, 0, &[Some(biased)]).unwrap();
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    // The pixel shader is converted again, once, with the bias read from its helper block.
    let shaders = log
        .iter()
        .filter(|l| l.starts_with("create_shader"))
        .count();
    assert_eq!(shaders, 1, "{log:#?}");
    let fragment = wcontainers_gpu::helper_binding(wcontainers_gpu::ShaderStage::Fragment);
    assert_eq!(helper_f32(&dev, &log, fragment, 0), Some(1.5), "{log:#?}");
    let sets = log
        .iter()
        .filter(|l| l.contains(&format!(" {fragment}=uniform ")))
        .count();
    assert_eq!(sets, 1, "{log:#?}");
    assert!(dev.backend.slow_paths().count(SlowPath::LodBiasInShader) >= 2);

    // Without a bias the plain program draws again, with no helper block.
    dev.set_samplers(PIXEL, 0, &[None]).unwrap();
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    let log = log_from(&dev, from);
    assert!(
        !log.iter().any(|l| l.starts_with("create_shader")),
        "{log:#?}"
    );
    // The helper set now holds the vertex program's bases alone.
    let set = &log[find(&log, "create_binding_set")];
    assert!(!set.contains(&format!(" {fragment}=uniform ")), "{set}");
}

/// The words of the helper block bound at `binding` in the first binding set of `log` holding one.
fn helper_words(dev: &Dev, log: &[String], binding: u32) -> Vec<u32> {
    let key = format!(" {binding}=uniform ");
    let line = log
        .iter()
        .find(|l| l.starts_with("create_binding_set") && l.contains(&key))
        .unwrap_or_else(|| panic!("no helper block in {log:#?}"));
    let range = line.split(&key).nth(1).unwrap().split(' ').next().unwrap();
    let (buffer, rest) = range.split_once('+').unwrap();
    let (offset, size) = rest.split_once(':').unwrap();
    let (offset, size): (usize, usize) = (offset.parse().unwrap(), size.parse().unwrap());
    let bytes = dev.backend.buffer_bytes(buffer.parse().unwrap()).unwrap();
    bytes[offset..offset + size]
        .chunks(4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
        .collect()
}

#[test]
fn border_and_mirror_once_addressing_the_back_end_lacks_is_applied_in_the_program() {
    let mut dev = device();
    sampling(&mut dev, "samplers.ps_sample");
    let fragment = wcontainers_gpu::helper_binding(wcontainers_gpu::ShaderStage::Fragment);
    // Border in u, mirror-once in v, clamp in w: the program filters through the sampler, which
    // is made clamping, reading its state, its border colour and the view's components.
    let border = SamplerDesc {
        address: [4, 5, 3],
        border: [0.25, 0.5, 0.75, 1.0],
        ..SamplerDesc::default_state()
    };
    let from = dev.backend.log.len();
    let border = dev.create_sampler_state(&border).unwrap();
    dev.set_samplers(PIXEL, 0, &[Some(border)]).unwrap();
    dev.draw(3, 0);
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    let made = &log[find(&log, "create_sampler")];
    assert!(made.contains("[Clamp, Clamp, Clamp]"), "{made}");
    let count = |what: &str| log.iter().filter(|l| l.starts_with(what)).count();
    assert_eq!(count("create_shader"), 1, "{log:#?}");
    let words = helper_words(&dev, &log, fragment);
    let state = [1, 1, 1, 3, 4, 2, 1, 0, 32f32.to_bits()];
    assert!(words.windows(9).any(|w| w == state), "{words:?}");
    let colour = [0.25f32, 0.5, 0.75, 1.0].map(f32::to_bits);
    assert!(words.windows(4).any(|w| w == colour), "{words:?}");
    assert!(words.contains(&1), "R32_FLOAT has red alone: {words:?}");
    assert!(dev.backend.slow_paths().count(SlowPath::BorderInShader) >= 2);

    // Mirror-once alone samples natively through the clamping sampler, the program mirroring.
    let mirror = SamplerDesc {
        address: [5, 1, 1],
        ..SamplerDesc::default_state()
    };
    let mirror = dev.create_sampler_state(&mirror).unwrap();
    dev.set_samplers(PIXEL, 0, &[Some(mirror)]).unwrap();
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    assert_eq!(
        log.iter()
            .filter(|l| l.starts_with("create_shader"))
            .count(),
        1,
        "{log:#?}"
    );
    let words = helper_words(&dev, &log, fragment);
    let state = [1, 1, 1, 4, 0, 0, 1, 0, 32f32.to_bits()];
    assert!(words.windows(9).any(|w| w == state), "{words:?}");
    assert_eq!(
        dev.backend.slow_paths().count(SlowPath::MirrorOnceInShader),
        1
    );
}

#[test]
fn a_texture_under_a_minimum_level_clamp_is_sampled_through_a_merged_sampler() {
    let mut dev = device();
    sampling(&mut dev, "samplers.ps_sample");
    let desc = TextureDesc {
        mip_levels: 4,
        misc_flags: MISC_RESOURCE_CLAMP,
        ..tex2d(16, 16, RGBA8, bind::SHADER_RESOURCE)
    };
    let texture = dev.create_texture(&desc, &[]).unwrap();
    let view = dev
        .create_view(&texture, None, bind::SHADER_RESOURCE)
        .unwrap();
    dev.set_shader_resources(PIXEL, 0, &[Some(Rc::new(ShaderView::Texture(view)))])
        .unwrap();
    texture.min_lod.set(1.5);
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    // The pixel shader is converted again once, and Direct3D's default sampler is made again once
    // with its minimum level raised to the clamp, bound beside the helper block holding it.
    let count = |what: &str| log.iter().filter(|l| l.starts_with(what)).count();
    assert_eq!(count("create_shader"), 1, "{log:#?}");
    let merged: Vec<&String> = log
        .iter()
        .filter(|l| l.starts_with("create_sampler") && l.contains("lod 1.5.."))
        .collect();
    assert_eq!(merged.len(), 1, "{log:#?}");
    let id = merged[0].split(' ').nth(1).unwrap();
    let binding = format!(" {}=sampler {id}", wcontainers_gpu::CLAMPED_SAMPLER_BINDING);
    let sets = log.iter().filter(|l| l.contains(&binding)).count();
    assert_eq!(sets, 1, "{log:#?}");
    let fragment = wcontainers_gpu::helper_binding(wcontainers_gpu::ShaderStage::Fragment);
    assert_eq!(helper_f32(&dev, &log, fragment, 0), Some(1.5), "{log:#?}");
    assert!(dev.backend.slow_paths().count(SlowPath::MinLodClamp) >= 2);

    // A clamp at the view's first level clamps nothing: the plain program draws again.
    texture.min_lod.set(0.0);
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    let log = log_from(&dev, from);
    assert!(
        !log.iter().any(|l| l.starts_with("create_shader")),
        "{log:#?}"
    );
    assert!(!log.iter().any(|l| l.contains(&binding)), "{log:#?}");
}

#[test]
fn dual_source_blending_draws_the_programs_second_output_where_the_back_end_blends_it() {
    let mut dev = device();
    dev.backend.caps.features |= wcontainers_gpu::Features::DUAL_SOURCE_BLEND;
    ready(&mut dev, "blend.vs", "blend.ps_dual", &[], 12);
    let mut blend = BlendDesc::default_state();
    blend.targets[0].enable = true;
    // D3D11_BLEND_SRC1_COLOR, D3D11_BLEND_INV_SRC1_COLOR.
    blend.targets[0].src = 16;
    blend.targets[0].dst = 17;
    let blend = dev.create_blend_state(&blend).unwrap();
    dev.om_set_blend_state(Some(blend), None, u32::MAX);
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    find(&log, "create_shader");
    find(&log, "draw ");
}

#[test]
fn a_texture_the_back_end_cannot_filter_is_filtered_in_the_program() {
    let mut dev = device();
    let uses = &mut dev.backend.caps.formats.uses[wcontainers_gpu::Format::R32Float.index()];
    uses.remove(wcontainers_gpu::FormatUse::FILTER);
    sampling(&mut dev, "samplers.ps_sample");
    let draws = |dev: &Dev| {
        dev.backend
            .log
            .iter()
            .filter(|l| l.starts_with("draw "))
            .count()
    };
    // Point sampling needs no filtering.
    let point = SamplerDesc {
        filter: 0,
        ..SamplerDesc::default_state()
    };
    let point = dev.create_sampler_state(&point).unwrap();
    dev.set_samplers(PIXEL, 0, &[Some(point)]).unwrap();
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    assert_eq!(draws(&dev), 1);
    assert_eq!(dev.backend.slow_paths().count(SlowPath::FilterInShader), 0);
    // An empty slot reads Direct3D's default sampler, which filters linearly: the program
    // filters from loads, reading the default sampler's state.
    dev.set_samplers(PIXEL, 0, &[None]).unwrap();
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    assert_eq!(draws(&dev), 2);
    let log = log_from(&dev, from);
    let fragment = wcontainers_gpu::helper_binding(wcontainers_gpu::ShaderStage::Fragment);
    let words = helper_words(&dev, &log, fragment);
    let state = [1, 1, 1, 2, 2, 2, 1, 0, 32f32.to_bits()];
    assert!(words.windows(9).any(|w| w == state), "{words:?}");
    assert_eq!(dev.backend.slow_paths().count(SlowPath::FilterInShader), 1);
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
    // The target's pending clear runs first, in a pass of its own.
    dev.flush();
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
fn a_depth_bias_clamp_the_back_end_lacks_is_added_by_the_pixel_program() {
    const D24S8: u32 = 45;
    let mut dev = device();
    let target = dev
        .create_texture(&tex2d(16, 16, RGBA8, bind::RENDER_TARGET), &[])
        .unwrap();
    let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
    let depth = |dev: &mut Dev, format| {
        let t = dev
            .create_texture(&tex2d(16, 16, format, bind::DEPTH_STENCIL), &[])
            .unwrap();
        dev.create_view(&t, None, bind::DEPTH_STENCIL).unwrap()
    };
    let d24 = depth(&mut dev, D24S8);
    begin(&mut dev, &rtv, Some(&d24), 16.0);
    let v = dev
        .create_shader(&corpus("common.vs_full"), SourceStage::Vertex)
        .unwrap();
    let p = dev
        .create_shader(&corpus("constants.ps_window"), SourceStage::Pixel)
        .unwrap();
    dev.vs_set_shader(Some(v));
    dev.ps_set_shader(Some(p));
    dev.ia_set_primitive_topology(topology::TRIANGLELIST);
    let biased = RasterizerDesc {
        depth_bias: 1000,
        slope_scaled_depth_bias: 2.0,
        depth_bias_clamp: 0.001,
        ..RasterizerDesc::default_state()
    };
    let from = dev.backend.log.len();
    let biased = dev.create_rasterizer_state(&biased).unwrap();
    dev.rs_set_state(Some(biased));
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    // The draw takes a twin of the raster block without the bias; the program adds it from its
    // helper block.
    let rasters: Vec<&String> = log
        .iter()
        .filter(|l| l.starts_with("create_raster"))
        .collect();
    assert_eq!(rasters.len(), 2, "{rasters:?}");
    // The state's own block leaves out the clamp the back end lacks.
    assert!(rasters[0].ends_with("bias 1000 2 0"), "{}", rasters[0]);
    assert!(rasters[1].ends_with("bias 0 0 0"), "{}", rasters[1]);
    let count = |p: &str| log.iter().filter(|l| l.starts_with(p)).count();
    assert_eq!(count("create_shader"), 1, "{log:#?}");
    let fragment = wcontainers_gpu::helper_binding(wcontainers_gpu::ShaderStage::Fragment);
    let words = helper_words(&dev, &log, fragment);
    let values = [1000.0f32, 2.0, 0.001, 1.0 / 16_777_215.0].map(f32::to_bits);
    assert!(words.windows(4).any(|w| w == values), "{words:?}");
    assert_eq!(
        dev.backend.slow_paths().count(SlowPath::DepthBiasInShader),
        1
    );
    // Float depth's step varies with the depth: refused by name.
    let d32 = depth(&mut dev, D32F);
    dev.om_set_render_targets(&[Some(rtv.clone())], Some(d32))
        .unwrap();
    dev.draw(3, 0);
    assert!(dev.removed.is_some());
}

#[test]
fn a_constant_window_shorter_than_its_block_is_read_through_a_copy_zero_past_it() {
    let mut dev = device();
    let target = dev
        .create_texture(&tex2d(16, 16, RGBA8, bind::RENDER_TARGET), &[])
        .unwrap();
    let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
    begin(&mut dev, &rtv, None, 16.0);
    let v = dev
        .create_shader(&corpus("common.vs_full"), SourceStage::Vertex)
        .unwrap();
    // A block of 64 constants (1024 bytes).
    let p = dev
        .create_shader(&corpus("constants.ps_window"), SourceStage::Pixel)
        .unwrap();
    dev.vs_set_shader(Some(v));
    dev.ps_set_shader(Some(p));
    dev.ia_set_primitive_topology(topology::TRIANGLELIST);
    let big = BufferDesc {
        byte_width: 4096 * 16,
        usage: usage::DEFAULT,
        bind_flags: bind::CONSTANT_BUFFER,
        ..Default::default()
    };
    let big = dev.create_buffer(&big, Some(&[7; 4096 * 16])).unwrap();
    let id = |line: &str| line.split(' ').nth(1).unwrap().to_owned();
    // A whole block's window binds the buffer itself.
    let from = dev.backend.log.len();
    dev.set_constant_ranges(PIXEL, 0, &[Some(big.clone())], &[Some((256, 64))])
        .unwrap();
    dev.draw(3, 0);
    let log = log_from(&dev, from);
    assert!(
        !log.iter().any(|l| l.starts_with("copy_buffer")),
        "{log:#?}"
    );
    // The last 16 constants: copied into a buffer of the block's size, zero past them, once while
    // the buffer is not written.
    let from = dev.backend.log.len();
    dev.set_constant_ranges(PIXEL, 0, &[Some(big.clone())], &[Some((4080, 16))])
        .unwrap();
    dev.draw(3, 0);
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    let made = &log[find(&log, "create_buffer")];
    assert!(made.contains("size 1024 "), "{made}");
    let copy = id(made);
    let count = |log: &[String], p: &str| log.iter().filter(|l| l.starts_with(p)).count();
    assert!(
        log.iter()
            .any(|l| l.starts_with(&format!("clear_buffer {copy} at 0 1024 bytes"))),
        "{log:#?}"
    );
    let copied = format!("+{} to {copy}+0 256 bytes", 4080 * 16);
    assert_eq!(
        log.iter().filter(|l| l.ends_with(&copied)).count(),
        1,
        "{log:#?}"
    );
    assert!(
        log.iter().any(|l| l.starts_with("create_binding_set")
            && l.contains(&format!("=uniform {copy}+0:1024"))),
        "{log:#?}"
    );
    assert_eq!(count(&log, "draw "), 2, "{log:#?}");
    // Written, it is copied again before the next draw, into the same copy.
    let from = dev.backend.log.len();
    let r = Resource::Buffer(big.clone());
    dev.update_subresource(&r, 0, None, &[9; 4096 * 16], 0, 0)
        .unwrap();
    dev.draw(3, 0);
    let log = log_from(&dev, from);
    assert_eq!(
        log.iter().filter(|l| l.ends_with(&copied)).count(),
        1,
        "{log:#?}"
    );
    assert_eq!(count(&log, "create_buffer"), 0, "{log:#?}");
    assert_eq!(count(&log, "create_binding_set"), 0, "{log:#?}");
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

#[test]
fn a_sample_mask_draws_through_a_raster_block_made_with_it_once() {
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
    let target = dev
        .create_texture(
            &TextureDesc {
                sample_count: 4,
                ..tex2d(16, 16, RGBA8, bind::RENDER_TARGET)
            },
            &[],
        )
        .unwrap();
    let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
    dev.om_set_render_targets(&[Some(rtv)], None).unwrap();
    let from = dev.backend.log.len();
    let rasters = |dev: &Dev, from: usize| -> Vec<String> {
        log_from(dev, from)
            .iter()
            .filter(|l| l.starts_with("draw "))
            .map(|l| {
                l.split(" raster ")
                    .nth(1)
                    .unwrap()
                    .split(' ')
                    .next()
                    .unwrap()
                    .to_string()
            })
            .collect()
    };
    dev.draw(3, 0);
    dev.om_set_blend_state(None, None, 0b0101);
    dev.draw(3, 0);
    dev.draw(3, 0);
    // Bits past the target's samples change nothing.
    dev.om_set_blend_state(None, None, 0xffff_fff5);
    dev.draw(3, 0);
    dev.om_set_blend_state(None, None, u32::MAX);
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let made: Vec<_> = log_from(&dev, from)
        .into_iter()
        .filter(|l| l.starts_with("create_raster"))
        .collect();
    assert_eq!(made.len(), 1, "{made:?}");
    assert!(made[0].contains("mask 0x5 "), "{}", made[0]);
    let masked = made[0].split(' ').nth(1).unwrap().to_string();
    let drawn = rasters(&dev, from);
    assert_eq!(drawn.len(), 5);
    assert_ne!(drawn[0], masked);
    assert_eq!(&drawn[1..4], [masked.clone(), masked.clone(), masked]);
    assert_eq!(drawn[4], drawn[0]);
}

/// One of the shaders crate's own compiled test shaders.
fn own(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../wcontainers-shaders/tests/own/{name}.dxbc",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn a_geometry_shader_runs_as_compute_before_the_draw_of_its_output() {
    let mut dev = device();
    let target = dev
        .create_texture(&tex2d(16, 16, RGBA8, bind::RENDER_TARGET), &[])
        .unwrap();
    let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
    begin(&mut dev, &rtv, None, 16.0);
    let (vsb, gsb, psb) = (own("vs_geometry"), own("gs_quads"), own("ps_geometry"));
    let vs = dev.create_shader(&vsb, SourceStage::Vertex).unwrap();
    let ps = dev.create_shader(&psb, SourceStage::Pixel).unwrap();
    // A geometry shader's bytecode made as a vertex shader, and the reverse, are refused.
    assert!(matches!(
        dev.create_geometry_shader(&vsb, None),
        Err(Error::InvalidArg(_))
    ));
    let gs = dev.create_geometry_shader(&gsb, None).unwrap();
    let mut colour = element("COLOR", RGBA32F, 12);
    colour.semantic_index = 0;
    let layout = dev
        .create_input_layout(&[element("POSITION", RGB32F, 0), colour], &vsb)
        .unwrap();
    let vertices = immutable(&mut dev, &[0u8; 28 * 3], bind::VERTEX_BUFFER);
    let constants = immutable(&mut dev, &[0u8; 32], bind::CONSTANT_BUFFER);
    dev.vs_set_shader(Some(vs));
    dev.ps_set_shader(Some(ps));
    dev.gs_set_shader(Some(gs));
    dev.ia_set_input_layout(Some(layout));
    dev.ia_set_vertex_buffers(0, &[Some((vertices, 28, 0))])
        .unwrap();
    dev.set_constant_buffers(GEOMETRY, 0, &[Some(constants)])
        .unwrap();
    dev.ia_set_primitive_topology(topology::POINTLIST);
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    // The vertex stage's and the geometry stage's dispatches, then the draw of four vertices of
    // a strip per point, by restarting indices.
    let vertex = find(&log, "dispatch ");
    let geometry = vertex + 1 + find(&log[vertex + 1..], "dispatch ");
    let draw = find(&log, "draw ");
    assert!(vertex < geometry && geometry < draw, "{log:#?}");
    assert!(log[vertex].ends_with("groups [1, 1, 1]"), "{}", log[vertex]);
    assert!(
        log[draw].contains("TriangleStrip restart true"),
        "{}",
        log[draw]
    );
    assert!(log[draw].contains("count: 24"), "{}", log[draw]);
    assert_eq!(
        dev.backend.slow_paths().count(SlowPath::GeometryAsCompute),
        1
    );
    // Without a geometry shader the same draw is drawn as points.
    dev.gs_set_shader(None);
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    let log = log_from(&dev, from);
    assert!(!log.iter().any(|l| l.starts_with("dispatch ")));
    assert!(log[find(&log, "draw ")].contains("PointList"));
}

#[test]
fn hull_and_domain_shaders_run_as_compute_before_the_domain_programs_draw() {
    let mut dev = device();
    let target = dev
        .create_texture(&tex2d(16, 16, RGBA8, bind::RENDER_TARGET), &[])
        .unwrap();
    let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
    begin(&mut dev, &rtv, None, 16.0);
    let vsb = corpus("tessellation.vs");
    let vs = dev.create_shader(&vsb, SourceStage::Vertex).unwrap();
    let ps = dev
        .create_shader(&corpus("tessellation.ps"), SourceStage::Pixel)
        .unwrap();
    // A hull shader's bytecode made as a domain shader is refused.
    assert!(matches!(
        dev.create_shader(&corpus("tessellation.hs_tri"), SourceStage::Domain),
        Err(Error::InvalidArg(_))
    ));
    let hs = dev
        .create_shader(&corpus("tessellation.hs_tri"), SourceStage::Hull)
        .unwrap();
    let ds = dev
        .create_shader(&corpus("tessellation.ds_tri"), SourceStage::Domain)
        .unwrap();
    let mut colour = element("COLOR", RGBA32F, 12);
    colour.semantic_index = 0;
    let layout = dev
        .create_input_layout(&[element("POSITION", RGB32F, 0), colour], &vsb)
        .unwrap();
    let vertices = immutable(&mut dev, &[0u8; 28 * 3], bind::VERTEX_BUFFER);
    let constants = immutable(&mut dev, &[0u8; 32], bind::CONSTANT_BUFFER);
    dev.vs_set_shader(Some(vs));
    dev.ps_set_shader(Some(ps));
    dev.ia_set_input_layout(Some(layout));
    dev.ia_set_vertex_buffers(0, &[Some((vertices, 28, 0))])
        .unwrap();
    for stage in [HULL, DOMAIN] {
        dev.set_constant_buffers(stage, 0, &[Some(constants.clone())])
            .unwrap();
    }
    dev.hs_set_shader(Some(hs));
    // Without a domain shader, or from a topology other than patches, Direct3D draws nothing.
    dev.ia_set_primitive_topology(topology::PATCHLIST_1 + 2);
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    dev.ds_set_shader(Some(ds));
    dev.ia_set_primitive_topology(topology::TRIANGLELIST);
    dev.draw(3, 0);
    let log = log_from(&dev, from);
    assert!(
        !log.iter()
            .any(|l| l.starts_with("dispatch ") || l.starts_with("draw ")),
        "{log:#?}"
    );
    dev.ia_set_primitive_topology(topology::PATCHLIST_1 + 2);
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    assert!(dev.removed.is_none(), "{:?}", dev.removed);
    let log = log_from(&dev, from);
    // The vertex stage's dispatch, then the hull stage's and the tessellator's, then the domain
    // program's indirect draw.
    let vertex = find(&log, "dispatch ");
    let draw = find(&log, "draw ");
    assert!(
        log[vertex + 1..draw]
            .iter()
            .any(|l| l.starts_with("dispatch ")),
        "{log:#?}"
    );
    assert!(log[draw].contains("IndexedIndirect"), "{}", log[draw]);
    assert_eq!(
        dev.backend
            .slow_paths()
            .count(SlowPath::TessellationAsCompute),
        1
    );
    // Without the hull and domain shaders a triangle list is drawn as usual.
    dev.hs_set_shader(None);
    dev.ds_set_shader(None);
    dev.ia_set_primitive_topology(topology::TRIANGLELIST);
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    let log = log_from(&dev, from);
    assert!(!log.iter().any(|l| l.starts_with("dispatch ")));
    assert!(log[find(&log, "draw ")].contains("TriangleList"));
}

#[test]
fn stream_output_writes_its_targets_and_draw_auto_draws_what_they_hold() {
    use wcontainers_direct3d_11::stream::DeclEntry;
    let mut dev = device();
    let vsb = own("vs_geometry");
    let vs = dev.create_shader(&vsb, SourceStage::Vertex).unwrap();
    let entry = DeclEntry {
        stream: 0,
        semantic: Some("SV_Position".into()),
        semantic_index: 0,
        start_component: 0,
        component_count: 4,
        slot: 0,
    };
    let so = dev
        .create_geometry_shader(&vsb, Some((Some(&[entry]), None, u32::MAX)))
        .unwrap();
    let mut colour = element("COLOR", RGBA32F, 12);
    colour.semantic_index = 0;
    let layout = dev
        .create_input_layout(&[element("POSITION", RGB32F, 0), colour], &vsb)
        .unwrap();
    let vertices = immutable(&mut dev, &[0u8; 28 * 3], bind::VERTEX_BUFFER);
    let target = dev
        .create_buffer(
            &BufferDesc {
                byte_width: 256,
                usage: usage::DEFAULT,
                bind_flags: bind::STREAM_OUTPUT | bind::VERTEX_BUFFER,
                ..Default::default()
            },
            None,
        )
        .unwrap();
    // A buffer made without the stream output binding is refused as a target.
    assert!(dev.so_set_targets(&[Some(vertices.clone())], &[0]).is_err());
    dev.vs_set_shader(Some(vs));
    dev.gs_set_shader(Some(so));
    dev.ia_set_input_layout(Some(layout));
    dev.ia_set_vertex_buffers(0, &[Some((vertices, 28, 0))])
        .unwrap();
    dev.ia_set_primitive_topology(topology::TRIANGLELIST);
    dev.so_set_targets(&[Some(target.clone())], &[0]).unwrap();
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    // The vertex stage's dispatch and the stream output program's; nothing is drawn, as no
    // stream is rasterized.
    assert_eq!(
        log.iter().filter(|l| l.starts_with("dispatch ")).count(),
        2,
        "{log:#?}"
    );
    assert!(!log.iter().any(|l| l.starts_with("draw ")));
    assert_eq!(
        dev.backend.slow_paths().count(SlowPath::StreamOutAsCompute),
        1
    );
    // DrawAuto reads the filled size back; the recording back end runs no program, so the target
    // holds what SOSetTargets set: nothing to draw.
    dev.gs_set_shader(None);
    dev.so_set_targets(&[], &[]).unwrap();
    dev.ia_set_vertex_buffers(0, &[Some((target, 16, 0))])
        .unwrap();
    let from = dev.backend.log.len();
    dev.draw_auto();
    assert!(dev.removed.is_none());
    assert!(!log_from(&dev, from).iter().any(|l| l.starts_with("draw ")));
    assert_eq!(dev.backend.slow_paths().count(SlowPath::ReadBackWait), 1);
}

#[test]
fn a_view_reinterpreting_typeless_bits_reads_a_twin_copied_before_each_draw() {
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
    // R16G16_TYPELESS stored as FLOAT, read as UNORM.
    let typeless = dev
        .create_texture(&tex2d(4, 4, 33, bind::SHADER_RESOURCE), &[])
        .unwrap();
    let desc = ViewDesc {
        format: 35,
        dimension: ViewDim::Texture2D,
        first_mip: 0,
        mip_count: 1,
        first_layer: 0,
        layer_count: 1,
        read_only: 0,
    };
    let unorm = dev
        .create_view(&typeless, Some(&desc), bind::SHADER_RESOURCE)
        .unwrap();
    assert_eq!(unorm.format, wcontainers_gpu::Format::Rg16Unorm);
    dev.set_shader_resources(PIXEL, 0, &[Some(Rc::new(ShaderView::Texture(unorm)))])
        .unwrap();
    for _ in 0..2 {
        let from = dev.backend.log.len();
        dev.draw(3, 0);
        let log = log_from(&dev, from);
        assert!(
            find(&log, "copy_texture ") < find(&log, "draw "),
            "{log:#?}"
        );
    }
    assert_eq!(dev.backend.slow_paths().count(SlowPath::ReinterpretCopy), 2);
    assert!(dev.removed.is_none());
}

#[test]
fn targets_blended_differently_without_the_back_ends_draw_once_per_group_from_saved_depth() {
    let mut dev = device();
    dev.backend.caps.features -= wcontainers_gpu::Features::INDEPENDENT_BLEND;
    let elements = [
        element("POSITION", RGB32F, 0),
        element("COLOR", RGBA32F, 12),
    ];
    ready(&mut dev, "common.vs_color", "targets.ps", &elements, 28);
    let mut views = Vec::new();
    for _ in 0..3 {
        let t = dev
            .create_texture(&tex2d(16, 16, RGBA8, bind::RENDER_TARGET), &[])
            .unwrap();
        views.push(Some(
            dev.create_view(&t, None, bind::RENDER_TARGET).unwrap(),
        ));
    }
    let depth = dev
        .create_texture(&tex2d(16, 16, D32F, bind::DEPTH_STENCIL), &[])
        .unwrap();
    let dsv = dev.create_view(&depth, None, bind::DEPTH_STENCIL).unwrap();
    dev.om_set_render_targets(&views, Some(dsv)).unwrap();
    let ds = dev
        .create_depth_stencil_state(&DepthStencilDesc {
            depth_enable: true,
            depth_write: true,
            depth_func: 2,
            ..DepthStencilDesc::default_state()
        })
        .unwrap();
    dev.om_set_depth_stencil_state(Some(ds), 0);
    // Targets 0 and 2 blend alike, target 1 adds; target 3 is unbound.
    let mut blend = BlendDesc::default_state();
    blend.independent = true;
    for t in [0, 2] {
        blend.targets[t].enable = true;
        blend.targets[t].src = 5;
        blend.targets[t].dst = 6;
    }
    blend.targets[1].enable = true;
    blend.targets[1].dst = 2;
    let from = dev.backend.log.len();
    let blend = dev.create_blend_state(&blend).unwrap();
    let log = log_from(&dev, from);
    assert!(
        log.iter()
            .all(|l| !l.starts_with("create_blend") || l.ends_with("same true")),
        "{log:#?}"
    );
    dev.om_set_blend_state(Some(blend), None, u32::MAX);
    let from = dev.backend.log.len();
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
    let log = log_from(&dev, from);
    // The runs' passes (the earlier target's pending clear runs as a pass of its own).
    let run = |l: &String| l.starts_with("begin_pass") && l.contains(" depth view ");
    let passes: Vec<_> = log.iter().filter(|l| run(l)).collect();
    let draws = log.iter().filter(|l| l.starts_with("draw ")).count();
    assert_eq!((passes.len(), draws), (2, 2), "{log:#?}");
    // Each run draws only its group's targets.
    assert!(
        passes[0].contains(" color 0 ") && passes[0].contains(" color 2 "),
        "{passes:#?}"
    );
    assert!(!passes[0].contains(" color 1 "), "{passes:#?}");
    assert!(
        passes[1].contains(" color 1 ") && !passes[1].contains(" color 0 "),
        "{passes:#?}"
    );
    // Depth saved before the first run and put back before the second.
    let copies: Vec<_> = log
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("copy_texture "))
        .map(|(i, _)| i)
        .collect();
    let begins: Vec<_> = log
        .iter()
        .enumerate()
        .filter(|(_, l)| run(l))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(copies.len(), 2, "{log:#?}");
    assert!(copies[0] < begins[0] && begins[0] < copies[1] && copies[1] < begins[1]);
    assert_eq!(dev.backend.slow_paths().count(SlowPath::TargetsSplit), 1);
}
