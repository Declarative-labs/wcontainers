//! The cache on the recording back end: a run records what it converts and draws, and the next
//! run's first device builds the recorded combination before any draw, which its first draw then
//! uses. Its own test binary, since the cache is the process's.

use std::io::Write;
use std::sync::{Arc, Mutex};

use wcontainers_direct3d_11::desc::*;
use wcontainers_direct3d_11::record::Recorder;
use wcontainers_direct3d_11::{Device, cache};
use wcontainers_gpu::SourceStage;
use wcontainers_shaders::cache::Cache;

type Dev = Device<Recorder>;

/// A file the test reads back: the bytes appended.
#[derive(Clone, Default)]
struct File(Arc<Mutex<Vec<u8>>>);

impl Write for File {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn corpus(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../wcontainers-graphics-test/shaders/dxbc/{name}.dxbc",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
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

/// One frame of a triangle: what a program does each run.
fn frame(dev: &mut Dev) {
    let target = dev
        .create_texture(
            &TextureDesc {
                width: 16,
                height: 16,
                depth: 1,
                mip_levels: 1,
                array_size: 1,
                format: 28,
                sample_count: 1,
                usage: usage::DEFAULT,
                bind_flags: bind::RENDER_TARGET,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
    let rtv = dev.create_view(&target, None, bind::RENDER_TARGET).unwrap();
    dev.om_set_render_targets(&[Some(rtv)], None).unwrap();
    dev.rs_set_viewports(&[Viewport {
        width: 16.0,
        height: 16.0,
        max_depth: 1.0,
        ..Default::default()
    }]);
    let (vsb, psb) = (corpus("common.vs_color"), corpus("common.ps_color"));
    let vs = dev.create_shader(&vsb, SourceStage::Vertex).unwrap();
    let ps = dev.create_shader(&psb, SourceStage::Pixel).unwrap();
    let layout = dev
        .create_input_layout(&[element("POSITION", 6, 0), element("COLOR", 2, 12)], &vsb)
        .unwrap();
    dev.vs_set_shader(Some(vs));
    dev.ps_set_shader(Some(ps));
    dev.ia_set_input_layout(Some(layout));
    let vertices = dev
        .create_buffer(
            &BufferDesc {
                byte_width: 84,
                usage: usage::IMMUTABLE,
                bind_flags: bind::VERTEX_BUFFER,
                ..Default::default()
            },
            Some(&[0; 84]),
        )
        .unwrap();
    dev.ia_set_vertex_buffers(0, &[Some((vertices, 28, 0))])
        .unwrap();
    dev.ia_set_primitive_topology(topology::TRIANGLELIST);
    dev.draw(3, 0);
    assert!(dev.removed.is_none());
}

fn device() -> Dev {
    Device::new(Recorder::new(None), &[level::L11_0]).unwrap()
}

#[test]
fn a_run_builds_at_its_start_what_the_last_run_drew() {
    // First run: an empty file gains the two shaders and the combination drawn.
    let file = File::default();
    let empty = Cache::default();
    file.0.lock().unwrap().extend_from_slice(empty.bytes());
    cache::open(empty, Box::new(file.clone()));
    let mut dev = device();
    assert_eq!(dev.started_use(), (0, 0));
    frame(&mut dev);
    drop(dev);
    let bytes = file.0.lock().unwrap().clone();
    let (loaded, dropped) = Cache::load(bytes).unwrap();
    assert_eq!((loaded.len(), dropped), (3, 0));

    // Next run: the first device prepares it before any draw, and the first draw takes it.
    let file = File::default();
    cache::open(loaded, Box::new(file.clone()));
    let mut dev = device();
    let prepared: Vec<String> = dev
        .backend
        .log
        .iter()
        .filter(|l| l.starts_with("prepare "))
        .cloned()
        .collect();
    assert_eq!(prepared.len(), 1, "{:#?}", dev.backend.log);
    assert_eq!(dev.started_use(), (1, 0));
    frame(&mut dev);
    assert_eq!(dev.started_use(), (1, 1));
    let draw = dev
        .backend
        .log
        .iter()
        .find(|l| l.starts_with("draw "))
        .unwrap();
    let program = |l: &str| l.split(' ').nth(2).unwrap().to_string();
    assert_eq!(program(draw), program(&prepared[0]));
    // Nothing new was converted or drawn, so nothing was appended.
    assert!(file.0.lock().unwrap().is_empty());

    // A later device of the same process builds nothing.
    let dev = device();
    assert_eq!(dev.started_use(), (0, 0));
}
