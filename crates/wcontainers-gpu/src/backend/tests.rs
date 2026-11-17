//! A trivial back end that only records base calls, to show the fast-path defaults are built from them.

use super::*;
use crate::caps::{FormatCaps, FormatUse, Limits};
use crate::draw::Viewport;
use alloc::format;
use alloc::string::String;
use alloc::vec;

#[derive(Debug, Default)]
struct Recorder {
    caps: Option<Caps>,
    slow: SlowPaths,
    next: u32,
    log: Vec<String>,
    /// Bytes written into each buffer, by id.
    buffers: Vec<Vec<u8>>,
    /// Read backs already "finished" by the fake GPU.
    ready_after_wait: bool,
}

type Id = u32;

impl Recorder {
    fn new() -> Self {
        let limits = Limits {
            max_texture_1d: 8192,
            max_texture_2d: 8192,
            max_texture_3d: 2048,
            max_texture_layers: 256,
            max_buffer_size: 1 << 28,
            max_uniform_buffer_size: 1 << 16,
            max_storage_buffer_size: 1 << 27,
            uniform_buffers_per_stage: 12,
            textures_per_stage: 16,
            samplers_per_stage: 16,
            storage_buffers_per_stage: 8,
            storage_textures_per_stage: 4,
            vertex_streams: 8,
            vertex_attributes: 16,
            max_vertex_stride: 2048,
            color_targets: 8,
            inter_stage_slots: 16,
            sample_counts: 0b1001,
            min_uniform_offset_alignment: 256,
            min_storage_offset_alignment: 256,
            immediate_bytes: 64,
            compute_workgroup_storage: 16384,
            compute_invocations: 256,
            compute_size: [256, 256, 64],
            max_dispatch: 65535,
            max_point_size: 1.0,
            max_line_width: 1.0,
            max_anisotropy: 16,
            capture_buffers: 0,
        };
        Recorder {
            caps: Some(Caps {
                features: Features::empty(),
                limits,
                formats: FormatCaps([FormatUse::all(); Format::COUNT]),
            }),
            ..Default::default()
        }
    }

    fn id(&mut self) -> Id {
        self.next += 1;
        self.next
    }
}

impl Backend for Recorder {
    type Buffer = Id;
    type Texture = Id;
    type TextureView = Id;
    type Sampler = Id;
    type Shader = Id;
    type Program = Id;
    type Blend = Id;
    type DepthStencil = Id;
    type Raster = Id;
    type VertexLayout = Id;
    type BindingSet = Id;
    type QuerySet = Id;
    type Readback = Id;
    type Fence = Id;
    type ResourceTable = Id;
    type Surface = Id;

