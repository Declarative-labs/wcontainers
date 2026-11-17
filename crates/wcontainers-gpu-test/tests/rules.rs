//! Each rule of the interface (plan section 4) broken once on the recording back end, which refuses
//! it: as a violation that loses the device for recording calls, as `GpuError::Invalid` or
//! `GpuError::Absent` for calls that make objects. The rules `tests/recorder.rs` breaks (attachments
//! of different sizes, depth aspects, copies and read backs inside a pass, a read finished before it
//! is ready, objects used after destroy, blocks needing absent features) and the usage-scope rule
//! (`tests/helpers.rs`) are not repeated here.

use wcontainers_gpu::*;
use wcontainers_gpu_test::{Id, Recorder};

const PROGRAM: &str = "
struct P { c: vec4<f32> }
@group(0) @binding(0) var<uniform> p: P;
@group(0) @binding(32) var t: texture_2d<f32>;
@vertex fn vs(@location(0) v: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(v, 0.5, 1.0);
}
@fragment fn plain() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0);
}
@fragment fn colored() -> @location(0) vec4<f32> {
    return p.c;
}
@fragment fn loads() -> @location(0) vec4<f32> {
    return textureLoad(t, vec2<i32>(0, 0), 0);
}
";

fn buffer(r: &mut Recorder, size: u64, usage: BufferUsage) -> Id {
    r.create_buffer(&BufferDesc {
        size,
        usage,
        intent: MapIntent::Rarely,
    })
    .unwrap()
}

fn texture(r: &mut Recorder, format: Format, size: u32, usage: TextureUsage) -> Id {
    r.create_texture(&TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width: size,
        height: size,
        depth_or_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage,
        cube: false,
        reinterpret: false,
    })
    .unwrap()
}

fn view(r: &mut Recorder, t: &Id, format: Format) -> Id {
    r.create_view(
        t,
        &TextureViewDesc {
            format,
            dimension: ViewDimension::D2,
            base_mip: 0,
            mip_count: 1,
            base_layer: 0,
            layer_count: 1,
            stencil: false,
        },
    )
    .unwrap()
}

const ALL: TextureUsage = TextureUsage::all();
const COPIES: BufferUsage = BufferUsage::COPY_SRC.union(BufferUsage::COPY_DST);

fn region(t: &Id) -> TextureRegion<'_, Recorder> {
    TextureRegion {
        texture: t,
        mip: 0,
        layer: 0,
        origin: Origin::default(),
        stencil: false,
    }
}

fn extent(n: u32) -> Extent {
    Extent {
        width: n,
        height: n,
        depth_or_layers: 1,
    }
}

/// A recorder with a 2 by 2 target, its pass open, and what a draw into it needs.
struct Drawing {
    r: Recorder,
    target: Id,
    target_view: Id,
    vertices: Id,
    layout: Id,
    blend: Id,
    depth: Id,
    raster: Id,
}

impl Drawing {
    fn new() -> Drawing {
        let mut r = Recorder::new();
        let target = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
        let target_view = view(&mut r, &target, Format::Rgba8Unorm);
        let vertices = buffer(&mut r, 24, BufferUsage::VERTEX | BufferUsage::COPY_DST);
        let corners: Vec<u8> = [-1.0f32, -1.0, 3.0, -1.0, -1.0, 3.0]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        r.write_buffer(&vertices, 0, &corners, WriteMode::Discard);
        let mut desc = VertexLayoutDesc {
            streams: [None; MAX_VERTEX_STREAMS],
            attributes: [None; MAX_VERTEX_STREAMS],
        };
        desc.streams[0] = Some(VertexStream {
            stride: 8,
            step: StepMode::Vertex,
        });
        desc.attributes[0] = Some(VertexAttribute {
            location: 0,
            stream: 0,
            offset: 0,
            format: VertexFormat::Float32x2,
        });
        let layout = r.create_vertex_layout(&desc).unwrap();
        let blend = r
            .create_blend(&BlendDesc {
                targets: [RenderTargetBlend {
                    blend: None,
                    writes: ColorWrites::all(),
                }; MAX_COLOR_TARGETS],
                same_for_all: true,
                alpha_to_coverage: false,
                logic_op: None,
            })
            .unwrap();
        let depth = r
            .create_depth_stencil(&DepthStencilDesc {
                depth_compare: None,
                depth_write: false,
                stencil: None,
                stencil_read_mask: [0xff; 2],
                stencil_write_mask: [0xff; 2],
                depth_bounds: None,
            })
            .unwrap();
        let raster = r
            .create_raster(&RasterDesc {
                fill: FillMode::Solid,
                cull: CullMode::None,
                front: FrontFace::CounterClockwise,
                depth_bias: 0,
                depth_bias_slope: 0,
                depth_bias_clamp: 0,
                depth_clip: true,
                scissor: false,
                line_width: 1.0f32.to_bits(),
                provoking: ProvokingVertex::First,
                depth_minus_one: false,
                sample_mask: !0,
            })
            .unwrap();
        Drawing {
            r,
            target,
            target_view,
            vertices,
            layout,
            blend,
            depth,
            raster,
        }
    }

