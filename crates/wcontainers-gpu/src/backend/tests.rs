//! A trivial back end that only records base calls, to show the fast-path defaults are built from them.

use super::*;
use crate::caps::{FormatCaps, FormatUse, Limits};
use crate::draw::Viewport;
use crate::helpers::MipRead;
use crate::resource::Filter;
use crate::state::ProvokingVertex;
use alloc::format;
use alloc::string::String;
use alloc::vec;

#[derive(Debug, Default)]
struct Recorder {
    caps: Option<Caps>,
    slow: SlowPaths,
    helpers: Helpers<Recorder>,
    next: u32,
    log: Vec<String>,
    /// Bytes written into each buffer, by id.
    buffers: Vec<Vec<u8>>,
    /// Read backs already "finished" by the fake GPU.
    ready_after_wait: bool,
    /// Each sampler's minifying filter, by id.
    samplers: Vec<(Id, Filter)>,
    /// The samplers the last binding set holds.
    bound_samplers: Vec<Id>,
    /// Each buffer's usage, by id.
    usages: Vec<(Id, BufferUsage)>,
    /// The format of each view made, in order.
    view_formats: Vec<Format>,
}

type Id = u32;

impl Recorder {
    fn new() -> Self {
        let limits = Limits {
            max_texture_2d: 8192,
            max_texture_cube: 8192,
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
            color_bytes_per_sample: 32,
            inter_stage_slots: 16,
            min_uniform_offset_alignment: 256,
            min_storage_offset_alignment: 256,
            buffer_copy_alignment: 4,
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
                adapter: crate::caps::AdapterInfo::default(),
                features: Features::empty(),
                limits,
                formats: FormatCaps {
                    uses: [FormatUse::all(); Format::COUNT],
                    samples: [0b101; Format::COUNT],
                },
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
    fn helpers(&mut self) -> &mut Helpers<Self> {
        &mut self.helpers
    }
    fn create_buffer(&mut self, desc: &BufferDesc) -> Result<Id, GpuError> {
        let id = self.id();
        self.log
            .push(format!("create_buffer {id} size {}", desc.size));
        self.usages.push((id, desc.usage));
        self.buffers.resize(id as usize + 1, Vec::new());
        self.buffers[id as usize] = vec![0; desc.size as usize];
        Ok(id)
    }
    fn create_texture(&mut self, _: &TextureDesc) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn create_view(&mut self, t: &Id, d: &TextureViewDesc) -> Result<Id, GpuError> {
        let id = self.id();
        self.view_formats.push(d.format);
        self.log.push(format!(
            "create_view {id} of {t} {:?} mip {}+{} layer {}+{}",
            d.dimension, d.base_mip, d.mip_count, d.base_layer, d.layer_count
        ));
        Ok(id)
    }
    fn create_sampler(&mut self, desc: &SamplerDesc) -> Result<Id, GpuError> {
        let id = self.id();
        self.samplers.push((id, desc.min));
        Ok(id)
    }
    fn create_shader(&mut self, _: &ShaderModule) -> Result<Id, GpuError> {
        Ok(self.id())
    }
    fn create_program(&mut self, _: &Program<'_, Self>) -> Result<Id, GpuError> {
        let id = self.id();
        self.log.push(format!("create_program {id}"));
        Ok(id)
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
    fn create_binding_set(&mut self, entries: &[BindingEntry<'_, Self>]) -> Result<Id, GpuError> {
        self.bound_samplers = entries
            .iter()
            .filter_map(|e| match e.resource {
                BindingResource::Sampler(s) => Some(*s),
                _ => None,
            })
            .collect();
        Ok(self.id())
    }
    fn binding_layout_key(&self, _: &Id) -> BindingLayoutKey {
        BindingLayoutKey::default()
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
    fn write_texture(
        &mut self,
        _: &TextureRegion<'_, Self>,
        _: Extent,
        _: &[u8],
        _: u32,
        _: u32,
        _: WriteMode,
    ) {
    }
    fn begin_pass(&mut self, pass: &PassDesc<'_, Self>) {
        let c = pass.colors[0]
            .as_ref()
            .map(|c| (*c.view, c.depth_slice, c.load));
        let d = pass
            .depth
            .as_ref()
            .map(|d| (d.depth.map(|a| a.load), d.stencil.map(|a| a.load)));
        self.log.push(format!("begin_pass color {c:?} depth {d:?}"));
    }
    fn end_pass(&mut self) {
        self.log.push("end_pass".into());
    }
    fn draw(&mut self, call: &DrawCall<'_, Self>) {
        let index = call.index.as_ref().map(|i| *i.buffer);
        let scissor = call.scissors.first().map(|r| (r.x, r.y, r.width, r.height));
        self.log.push(format!(
            "draw {:?} index {index:?} {:?} scissor {scissor:?} stencil {}",
            call.topology, call.range, call.stencil_reference[0]
        ));
    }
    fn begin_query(&mut self, _: u32) {}
    fn end_query(&mut self) {}
    fn copy_buffer(&mut self, _: &Id, _: u64, _: &Id, _: u64, _: u64) {}
    fn copy_texture(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        e: Extent,
    ) {
        self.log.push(format!(
            "copy_texture {} mip {} to {} mip {}, {}x{}x{}",
            src.texture, src.mip, dst.texture, dst.mip, e.width, e.height, e.depth_or_layers
        ));
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
    fn clear_buffer(&mut self, _: &Id, _: u64, _: u64, _: &[u8]) {}
    fn resolve(&mut self, _: &TextureRegion<'_, Self>, _: &TextureRegion<'_, Self>, _: Format) {}
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
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: 0,
            first_instance: 0,
            instances: 1,
        },
    }
}

/// The indices written into the ring so far.
fn indices(r: &Recorder, id: Id) -> Vec<u32> {
    let head = r.helpers.rings[RingKind::Indices as usize]
        .as_ref()
        .unwrap()
        .head as usize;
    r.buffers[id as usize][..head]
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
        ProvokingVertex::First,
        0,
        1,
    )
    .unwrap();
    // Each triangle keeps the fan's winding and starts with vertex i + 1, OpenGL's first-vertex rule.
    assert_eq!(indices(&r, 1), [11, 12, 10, 12, 13, 10, 13, 14, 10]);
    assert_eq!(
        r.log,
        [
            "create_buffer 1 size 4194304",
            "write_buffer 1 36 bytes NoOverwrite",
            "draw TriangleList index Some(1) Indexed { first: 0, count: 9, base_vertex: 0, first_instance: 0, instances: 1 } scissor None stencil 0",
        ],
        "the list goes into the ring, so no buffer is made per draw"
    );
    r.draw_fan_or_loop(
        &call(&vp),
        FanOrLoop::TriangleFan,
        IndexSource::Sequential { first: 0, count: 3 },
        ProvokingVertex::First,
        0,
        1,
    )
    .unwrap();
    assert_eq!(lines(&r, "create_buffer").len(), 1);
    assert_eq!(
        &indices(&r, 1)[9..12],
        [1, 2, 0],
        "the next list follows in the ring"
    );
    assert_eq!(r.slow_paths().count(SlowPath::FanConverted), 2);
    assert_eq!(
        r.slow_paths().take_warnings(),
        [SlowPath::FanConverted],
        "one warning however often"
    );
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
    r.draw_fan_or_loop(
        &call(&[]),
        FanOrLoop::LineLoop,
        src,
        ProvokingVertex::First,
        2,
        4,
    )
    .unwrap();
    assert_eq!(indices(&r, 1), [4, 5, 5, 6, 6, 4, 7, 8, 8, 7]);
    assert!(r.log[2].starts_with("draw LineList index Some(1) Indexed { first: 0, count: 10, base_vertex: 3, first_instance: 2, instances: 4 }"));
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
    r.draw_fan_or_loop(
        &call(&[]),
        FanOrLoop::TriangleFan,
        src,
        ProvokingVertex::Last,
        0,
        1,
    )
    .unwrap();
    // The last-vertex rule keeps the centre first, so vertex i + 2 ends each triangle.
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

fn texture(dimension: TextureDimension, format: Format, layers: u32, mips: u32) -> TextureDesc {
    TextureDesc {
        dimension,
        format,
        width: 8,
        height: 8,
        depth_or_layers: layers,
        mip_levels: mips,
        samples: 1,
        usage: crate::TextureUsage::SAMPLE | crate::TextureUsage::RENDER,
        reinterpret: false,
        cube: false,
    }
}

fn lines<'a>(r: &'a Recorder, start: &str) -> Vec<&'a str> {
    r.log
        .iter()
        .filter(|l| l.starts_with(start))
        .map(|l| l.as_str())
        .collect()
}

#[test]
fn clear_texture_default_is_a_clearing_pass() {
    let mut r = Recorder::new();
    let format = Format::Depth24PlusStencil8;
    let desc = texture(TextureDimension::D2, format, 1, 1);
    let value = ClearValue::DepthStencil {
        depth: Some(0),
        stencil: None,
    };
    r.clear_texture(&7, &desc, format, 0..1, 0..1, value)
        .unwrap();
    assert_eq!(
        r.log,
        [
            "create_view 1 of 7 D2 mip 0+1 layer 0+1",
            "begin_pass color None depth Some((Some(Clear(0)), Some(Load)))",
            "end_pass"
        ]
    );
}

#[test]
fn clear_texture_of_a_volume_clears_every_slice_of_each_level() {
    let mut r = Recorder::new();
    let desc = texture(TextureDimension::D3, Format::Rgba8Unorm, 4, 2);
    r.clear_texture(
        &7,
        &desc,
        desc.format,
        0..2,
        0..4,
        ClearValue::Color([0; 4]),
    )
    .unwrap();
    assert_eq!(
        lines(&r, "create_view"),
        [
            "create_view 1 of 7 D3 mip 0+1 layer 0+1",
            "create_view 2 of 7 D3 mip 1+1 layer 0+1"
        ]
    );
    let passes = lines(&r, "begin_pass");
    assert_eq!(
        passes.len(),
        4 + 2,
        "four slices at level 0, two at level 1"
    );
    for (pass, slice) in passes.iter().zip([0, 1, 2, 3, 0, 1]) {
        assert!(pass.contains(&format!(", {slice}, Clear")), "{pass}");
    }
}

#[test]
fn clear_texture_of_layers_attaches_one_layer_per_pass() {
    let mut r = Recorder::new();
    let desc = texture(TextureDimension::D2, Format::Rgba8Unorm, 6, 1);
    r.clear_texture(
        &7,
        &desc,
        desc.format,
        0..1,
        1..4,
        ClearValue::Color([0; 4]),
    )
    .unwrap();
    assert_eq!(
        lines(&r, "create_view"),
        [
            "create_view 1 of 7 D2 mip 0+1 layer 1+1",
            "create_view 2 of 7 D2 mip 0+1 layer 2+1",
            "create_view 3 of 7 D2 mip 0+1 layer 3+1"
        ]
    );
    assert_eq!(lines(&r, "begin_pass").len(), 3);
}

#[test]
fn loop_with_restart_off_keeps_the_largest_index_as_a_vertex() {
    let mut r = Recorder::new();
    let guest: Vec<u8> = [0u32, 1, u32::MAX, 2]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let src = IndexSource::Guest {
        indices: &guest,
        format: IndexFormat::U32,
        base_vertex: 0,
    };
    let mut c = call(&[]);
    c.restart = false;
    r.draw_fan_or_loop(&c, FanOrLoop::LineLoop, src, ProvokingVertex::First, 0, 1)
        .unwrap();
    assert_eq!(
        indices(&r, 1),
        [0, 1, 1, u32::MAX, u32::MAX, 2, 2, 0],
        "a line list never restarts, so the largest value is an ordinary vertex"
    );
}

#[test]
fn clear_of_a_stencil_format_names_no_depth_aspect() {
    let mut r = Recorder::new();
    let desc = texture(TextureDimension::D2, Format::Stencil8, 1, 1);
    let value = ClearValue::DepthStencil {
        depth: Some(0),
        stencil: Some(3),
    };
    r.clear_texture(&7, &desc, desc.format, 0..1, 0..1, value)
        .unwrap();
    assert_eq!(
        r.log[1],
        "begin_pass color None depth Some((None, Some(Clear(3))))"
    );
}

fn read_f32(r: &Recorder, buffer: Id, at: usize) -> f32 {
    f32::from_le_bytes(r.buffers[buffer as usize][at..at + 4].try_into().unwrap())
}

#[test]
fn ring_aligns_parts_pads_tails_and_wraps_with_discard() {
    let mut r = Recorder::new();
    let mut offsets = [0; 2];
    let ring = r
        .write_buffer_ring(
            RingKind::Data,
            &[&[1, 2, 3, 4, 5], &[6; 8]],
            256,
            &mut offsets,
        )
        .unwrap();
    assert_eq!(offsets, [0, 256]);
    assert_eq!(&r.buffers[ring as usize][..6], [1, 2, 3, 4, 5, 0]);
    assert_eq!(&r.buffers[ring as usize][256..264], [6; 8]);
    assert_eq!(
        r.log,
        [
            "create_buffer 1 size 4194304",
            "write_buffer 1 4 bytes NoOverwrite",
            "write_buffer 1 4 bytes NoOverwrite",
            "write_buffer 1 8 bytes NoOverwrite",
        ]
    );
    // Too little room left: the ring starts again at zero, dropping its old contents by renaming.
    let big = vec![9u8; RING_SIZE as usize - 256];
    let mut one = [0];
    r.write_buffer_ring(RingKind::Data, &[&big], 256, &mut one)
        .unwrap();
    assert_eq!(one, [0]);
    assert_eq!(
        r.log.last().unwrap(),
        "write_buffer 1 4194048 bytes Discard"
    );
    // Larger than the ring: a larger ring replaces it.
    let huge = vec![1u8; RING_SIZE as usize + 4];
    let ring = r
        .write_buffer_ring(RingKind::Data, &[&huge], 4, &mut one)
        .unwrap();
    assert_eq!(ring, 2);
    assert!(r.log.contains(&"destroy_buffer 1".into()));
    assert!(r.log.contains(&"create_buffer 2 size 8388608".into()));
}

#[test]
fn ring_gives_empty_parts_an_aligned_offset() {
    let mut r = Recorder::new();
    let mut o = [0];
    r.write_buffer_ring(RingKind::Data, &[&[0u8; 145]], 256, &mut o)
        .unwrap();
    assert_eq!(o, [0]);
    r.write_buffer_ring(RingKind::Data, &[&[]], 256, &mut o)
        .unwrap();
    assert_eq!(o, [256]);
    let writes = r
        .log
        .iter()
        .filter(|l| l.starts_with("write_buffer"))
        .count();
    r.write_buffer_ring(RingKind::Data, &[&[], &[]], 64, &mut [0, 0])
        .unwrap();
    assert_eq!(
        r.log
            .iter()
            .filter(|l| l.starts_with("write_buffer"))
            .count(),
        writes
    );
}

#[test]
fn own_view_follows_cube_and_layers() {
    let mut d = TextureDesc {
        dimension: TextureDimension::D2,
        format: Format::Rgba8Unorm,
        width: 4,
        height: 4,
        depth_or_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage: crate::TextureUsage::SAMPLE,
        reinterpret: false,
        cube: false,
    };
    assert_eq!(d.own_view(), ViewDimension::D2);
    d.depth_or_layers = 6;
    assert_eq!(d.own_view(), ViewDimension::D2Array);
    d.cube = true;
    assert_eq!(d.own_view(), ViewDimension::Cube);
    d.depth_or_layers = 12;
    assert_eq!(d.own_view(), ViewDimension::CubeArray);
    d.dimension = TextureDimension::D3;
    assert_eq!(d.own_view(), ViewDimension::D3);
}

#[test]
fn generate_mips_draws_each_level_and_layer_from_the_one_above() {
    let mut r = Recorder::new();
    let desc = TextureDesc {
        dimension: TextureDimension::D2,
        format: Format::Rgba8UnormSrgb,
        width: 8,
        height: 8,
        depth_or_layers: 2,
        mip_levels: 3,
        samples: 1,
        usage: crate::TextureUsage::SAMPLE | crate::TextureUsage::RENDER,
        reinterpret: false,
        cube: false,
    };
    r.generate_mips(&50, &desc, desc.format, 1..3, 0..2)
        .unwrap();
    let views: Vec<_> = lines(&r, "create_view")
        .iter()
        .map(|l| l.split(" of ").nth(1).unwrap())
        .collect();
    assert_eq!(
        views,
        [
            "50 D2Array mip 0+1 layer 0+2",
            "50 D2 mip 1+1 layer 0+1",
            "50 D2 mip 1+1 layer 1+1",
            "50 D2Array mip 1+1 layer 0+2",
            "50 D2 mip 2+1 layer 0+1",
            "50 D2 mip 2+1 layer 1+1",
        ],
        "the level above is read through a whole-array view, never a single-layer one"
    );
    let scissors: Vec<_> = r
        .log
        .iter()
        .filter(|l| l.starts_with("draw"))
        .map(|l| String::from(l.split("scissor ").nth(1).unwrap()))
        .collect();
    assert_eq!(
        scissors,
        [
            "Some((0, 0, 4, 4)) stencil 0",
            "Some((0, 0, 4, 4)) stencil 0",
            "Some((0, 0, 2, 2)) stencil 0",
            "Some((0, 0, 2, 2)) stencil 0",
        ]
    );
}

#[test]
fn generate_mips_of_a_volume_draws_each_slice_at_its_centre() {
    let mut r = Recorder::new();
    let desc = TextureDesc {
        dimension: TextureDimension::D3,
        format: Format::Rgba16Float,
        width: 8,
        height: 8,
        depth_or_layers: 4,
        mip_levels: 2,
        samples: 1,
        usage: crate::TextureUsage::SAMPLE | crate::TextureUsage::RENDER,
        reinterpret: false,
        cube: false,
    };
    r.generate_mips(&50, &desc, desc.format, 1..2, 0..1)
        .unwrap();
    let passes: Vec<_> = r
        .log
        .iter()
        .filter(|l| l.starts_with("begin_pass"))
        .collect();
    assert_eq!(passes.len(), 2);
    assert!(passes[0].contains(", 0, DontCare"));
    assert!(passes[1].contains(", 1, DontCare"));
    let ring = r.helpers.rings[RingKind::Data as usize]
        .as_ref()
        .unwrap()
        .buffer;
    assert_eq!(read_f32(&r, ring, 16), 0.25);
    assert_eq!(read_f32(&r, ring, 256 + 16), 0.75);
}

#[test]
fn generate_mips_of_a_cube_reads_whole_cube_views_and_names_the_face() {
    let mut r = Recorder::new();
    let mut desc = texture(TextureDimension::D2, Format::Rgba8Unorm, 6, 2);
    desc.cube = true;
    r.generate_mips(&50, &desc, desc.format, 1..2, 0..6)
        .unwrap();
    let views: Vec<_> = lines(&r, "create_view")
        .iter()
        .map(|l| l.split(" of ").nth(1).unwrap())
        .collect();
    assert_eq!(views[0], "50 Cube mip 0+1 layer 0+6");
    assert!(views[1..].iter().all(|v| v.starts_with("50 D2 mip 1+1")));
    assert_eq!(views.len(), 7);
    let ring = r.helpers.rings[RingKind::Data as usize]
        .as_ref()
        .unwrap()
        .buffer;
    let face = |n: usize| {
        let at = n * 256 + 24;
        u32::from_le_bytes(r.buffers[ring as usize][at..at + 4].try_into().unwrap())
    };
    assert_eq!((0..6).map(face).collect::<Vec<_>>(), [0, 1, 2, 3, 4, 5]);
}

#[test]
fn defaults_build_only_the_built_in_programs_they_draw_with() {
    let mut r = Recorder::new();
    let desc = texture(TextureDimension::D2, Format::Rgba8Unorm, 1, 3);
    r.generate_mips(&50, &desc, desc.format, 1..3, 0..1)
        .unwrap();
    r.generate_mips(&50, &desc, desc.format, 1..3, 0..1)
        .unwrap();
    assert_eq!(
        lines(&r, "create_program").len(),
        1,
        "the 2D mip program, once"
    );
}

#[test]
fn clear_rects_draws_one_scissored_triangle_per_rectangle() {
    let mut r = Recorder::new();
    let rects = [
        ScissorRect {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        },
        ScissorRect {
            x: 10,
            y: 0,
            width: 5,
            height: 5,
        },
    ];
    r.clear_rects(
        &7,
        Format::Rgba8Uint,
        ClearValue::Color([1, 2, 3, 4]),
        &rects,
    )
    .unwrap();
    let passes: Vec<_> = r
        .log
        .iter()
        .filter(|l| !l.contains("_buffer") && !l.starts_with("create_program"))
        .collect();
    assert_eq!(passes[0], "begin_pass color Some((7, 0, Load)) depth None");
    assert!(passes[1].ends_with("scissor Some((1, 2, 3, 4)) stencil 0"));
    assert!(passes[2].ends_with("scissor Some((10, 0, 5, 5)) stencil 0"));
    assert_eq!(passes[3], "end_pass");
    let ring = r.helpers.rings[RingKind::Data as usize]
        .as_ref()
        .unwrap()
        .buffer;
    assert_eq!(
        &r.buffers[ring as usize][..16],
        [1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0]
    );

    let mut r = Recorder::new();
    let depth = ClearValue::DepthStencil {
        depth: Some(1.0f32.to_bits()),
        stencil: Some(5),
    };
    r.clear_rects(&8, Format::Depth24PlusStencil8, depth, &rects[..1])
        .unwrap();
    let lines: Vec<_> = r
        .log
        .iter()
        .filter(|l| !l.contains("_buffer") && !l.starts_with("create_program"))
        .collect();
    assert_eq!(
        lines[0],
        "begin_pass color None depth Some((Some(Load), Some(Load)))"
    );
    assert!(lines[1].ends_with("stencil 5"));
    let ring = r.helpers.rings[RingKind::Data as usize]
        .as_ref()
        .unwrap()
        .buffer;
    assert_eq!(read_f32(&r, ring, 20), 1.0);

    // An aspect the format lacks is not cleared, and nothing is drawn.
    let mut r = Recorder::new();
    let stencil = ClearValue::DepthStencil {
        depth: None,
        stencil: Some(1),
    };
    r.clear_rects(&8, Format::Depth32Float, stencil, &rects)
        .unwrap();
    assert!(r.log.is_empty());
}

#[test]
fn absent_features_say_which() {
    let mut r = Recorder::new();
    const BLOCK: Id = 100;
    let dispatch = DispatchCall {
        program: &BLOCK,
        bindings: [None; crate::MAX_BINDING_SETS],
        immediates: &[],
        groups: crate::DispatchGroups::Direct([1, 1, 1]),
    };
    assert_eq!(
        r.dispatch(&dispatch).unwrap_err(),
        GpuError::Absent(Features::COMPUTE)
    );
    assert_eq!(
        r.capture_vertices(&call(&[]), &[], false).unwrap_err(),
        GpuError::Absent(Features::TRANSFORM_FEEDBACK)
    );
    assert_eq!(
        r.create_resource_table(10).unwrap_err(),
        GpuError::Absent(Features::RESOURCE_TABLES)
    );
    assert_eq!(
        r.write_resource_table(&BLOCK, 0, &[]).unwrap_err(),
        GpuError::Absent(Features::RESOURCE_TABLES)
    );
    assert_eq!(
        r.write_timestamp(&BLOCK, 0).unwrap_err(),
        GpuError::Absent(Features::TIMESTAMPS)
    );
    assert_eq!(
        r.timestamps_disjoint().unwrap_err(),
        GpuError::Absent(Features::TIMESTAMPS)
    );
}

#[test]
fn resolve_queries_default_reads_back_and_writes_in_order() {
    let mut r = Recorder::new();
    let desc = BufferDesc {
        size: 64,
        usage: BufferUsage::COPY_DST,
        intent: MapIntent::Rarely,
    };
    let dst = r.create_buffer(&desc).unwrap();
    // The recorder's query read back answers from the buffer of the same id: made next, holding the
    // results.
    let results = r.create_buffer(&desc).unwrap();
    let rb = results + 1;
    r.buffers.resize(rb as usize + 1, Vec::new());
    r.buffers[rb as usize] = [7u64, 0, 1 << 40]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    r.resolve_queries(&99, 0, 3, &dst, 16).unwrap();
    assert_eq!(
        &r.log[2..],
        [
            "wait_read 3",
            "finish_read 3",
            "write_buffer 1 24 bytes Ordered"
        ]
    );
    assert_eq!(&r.buffers[dst as usize][16..24], 7u64.to_le_bytes());
    assert_eq!(&r.buffers[dst as usize][32..40], (1u64 << 40).to_le_bytes());
    assert_eq!(r.slow_paths().count(SlowPath::QueryResolveWait), 1);
}

#[test]
fn indices_and_other_data_never_share_a_ring() {
    let mut r = Recorder::new();
    let mut o = [0];
    let data = r
        .write_buffer_ring(RingKind::Data, &[&[1; 16]], 4, &mut o)
        .unwrap();
    r.draw_fan_or_loop(
        &call(&[]),
        FanOrLoop::TriangleFan,
        IndexSource::Sequential { first: 0, count: 4 },
        ProvokingVertex::First,
        0,
        1,
    )
    .unwrap();
    let index = r.helpers.rings[RingKind::Indices as usize]
        .as_ref()
        .unwrap()
        .buffer;
    assert_ne!(data, index);
    let usage = |id| r.usages.iter().find(|(b, _)| *b == id).unwrap().1;
    assert_eq!(
        usage(data),
        BufferUsage::VERTEX | BufferUsage::UNIFORM | BufferUsage::COPY_DST
    );
    assert_eq!(usage(index), BufferUsage::INDEX | BufferUsage::COPY_DST);
}

#[test]
fn byte_indices_convert_and_restart_at_their_largest_value() {
    let mut r = Recorder::new();
    let guest = [3u8, 4, 5, 6, 0xff, 1, 2];
    let src = IndexSource::Guest {
        indices: &guest,
        format: IndexFormat::U8,
        base_vertex: 0,
    };
    r.draw_fan_or_loop(
        &call(&[]),
        FanOrLoop::LineLoop,
        src,
        ProvokingVertex::First,
        0,
        1,
    )
    .unwrap();
    assert_eq!(indices(&r, 1), [3, 4, 4, 5, 5, 6, 6, 3, 1, 2, 2, 1]);
}

/// The built-in programs made so far.
fn made(r: &Recorder) -> Vec<Builtin> {
    r.helpers.programs.iter().map(|(w, _)| *w).collect()
}

fn generate(r: &mut Recorder, desc: &TextureDesc, values: Format) {
    r.generate_mips(&50, desc, values, 1..2, 0..desc.depth_or_layers)
        .unwrap();
}

#[test]
fn generate_mips_filters_in_the_program_where_the_format_cannot_be_filtered() {
    let mut r = Recorder::new();
    let filter = FormatUse::all() - FormatUse::FILTER;
    r.caps.as_mut().unwrap().formats.uses[Format::R32Float.index()] = filter;
    let desc = texture(TextureDimension::D2, Format::R32Float, 1, 2);
    generate(&mut r, &desc, desc.format);
    assert_eq!(r.slow_paths().count(SlowPath::FilterInShader), 1);
    assert_eq!(
        made(&r),
        [Builtin::Mip(ViewDimension::D2, MipRead::Unfiltered)]
    );
    let bound = r.bound_samplers[0];
    assert_eq!(
        r.samplers.iter().find(|s| s.0 == bound).unwrap().1,
        Filter::Nearest
    );

    let mut r = Recorder::new();
    let desc = texture(TextureDimension::D2, Format::R32Float, 1, 2);
    generate(&mut r, &desc, desc.format);
    assert_eq!(r.slow_paths().count(SlowPath::FilterInShader), 0);
    assert_eq!(
        made(&r),
        [Builtin::Mip(ViewDimension::D2, MipRead::Filtered)]
    );

    // Two layers drawn in the program: counted once per draw.
    let mut r = Recorder::new();
    r.caps.as_mut().unwrap().formats.uses[Format::R32Float.index()] = filter;
    let desc = texture(TextureDimension::D2, Format::R32Float, 2, 2);
    generate(&mut r, &desc, desc.format);
    assert_eq!(r.slow_paths().count(SlowPath::FilterInShader), 2);
}

#[test]
fn generate_mips_through_the_srgb_twin_reads_and_writes_srgb_views() {
    let mut r = Recorder::new();
    let mut desc = texture(TextureDimension::D2, Format::Rgba8Unorm, 1, 3);
    desc.reinterpret = true;
    r.generate_mips(&50, &desc, Format::Rgba8UnormSrgb, 1..3, 0..1)
        .unwrap();
    assert_eq!(r.view_formats, [Format::Rgba8UnormSrgb; 4]);
    assert_eq!(
        made(&r),
        [Builtin::Mip(ViewDimension::D2, MipRead::Filtered)]
    );
    assert_eq!(r.slow_paths().count(SlowPath::FilterInShader), 0);
    assert!(matches!(
        r.generate_mips(&50, &desc, Format::Bgra8UnormSrgb, 1..3, 0..1),
        Err(GpuError::Invalid(_))
    ));
}

#[test]
fn generate_mips_with_nothing_to_draw_builds_and_counts_nothing() {
    let mut r = Recorder::new();
    let filter = FormatUse::all() - FormatUse::FILTER;
    r.caps.as_mut().unwrap().formats.uses[Format::R32Float.index()] = filter;
    let one_level = texture(TextureDimension::D2, Format::R32Float, 1, 1);
    r.generate_mips(&50, &one_level, one_level.format, 0..1, 0..1)
        .unwrap();
    let two_levels = texture(TextureDimension::D2, Format::R32Float, 1, 2);
    r.generate_mips(&50, &two_levels, two_levels.format, 1..2, 1..1)
        .unwrap();
    assert_eq!(r.slow_paths().count(SlowPath::FilterInShader), 0);
    assert!(made(&r).is_empty());
    assert!(r.log.is_empty());
}

#[test]
fn generate_mips_of_an_integer_twin_decodes_and_encodes() {
    let mut r = Recorder::new();
    let desc = texture(TextureDimension::D2, Format::Rgba16Uint, 3, 2);
    generate(&mut r, &desc, Format::Rgba16Unorm);
    assert_eq!(
        made(&r),
        [Builtin::Mip(ViewDimension::D2Array, MipRead::Unorm)]
    );
    let ring = r.helpers.rings[RingKind::Data as usize]
        .as_ref()
        .unwrap()
        .buffer;
    let scale = u32::from_le_bytes(r.buffers[ring as usize][..4].try_into().unwrap());
    assert_eq!(scale, 65535);

    let mut r = Recorder::new();
    let desc = texture(TextureDimension::D2, Format::Rgba8Sint, 1, 2);
    generate(&mut r, &desc, Format::Rgba8Snorm);
    assert_eq!(made(&r), [Builtin::Mip(ViewDimension::D2, MipRead::Snorm)]);

    let mut r = Recorder::new();
    let desc = texture(TextureDimension::D2, Format::Rgba8Uint, 1, 2);
    assert!(matches!(
        r.generate_mips(&50, &desc, Format::Rgba16Unorm, 1..2, 0..1),
        Err(GpuError::Invalid(_))
    ));
}

#[test]
fn generate_mips_of_an_integer_cube_reads_a_2d_array() {
    // With views: a 2D array view of the cube.
    let mut r = Recorder::new();
    r.caps.as_mut().unwrap().features = Features::TEXTURE_VIEWS;
    let mut desc = texture(TextureDimension::D2, Format::R16Uint, 6, 2);
    desc.cube = true;
    generate(&mut r, &desc, Format::R16Unorm);
    assert_eq!(
        made(&r),
        [Builtin::Mip(ViewDimension::D2Array, MipRead::Unorm)]
    );
    assert!(lines(&r, "create_view")[0].ends_with(" of 50 D2Array mip 0+1 layer 0+6"));
    assert_eq!(r.slow_paths().count(SlowPath::ReinterpretCopy), 0);

    // Without: the level above copied into a 2D array texture first.
    let mut r = Recorder::new();
    generate(&mut r, &desc, Format::R16Unorm);
    assert_eq!(r.slow_paths().count(SlowPath::ReinterpretCopy), 1);
    let copy = lines(&r, "copy_texture");
    assert_eq!(copy.len(), 1);
    assert!(
        copy[0].starts_with("copy_texture 50 mip 0 to "),
        "{}",
        copy[0]
    );
    assert!(copy[0].ends_with(" mip 0, 8x8x6"), "{}", copy[0]);
    let view = lines(&r, "create_view")[0];
    assert!(
        view.contains("D2Array mip 0+1 layer 0+6") && !view.contains(" of 50 "),
        "{view}"
    );
}