    fn caps(&self) -> &Caps {
        self.caps.as_ref().unwrap()
    }
    fn slow_paths(&mut self) -> &mut SlowPaths {
        &mut self.slow
    }
    fn status(&mut self) -> DeviceStatus {
        DeviceStatus::Ok
    }
    fn create_buffer(&mut self, desc: &BufferDesc) -> Result<Id, GpuError> {
        let id = self.id();
        self.log
            .push(format!("create_buffer {id} size {}", desc.size));
        self.buffers.resize(id as usize + 1, Vec::new());
        self.buffers[id as usize] = vec![0; desc.size as usize];
        Ok(id)
    }
    fn create_texture(&mut self, _: &TextureDesc) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn create_view(&mut self, _: &Id, _: &TextureViewDesc) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn create_sampler(&mut self, _: &SamplerDesc) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn create_shader(&mut self, _: &ShaderModule) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn create_program(&mut self, _: &Program<'_, Self>) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn create_blend(&mut self, _: &BlendDesc) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn create_depth_stencil(&mut self, _: &DepthStencilDesc) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn create_raster(&mut self, _: &RasterDesc) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn create_vertex_layout(&mut self, _: &VertexLayoutDesc) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn create_binding_set(&mut self, _: &[BindingEntry<'_, Self>]) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn create_query_set(&mut self, _: QueryKind, _: u32) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn destroy(&mut self, object: Object<Self>) {
        if let Object::Buffer(id) = object {
            self.log.push(format!("destroy_buffer {id}"));
        }
    }
    fn write_buffer(&mut self, buffer: &Id, offset: u64, data: &[u8], mode: WriteMode) {
        self.log.push(format!(
            "write_buffer {buffer} {} bytes {mode:?}",
            data.len()
        ));
        let b = &mut self.buffers[*buffer as usize];
        b[offset as usize..offset as usize + data.len()].copy_from_slice(data);
    }
    fn write_texture(&mut self, _: &TextureRegion<'_, Self>, _: Extent, _: &[u8], _: u32, _: u32) {}
    fn begin_pass(&mut self, pass: &PassDesc<'_, Self>) {
        let c = pass.colors[0].as_ref().map(|c| c.load);
        let d = pass.depth.as_ref().map(|d| (d.depth_load, d.stencil_load));
        self.log.push(format!("begin_pass color {c:?} depth {d:?}"));
    }
    fn end_pass(&mut self) {
        self.log.push("end_pass".into());
    }
    fn draw(&mut self, call: &DrawCall<'_, Self>) {
        let index = call.index.as_ref().map(|i| *i.buffer);
        self.log.push(format!(
            "draw {:?} index {index:?} {:?}",
            call.topology, call.range
        ));
    }
    fn begin_query(&mut self, _: u32) {}
    fn end_query(&mut self) {}
    fn copy_buffer(&mut self, _: &Id, _: u64, _: &Id, _: u64, _: u64) {}
    fn copy_texture(
        &mut self,
        _: &TextureRegion<'_, Self>,
        _: &TextureRegion<'_, Self>,
        _: Extent,
    ) {
    }
    fn copy_buffer_to_texture(
        &mut self,
        _: &CopyBufferTexture<'_, Self>,
        _: &TextureRegion<'_, Self>,
        _: Extent,
    ) {
    }
    fn copy_texture_to_buffer(
        &mut self,
        _: &TextureRegion<'_, Self>,
        _: &CopyBufferTexture<'_, Self>,
        _: Extent,
    ) {
    }
    fn clear_buffer(&mut self, _: &Id, _: u64, _: u64) {}
    fn resolve(&mut self, _: &TextureRegion<'_, Self>, _: &TextureRegion<'_, Self>) {}
    fn submit(&mut self) -> Id {
        self.id()
    }
    fn fence_done(&mut self, _: &Id) -> bool {
        true
    }
    fn wait_fence(&mut self, _: &Id) {}
    fn read_back(&mut self, src: &ReadSource<'_, Self>) -> Result<Id, GpuError> {
        let ReadSource::Buffer { buffer, .. } = src else {
            return Err(GpuError::Invalid("buffers only"));
        };
        self.log.push(format!("read_back {buffer}"));
        Ok(**buffer)
    }
    fn read_queries(&mut self, _: &Id, _: u32, _: u32) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn read_state(&mut self, _: &Id) -> ReadState {
        if self.ready_after_wait {
            ReadState::Ready
        } else {
            ReadState::Pending
        }
    }
    fn wait_read(&mut self, rb: &Id) {
        self.log.push(format!("wait_read {rb}"));
        self.ready_after_wait = true;
    }
    fn finish_read(&mut self, rb: Id, dst: &mut [u8]) {
        self.log.push(format!("finish_read {rb}"));
        dst.copy_from_slice(&self.buffers[rb as usize][..dst.len()]);
    }
    fn create_surface(&mut self, _: u32) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn configure_surface(
        &mut self,
        _: &Id,
        _: Format,
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<(), GpuError> {
        Ok(())
    }
    fn current_texture(&mut self, _: &Id) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn present(&mut self, _: &Id) -> Result<(), GpuError> {
        Ok(())
    }
}

/// A draw call naming made-up blocks; the recorder never looks inside them.
fn call<'a>(viewports: &'a [Viewport]) -> DrawCall<'a, Recorder> {
    const BLOCK: Id = 100;
    DrawCall {
        program: &BLOCK,
        blend: &BLOCK,
        depth_stencil: &BLOCK,
        raster: &BLOCK,
        vertex_layout: &BLOCK,
        bindings: [None; crate::MAX_BINDING_SETS],
        vertex_buffers: &[],
        index: None,
        topology: Topology::TriangleFan,
        restart: true,
        viewports,
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: 0,
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: 0,
            first_instance: 0,
            instances: 1,
        },
    }
}