    fn program(&mut self, fragment: &str) -> Id {
        let module = naga::front::wgsl::parse_str(PROGRAM).unwrap();
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap();
        let mut shader = |entry: &str, stage| {
            self.r
                .create_shader(&ShaderModule {
                    module: module.clone(),
                    info: info.clone(),
                    entry_point: entry.into(),
                    stage,
                    key: *blake3::hash(entry.as_bytes()).as_bytes(),
                })
                .unwrap()
        };
        let v = shader("vs", ShaderStage::Vertex);
        let f = shader(fragment, ShaderStage::Fragment);
        self.r
            .create_program(&Program::Render {
                vertex: &v,
                fragment: Some(&f),
            })
            .unwrap()
    }

    fn begin(&mut self) {
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view: &self.target_view,
            depth_slice: 0,
            resolve: None,
            load: LoadOp::Load,
            store: StoreOp::Store,
        });
        self.r.begin_pass(&pass);
    }

    /// Draws one triangle over the target with `program`, the binding set and topology given.
    fn draw(&mut self, program: &Id, set: Option<&Id>, topology: Topology, restart: bool) {
        let viewport = [Viewport {
            x: 0f32.to_bits(),
            y: 0f32.to_bits(),
            width: 2f32.to_bits(),
            height: 2f32.to_bits(),
            min_depth: 0f32.to_bits(),
            max_depth: 1f32.to_bits(),
        }];
        let streams = [Some(VertexBinding {
            buffer: &self.vertices,
            offset: 0,
        })];
        self.r.draw(&DrawCall {
            program,
            blend: &self.blend,
            depth_stencil: &self.depth,
            raster: &self.raster,
            vertex_layout: &self.layout,
            bindings: [set, None, None, None],
            vertex_buffers: &streams,
            index: None,
            topology,
            restart,
            viewports: &viewport,
            scissors: &[],
            blend_constant: [0; 4],
            stencil_reference: [0; 2],
            immediates: &[],
            range: DrawRange::Vertices {
                first: 0,
                count: 3,
                first_instance: 0,
                instances: 1,
            },
        });
    }

    fn refused(&mut self, violation: &str) {
        assert_eq!(self.r.violations, [violation]);
        assert_eq!(self.r.status(), DeviceStatus::Lost(LossReason::Error));
    }
}

#[test]
fn draws_happen_inside_a_pass() {
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.draw(&p, None, Topology::TriangleList, false);
    d.refused("draw outside a pass");
}

#[test]
fn passes_do_not_nest() {
    let mut d = Drawing::new();
    d.begin();
    d.begin();
    d.refused("begin_pass inside a pass");
}

#[test]
fn submits_happen_outside_a_pass() {
    let mut d = Drawing::new();
    d.begin();
    d.r.submit();
    d.refused("submit inside a pass");
}

#[test]
fn texture_copies_and_buffer_clears_happen_outside_a_pass() {
    let mut d = Drawing::new();
    let other = texture(&mut d.r, Format::Rgba8Unorm, 2, ALL);
    let b = buffer(&mut d.r, 16, COPIES);
    d.begin();
    d.r.copy_texture(&region(&other), &region(&d.target), extent(2));
    d.r.clear_buffer(&b, 0, 16, &[0]);
    assert_eq!(
        d.r.violations,
        ["copy_texture inside a pass", "clear_buffer inside a pass"]
    );
}

#[test]
fn queries_run_one_at_a_time_inside_a_pass_with_a_set() {
    let mut d = Drawing::new();
    d.r.begin_query(0);
    assert_eq!(d.r.violations, ["begin_query outside a pass"]);

    let mut d = Drawing::new();
    d.begin();
    d.r.begin_query(0);
    assert_eq!(
        d.r.violations,
        ["begin_query in a pass without an occlusion set"]
    );

    let mut d = Drawing::new();
    let set = d.r.create_query_set(QueryKind::OcclusionAny, 4).unwrap();
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: Some(&set),
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &d.target_view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Load,
        store: StoreOp::Store,
    });
    d.r.begin_pass(&pass);
    d.r.begin_query(1);
    d.r.begin_query(2);
    d.r.end_query();
    d.r.end_query();
    assert_eq!(
        d.r.violations,
        [
            "begin_query while a query runs",
            "end_query without a running query"
        ]
    );
}

#[test]
fn a_query_index_lies_inside_its_set() {
    let mut d = Drawing::new();
    let set = d.r.create_query_set(QueryKind::OcclusionAny, 1).unwrap();
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: Some(&set),
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &d.target_view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Load,
        store: StoreOp::Store,
    });
    d.r.begin_pass(&pass);
    d.r.begin_query(1);
    d.r.end_query();
    d.refused("query index past the set");
}

#[test]
fn an_attachment_is_one_level_and_layer_with_render_usage() {
    let mut r = Recorder::new();
    let t = texture(&mut r, Format::Rgba8Unorm, 2, TextureUsage::SAMPLE);
    let v = view(&mut r, &t, Format::Rgba8Unorm);
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &v,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Load,
        store: StoreOp::Store,
    });
    r.begin_pass(&pass);
    assert_eq!(r.violations, ["an attachment without render usage"]);
}

#[test]
fn an_attachment_slice_lies_inside_its_level() {
    let mut d = Drawing::new();
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &d.target_view,
        depth_slice: 1,
        resolve: None,
        load: LoadOp::Load,
        store: StoreOp::Store,
    });
    d.r.begin_pass(&pass);
    d.refused("an attachment's slice outside its level");
}

#[test]
fn a_draw_reads_no_subresource_it_writes() {
    let mut d = Drawing::new();
    let p = d.program("loads");
    let target_view = d.target_view;
    let set =
        d.r.create_binding_set(&[BindingEntry {
            binding: 32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(&target_view),
        }])
        .unwrap();
    d.begin();
    d.draw(&p, Some(&set), Topology::TriangleList, false);
    d.refused("a subresource bound for reading while it is a written attachment");
}

#[test]
fn strips_alone_restart() {
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw(&p, None, Topology::TriangleList, true);
    d.refused("restart on a list topology");
}

#[test]
fn a_program_finds_every_binding_it_reads() {
    let mut d = Drawing::new();
    let p = d.program("colored");
    d.begin();
    d.draw(&p, None, Topology::TriangleList, false);
    d.refused("nothing bound at group 0 binding 0");
}

#[test]
fn a_binding_set_names_each_binding_once() {
    let mut r = Recorder::new();
    let b = buffer(&mut r, 256, BufferUsage::UNIFORM);
    let entry = BindingEntry {
        binding: 0,
        stages: StageSet::FRAGMENT,
        resource: BindingResource::UniformBuffer(BufferRange {
            buffer: &b,
            offset: 0,
            size: 16,
        }),
    };
    let twice = [
        entry,
        BindingEntry {
            binding: 0,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &b,
                offset: 0,
                size: 16,
            }),
        },
    ];
    assert_eq!(
        r.create_binding_set(&twice),
        Err(GpuError::Invalid("two entries at one binding"))
    );
}