fn indices(r: &Recorder, id: Id) -> Vec<u32> {
    r.buffers[id as usize]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect()
}

#[test]
fn fan_default_draws_a_generated_list() {
    let mut r = Recorder::new();
    let vp = [];
    r.draw_fan_or_loop(
        &call(&vp),
        FanOrLoop::TriangleFan,
        IndexSource::Sequential {
            first: 10,
            count: 5,
        },
        0,
        1,
    )
    .unwrap();
    assert_eq!(indices(&r, 1), [10, 11, 12, 10, 12, 13, 10, 13, 14]);
    assert_eq!(
        r.log,
        [
            "create_buffer 1 size 36",
            "write_buffer 1 36 bytes Discard",
            "draw TriangleList index Some(1) Indexed { first: 0, count: 9, base_vertex: 0, first_instance: 0, instances: 1 }",
            "destroy_buffer 1",
        ]
    );
    assert_eq!(r.slow_paths().count(SlowPath::FanConverted), 1);
    assert_eq!(r.slow_paths().take_warnings(), [SlowPath::FanConverted]);
}

#[test]
fn loop_default_closes_each_piece_and_honours_restart() {
    let mut r = Recorder::new();
    let guest: Vec<u8> = [4u16, 5, 6, 0xffff, 7, 8]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let src = IndexSource::Guest {
        indices: &guest,
        format: IndexFormat::U16,
        base_vertex: 3,
    };
    r.draw_fan_or_loop(&call(&[]), FanOrLoop::LineLoop, src, 2, 4)
        .unwrap();
    assert_eq!(indices(&r, 1), [4, 5, 6, 4, u32::MAX, 7, 8, 7]);
    assert!(r.log[2].starts_with("draw LineStrip index Some(1) Indexed { first: 0, count: 8, base_vertex: 3, first_instance: 2, instances: 4 }"));
}

#[test]
fn fan_restart_starts_a_new_fan() {
    let mut r = Recorder::new();
    let guest: Vec<u8> = [0u32, 1, 2, 3, u32::MAX, 9, 8, 7]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let src = IndexSource::Guest {
        indices: &guest,
        format: IndexFormat::U32,
        base_vertex: 0,
    };
    r.draw_fan_or_loop(&call(&[]), FanOrLoop::TriangleFan, src, 0, 1)
        .unwrap();
    assert_eq!(indices(&r, 1), [0, 1, 2, 0, 2, 3, 9, 8, 7]);
}

#[test]
fn read_back_now_default_waits_and_counts() {
    let mut r = Recorder::new();
    let b = r
        .create_buffer(&BufferDesc {
            size: 4,
            usage: BufferUsage::READ_BACK,
            intent: MapIntent::Rarely,
        })
        .unwrap();
    r.write_buffer(&b, 0, &[1, 2, 3, 4], WriteMode::Ordered);
    let mut out = [0u8; 4];
    r.read_back_now(
        &ReadSource::Buffer {
            buffer: &b,
            offset: 0,
            size: 4,
        },
        &mut out,
    )
    .unwrap();
    assert_eq!(out, [1, 2, 3, 4]);
    assert_eq!(&r.log[2..], ["read_back 1", "wait_read 1", "finish_read 1"]);
    assert_eq!(r.slow_paths().count(SlowPath::ReadBackWait), 1);
}

#[test]
fn clear_texture_default_is_a_clearing_pass() {
    let mut r = Recorder::new();
    r.clear_texture(
        &7,
        Format::Depth24PlusStencil8,
        ClearValue::DepthStencil {
            depth: Some(0),
            stencil: None,
        },
    );
    assert_eq!(
        r.log,
        [
            "begin_pass color None depth Some((Clear(0), Load))",
            "end_pass"
        ]
    );
}

#[test]
fn absent_features_say_which() {
    let mut r = Recorder::new();
    assert_eq!(
        r.create_resource_table(10).unwrap_err(),
        GpuError::Absent(Features::RESOURCE_TABLES)
    );
}