#[test]
fn buffers_and_textures_are_used_only_as_made() {
    let mut r = Recorder::new();
    let b = buffer(&mut r, 16, BufferUsage::COPY_SRC);
    r.write_buffer(&b, 0, &[1], WriteMode::Ordered);
    assert_eq!(
        r.violations,
        [format!(
            "write_buffer of buffer {} made without BufferUsage(COPY_DST)",
            0
        )]
    );

    let mut r = Recorder::new();
    let b = buffer(&mut r, 256, BufferUsage::COPY_DST);
    let t = texture(&mut r, Format::Rgba8Unorm, 2, TextureUsage::COPY_DST);
    let v = view(&mut r, &t, Format::Rgba8Unorm);
    let uniform = [BindingEntry {
        binding: 0,
        stages: StageSet::FRAGMENT,
        resource: BindingResource::UniformBuffer(BufferRange {
            buffer: &b,
            offset: 0,
            size: 16,
        }),
    }];
    assert_eq!(
        r.create_binding_set(&uniform),
        Err(GpuError::Invalid(
            "a uniform binding of a buffer without UNIFORM"
        ))
    );
    let sampled = [BindingEntry {
        binding: 32,
        stages: StageSet::FRAGMENT,
        resource: BindingResource::Texture(&v),
    }];
    assert_eq!(
        r.create_binding_set(&sampled),
        Err(GpuError::Invalid(
            "a texture binding of a texture without SAMPLE"
        ))
    );
    assert!(
        r.read_back(&ReadSource::Texture {
            region: region(&t),
            extent: extent(2),
        })
        .is_err()
    );
    assert_eq!(
        r.violations,
        [format!(
            "a read back of texture {} made without TextureUsage(COPY_SRC)",
            1
        )]
    );

    let mut d = Drawing::new();
    let p = d.program("plain");
    let other = buffer(&mut d.r, 24, BufferUsage::COPY_DST);
    d.vertices = other;
    d.begin();
    d.draw(&p, None, Topology::TriangleList, false);
    d.refused(&format!(
        "a vertex stream of buffer {} made without BufferUsage(VERTEX)",
        format!("{other:?}")
            .trim_start_matches("Id(")
            .trim_end_matches(')')
    ));
}

#[test]
fn texture_copies_stay_in_one_typeless_group() {
    let mut r = Recorder::new();
    let a = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    let b = texture(&mut r, Format::R32Float, 2, ALL);
    let c = texture(&mut r, Format::Rgba8Uint, 2, ALL);
    let d = texture(&mut r, Format::Depth32Float, 2, ALL);
    // The same bits seen differently copy.
    r.copy_texture(&region(&a), &region(&c), extent(2));
    r.copy_texture(&region(&b), &region(&d), extent(2));
    assert!(r.violations.is_empty());
    r.copy_texture(&region(&a), &region(&b), extent(2));
    assert_eq!(
        r.violations,
        ["copy between formats of different typeless groups"]
    );
}

#[test]
fn views_take_the_textures_format_or_its_twin() {
    let mut r = Recorder::new();
    let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    let desc = TextureViewDesc {
        format: Format::Rgba8UnormSrgb,
        dimension: ViewDimension::D2,
        base_mip: 0,
        mip_count: 1,
        base_layer: 0,
        layer_count: 1,
        stencil: false,
    };
    // Without `reinterpret`, not even the twin.
    assert_eq!(
        r.create_view(&t, &desc),
        Err(GpuError::Invalid(
            "view format neither the texture's nor its twin"
        ))
    );
    assert_eq!(
        r.create_view(
            &t,
            &TextureViewDesc {
                format: Format::Rgba8Unorm,
                mip_count: 2,
                ..desc
            }
        ),
        Err(GpuError::Invalid("view range outside the texture"))
    );
}

#[test]
fn writes_copies_and_clears_stay_inside_their_buffers() {
    let mut r = Recorder::new();
    let b = buffer(&mut r, 16, COPIES);
    r.write_buffer(&b, 12, &[0; 8], WriteMode::Ordered);
    r.copy_buffer(&b, 0, &b, 4, 8);
    r.clear_buffer(&b, 0, 6, &[1, 2, 3, 4]);
    assert_eq!(
        r.violations,
        [
            "write past the buffer's end",
            "copy between overlapping ranges of one buffer",
            "clear pattern of a size the interface does not take",
        ]
    );
}

#[test]
fn buffer_texture_copies_take_pitches_covering_the_region() {
    let mut r = Recorder::new();
    let b = buffer(&mut r, 256, COPIES);
    let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    r.copy_texture_to_buffer(
        &region(&t),
        &CopyBufferTexture {
            buffer: &b,
            offset: 0,
            row_pitch: 4,
            rows_per_layer: 2,
        },
        extent(2),
    );
    assert_eq!(r.violations, ["copy pitch smaller than the region"]);
}

#[test]
fn a_read_finishes_into_bytes_of_its_size() {
    let mut r = Recorder::new();
    let b = buffer(&mut r, 8, COPIES);
    let rb = r
        .read_back(&ReadSource::Buffer {
            buffer: &b,
            offset: 0,
            size: 8,
        })
        .unwrap();
    r.wait_read(&rb);
    r.finish_read(rb, &mut [0; 4]);
    assert_eq!(
        r.violations,
        ["finish_read into 4 bytes of a 8-byte read back"]
    );
}

#[test]
fn no_overwrite_writes_touch_nothing_unfinished_reads() {
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw(&p, None, Topology::TriangleList, false);
    d.r.end_pass();
    let vertices = d.vertices;
    // The draw read bytes 0 to 24; past them a no-overwrite write is fine, and after a submit
    // anywhere.
    let more = buffer(&mut d.r, 48, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    d.r.write_buffer(&more, 0, &[0; 4], WriteMode::NoOverwrite);
    d.r.write_buffer(&vertices, 16, &[0; 4], WriteMode::NoOverwrite);
    d.refused(&format!(
        "a NoOverwrite write over bytes of buffer {} an unfinished command reads",
        format!("{vertices:?}")
            .trim_start_matches("Id(")
            .trim_end_matches(')')
    ));
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw(&p, None, Topology::TriangleList, false);
    d.r.end_pass();
    let fence = d.r.submit();
    d.r.wait_fence(&fence);
    let vertices = d.vertices;
    d.r.write_buffer(&vertices, 16, &[0; 4], WriteMode::NoOverwrite);
    assert!(d.r.violations.is_empty());
}

#[test]
fn work_recorded_before_a_destroy_still_sees_the_object() {
    let mut r = Recorder::new();
    let a = buffer(&mut r, 4, COPIES);
    let b = buffer(&mut r, 4, COPIES);
    r.write_buffer(&a, 0, &[1, 2, 3, 4], WriteMode::Discard);
    r.copy_buffer(&a, 0, &b, 0, 4);
    r.destroy(Object::Buffer(a));
    let mut got = [0; 4];
    r.read_back_now(
        &ReadSource::Buffer {
            buffer: &b,
            offset: 0,
            size: 4,
        },
        &mut got,
    )
    .unwrap();
    assert_eq!(got, [1, 2, 3, 4]);
    assert!(r.violations.is_empty());
}

#[test]
fn storage_writes_come_from_fragment_programs_only() {
    let mut r = Recorder::new();
    let b = buffer(&mut r, 16, BufferUsage::STORAGE);
    let entry = [BindingEntry {
        binding: 160,
        stages: StageSet::VERTEX | StageSet::FRAGMENT,
        resource: BindingResource::StorageBuffer {
            range: BufferRange {
                buffer: &b,
                offset: 0,
                size: 16,
            },
            writable: true,
        },
    }];
    assert_eq!(
        r.create_binding_set(&entry),
        Err(GpuError::Absent(Features::VERTEX_STORAGE_WRITE))
    );
}

#[test]
fn present_needs_the_current_texture() {
    let mut r = Recorder::new();
    let s = r.create_surface(1).unwrap();
    r.configure_surface(&s, Format::Rgba8Unorm, 2, 2, true)
        .unwrap();
    assert!(r.present(&s).is_err());
    assert_eq!(r.violations, ["present without a current texture"]);
}
