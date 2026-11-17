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
@group(0) @binding(16) var s: sampler;
@fragment fn sampled() -> @location(0) vec4<f32> {
    return textureSampleLevel(t, s, vec2<f32>(0.5), 0.0);
}
@fragment fn per_sample(@builtin(sample_index) i: u32) -> @location(0) vec4<f32> {
    return vec4<f32>(f32(i));
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

const ALL: TextureUsage = TextureUsage::all().difference(TextureUsage::STORAGE);
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
        Drawing::on(Recorder::new())
    }

    /// The same on a recording back end whose capabilities `change` alters.
    fn with_caps(change: impl FnOnce(&mut Caps)) -> Drawing {
        Drawing::on(Recorder::with_caps(change))
    }

    fn on(mut r: Recorder) -> Drawing {
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
            naga::valid::Capabilities::MULTISAMPLED_SHADING,
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
        self.draw_with(program, set, topology, restart, |_| {});
    }

    /// Draws as [`Drawing::draw`] with the call changed by `change` first.
    fn draw_with(
        &mut self,
        program: &Id,
        set: Option<&Id>,
        topology: Topology,
        restart: bool,
        change: impl FnOnce(&mut DrawCall<'_, Recorder>),
    ) {
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
        let mut call = DrawCall {
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
        };
        change(&mut call);
        self.r.draw(&call);
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
    // The draw read bytes 0 to 24; past them a no-overwrite write is fine, and anywhere once the
    // caller waited for the work.
    let more = buffer(&mut d.r, 48, BufferUsage::VERTEX | BufferUsage::COPY_DST);
    d.r.write_buffer(&more, 0, &[0; 4], WriteMode::NoOverwrite);
    d.r.write_buffer(&vertices, 16, &[0; 4], WriteMode::NoOverwrite);
    d.refused(&format!(
        "a NoOverwrite write over bytes of buffer {} an unfinished command or an ordered write \
         touches",
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
    // An ordered write not yet waited for counts as unfinished too.
    d.r.write_buffer(&vertices, 0, &[0; 4], WriteMode::Ordered);
    d.r.write_buffer(&vertices, 2, &[0; 4], WriteMode::NoOverwrite);
    assert_eq!(d.r.violations.len(), 1);
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

fn multisampled(r: &mut Recorder, format: Format, size: u32) -> Result<Id, GpuError> {
    r.create_texture(&TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width: size,
        height: size,
        depth_or_layers: 1,
        mip_levels: 1,
        samples: 4,
        usage: TextureUsage::RENDER
            | TextureUsage::SAMPLE
            | TextureUsage::COPY_SRC
            | TextureUsage::COPY_DST,
        cube: false,
        reinterpret: false,
    })
}

#[test]
fn a_multisampled_texture_is_2d_with_one_level_and_layer_and_no_storage() {
    let mut r = Recorder::new();
    assert!(multisampled(&mut r, Format::Rgba8Unorm, 2).is_ok());
    let desc = TextureDesc {
        dimension: TextureDimension::D2,
        format: Format::Rgba8Unorm,
        width: 2,
        height: 2,
        depth_or_layers: 1,
        mip_levels: 1,
        samples: 4,
        usage: TextureUsage::RENDER,
        cube: false,
        reinterpret: false,
    };
    for broken in [
        TextureDesc {
            depth_or_layers: 2,
            ..desc
        },
        TextureDesc {
            mip_levels: 2,
            ..desc
        },
        TextureDesc {
            dimension: TextureDimension::D3,
            ..desc
        },
        TextureDesc {
            usage: TextureUsage::RENDER | TextureUsage::STORAGE,
            ..desc
        },
    ] {
        assert!(
            matches!(r.create_texture(&broken), Err(GpuError::Invalid(_))),
            "{broken:?}"
        );
    }
    // A count the format does not take.
    assert!(
        r.create_texture(&TextureDesc { samples: 2, ..desc })
            .is_err()
    );
}

/// A call moving bytes of a multisampled texture (given with another, a single-sampled texture and a
/// buffer).
type MovesBytes = fn(&mut Recorder, &Id, &Id, &Id, &Id);

/// A call breaking a rule on a fresh recorder.
type Breaks = fn(&mut Recorder);

#[test]
fn a_multisampled_textures_bytes_move_by_draws_resolves_and_whole_copies() {
    let moved =
        "a multisampled texture's bytes moved other than by a draw, a resolve or a whole copy";
    let not_whole = "a multisampled copy that is not of whole subresources of one sample count";
    let broken: [(&str, MovesBytes); 5] = [
        (moved, |r, ms, _, _, _| {
            r.write_texture(&region(ms), extent(2), &[0; 16], 8, 2, WriteMode::Ordered)
        }),
        (moved, |r, ms, _, _, b| {
            let dst = CopyBufferTexture {
                buffer: b,
                offset: 0,
                row_pitch: 8,
                rows_per_layer: 2,
            };
            r.copy_texture_to_buffer(&region(ms), &dst, extent(2))
        }),
        (moved, |r, ms, _, _, _| {
            let _ = r.read_back(&ReadSource::Texture {
                region: region(ms),
                extent: extent(2),
            });
        }),
        (not_whole, |r, ms, other, _, _| {
            r.copy_texture(&region(ms), &region(other), extent(1))
        }),
        (not_whole, |r, ms, _, single, _| {
            r.copy_texture(&region(ms), &region(single), extent(2))
        }),
    ];
    for (violation, f) in broken {
        let mut r = Recorder::new();
        let ms = multisampled(&mut r, Format::Rgba8Unorm, 2).unwrap();
        let other = multisampled(&mut r, Format::Rgba8Unorm, 2).unwrap();
        let single = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
        let b = buffer(&mut r, 64, COPIES);
        f(&mut r, &ms, &other, &single, &b);
        assert_eq!(r.violations, [violation]);
    }
    // A whole copy between equal sample counts is one of the ways.
    let mut r = Recorder::new();
    let ms = multisampled(&mut r, Format::Rgba8Unorm, 2).unwrap();
    let other = multisampled(&mut r, Format::Rgba8Unorm, 2).unwrap();
    r.copy_texture(&region(&ms), &region(&other), extent(2));
    assert!(r.violations.is_empty());
}

#[test]
fn attachments_share_one_sample_count() {
    let mut r = Recorder::new();
    let ms = multisampled(&mut r, Format::Rgba8Unorm, 2).unwrap();
    let ms = view(&mut r, &ms, Format::Rgba8Unorm);
    let single = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    let single = view(&mut r, &single, Format::Rgba8Unorm);
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    for (i, v) in [&ms, &single].into_iter().enumerate() {
        pass.colors[i] = Some(ColorAttachment {
            view: v,
            depth_slice: 0,
            resolve: None,
            load: LoadOp::Load,
            store: StoreOp::Store,
        });
    }
    r.begin_pass(&pass);
    assert_eq!(r.violations, ["attachments of different sample counts"]);
}

/// A resolve, at a pass's end or by `resolve`, goes from a multisampled subresource into a
/// single-sampled one of its size and format (or the twin, for `resolve`), of a format that averages.
#[test]
fn resolves_go_from_many_samples_to_one_of_an_averaging_format() {
    fn pass_resolving(r: &mut Recorder, from: &Id, to: &Id) {
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view: from,
            depth_slice: 0,
            resolve: Some(to),
            load: LoadOp::Load,
            store: StoreOp::Discard,
        });
        r.begin_pass(&pass);
    }
    let cases: [(&str, Breaks); 6] = [
        (
            "a resolve from a single-sampled attachment or into a multisampled one",
            |r| {
                let a = texture(r, Format::Rgba8Unorm, 2, ALL);
                let a = view(r, &a, Format::Rgba8Unorm);
                let b = texture(r, Format::Rgba8Unorm, 2, ALL);
                let b = view(r, &b, Format::Rgba8Unorm);
                pass_resolving(r, &a, &b);
            },
        ),
        (
            "a resolve target of another format than its attachment",
            |r| {
                let a = multisampled(r, Format::Rgba8Unorm, 2).unwrap();
                let a = view(r, &a, Format::Rgba8Unorm);
                let b = texture(r, Format::Bgra8Unorm, 2, ALL);
                let b = view(r, &b, Format::Bgra8Unorm);
                pass_resolving(r, &a, &b);
            },
        ),
        ("a resolve of a format that cannot be averaged", |r| {
            let a = multisampled(r, Format::Rgba8Uint, 2).unwrap();
            let a = view(r, &a, Format::Rgba8Uint);
            let b = texture(r, Format::Rgba8Uint, 2, ALL);
            let b = view(r, &b, Format::Rgba8Uint);
            pass_resolving(r, &a, &b);
        }),
        ("attachments of different sizes", |r| {
            let a = multisampled(r, Format::Rgba8Unorm, 2).unwrap();
            let a = view(r, &a, Format::Rgba8Unorm);
            let b = texture(r, Format::Rgba8Unorm, 4, ALL);
            let b = view(r, &b, Format::Rgba8Unorm);
            pass_resolving(r, &a, &b);
        }),
        ("a resolve between subresources of different sizes", |r| {
            let a = multisampled(r, Format::Rgba8Unorm, 2).unwrap();
            let b = texture(r, Format::Rgba8Unorm, 4, ALL);
            r.resolve(&region(&a), &region(&b), Format::Rgba8Unorm);
        }),
        (
            "a resolve between formats other than one format and its sRGB twin",
            |r| {
                let a = multisampled(r, Format::Rgba8Unorm, 2).unwrap();
                let b = texture(r, Format::Rgba8Unorm, 2, ALL);
                r.resolve(&region(&a), &region(&b), Format::Bgra8Unorm);
            },
        ),
    ];
    for (violation, f) in cases {
        let mut r = Recorder::new();
        f(&mut r);
        assert_eq!(r.violations, [violation]);
    }
}

/// A blend block where every target writes everything, blended as `blend` says.
fn blend_desc(blend: Option<(BlendComponent, BlendComponent)>) -> BlendDesc {
    BlendDesc {
        targets: [RenderTargetBlend {
            blend,
            writes: ColorWrites::all(),
        }; MAX_COLOR_TARGETS],
        same_for_all: true,
        alpha_to_coverage: false,
        logic_op: None,
    }
}

fn component(src: BlendFactor, dst: BlendFactor) -> BlendComponent {
    BlendComponent {
        src,
        dst,
        op: BlendOp::Add,
    }
}

#[test]
fn blocks_differ_per_target_only_with_independent_blend() {
    let mut r = Recorder::with_caps(|c| c.features.remove(Features::INDEPENDENT_BLEND));
    let mut desc = blend_desc(None);
    assert!(r.create_blend(&desc).is_ok());
    desc.same_for_all = false;
    assert_eq!(
        r.create_blend(&desc),
        Err(GpuError::Absent(Features::INDEPENDENT_BLEND))
    );
    assert!(Recorder::new().create_blend(&desc).is_ok());
}

#[test]
fn the_constant_alpha_factors_need_blend_constant_alpha() {
    let mut r = Recorder::with_caps(|c| c.features.remove(Features::BLEND_CONSTANT_ALPHA));
    let add = component(BlendFactor::One, BlendFactor::Zero);
    let desc = blend_desc(Some((
        component(
            BlendFactor::ConstantAlpha,
            BlendFactor::OneMinusConstantAlpha,
        ),
        add,
    )));
    assert_eq!(
        r.create_blend(&desc),
        Err(GpuError::Absent(Features::BLEND_CONSTANT_ALPHA))
    );
    assert!(Recorder::new().create_blend(&desc).is_ok());
}

#[test]
fn stencil_masks_differ_per_face_only_with_stencil_per_face() {
    let mut r = Recorder::new();
    assert!(!r.caps().features.contains(Features::STENCIL_PER_FACE));
    let mut desc = DepthStencilDesc {
        depth_compare: None,
        depth_write: false,
        stencil: None,
        stencil_read_mask: [0xff, 0x0f],
        stencil_write_mask: [0xff; 2],
        depth_bounds: None,
    };
    let absent = Err(GpuError::Absent(Features::STENCIL_PER_FACE));
    assert_eq!(r.create_depth_stencil(&desc), absent);
    desc.stencil_read_mask = [0xff; 2];
    desc.stencil_write_mask = [0x01, 0x02];
    assert_eq!(r.create_depth_stencil(&desc), absent);
}

#[test]
fn stencil_references_differ_per_face_only_with_stencil_per_face() {
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c| {
        c.stencil_reference = [1, 2]
    });
    d.refused("stencil references per face without STENCIL_PER_FACE");
}

/// A change made to a block's description.
type Change = fn(&mut RasterDesc);

#[test]
fn raster_blocks_need_the_features_they_use() {
    let base = RasterDesc {
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
    };
    let cases: [(Features, Change); 7] = [
        (Features::PROVOKING_VERTEX_FIRST, |_| {}),
        (Features::PROVOKING_VERTEX_LAST, |d| {
            d.provoking = ProvokingVertex::Last
        }),
        (Features::DEPTH_BIAS_CLAMP, |d| {
            d.depth_bias_clamp = 0.5f32.to_bits()
        }),
        (Features::FRACTIONAL_DEPTH_BIAS, |d| {
            d.depth_bias = 1.5f32.to_bits()
        }),
        (Features::SAMPLE_MASK, |d| d.sample_mask = 0b1110),
        (Features::CLIP_DEPTH_ZERO_ONE, |_| {}),
        (Features::WIDE_LINES, |d| d.line_width = 2.0f32.to_bits()),
    ];
    for (feature, change) in cases {
        let mut desc = base;
        change(&mut desc);
        let mut r = Recorder::with_caps(|c| c.features.remove(feature));
        assert_eq!(
            r.create_raster(&desc),
            Err(GpuError::Absent(feature)),
            "{feature:?}"
        );
        if Recorder::new().caps().features.contains(feature) {
            assert!(Recorder::new().create_raster(&desc).is_ok(), "{feature:?}");
        }
    }
}

#[test]
fn buffer_bindings_are_aligned_inside_the_buffer_and_the_limits() {
    let mut r = Recorder::new();
    let limits = r.caps().limits;
    let uniform = buffer(&mut r, 1 << 17, BufferUsage::UNIFORM);
    let storage = buffer(&mut r, 1024, BufferUsage::STORAGE);
    let bind = |r: &mut Recorder, resource| {
        r.create_binding_set(&[BindingEntry {
            binding: 0,
            stages: StageSet::FRAGMENT,
            resource,
        }])
    };
    let range = |buffer, offset, size| BufferRange {
        buffer,
        offset,
        size,
    };
    let storage_at = |offset, size| BindingResource::StorageBuffer {
        range: range(&storage, offset, size),
        writable: false,
    };
    let unaligned = Err(GpuError::Invalid(
        "a buffer binding at an offset off the limit's alignment",
    ));
    assert_eq!(limits.min_storage_offset_alignment, 256);
    assert_eq!(bind(&mut r, storage_at(4, 16)), unaligned);
    assert_eq!(
        bind(
            &mut r,
            BindingResource::UniformBuffer(range(&uniform, 4, 16))
        ),
        unaligned
    );
    assert!(bind(&mut r, storage_at(256, 16)).is_ok());
    assert_eq!(
        bind(&mut r, storage_at(768, 512)),
        Err(GpuError::Invalid("a buffer binding past the buffer's end"))
    );
    assert_eq!(
        bind(
            &mut r,
            BindingResource::UniformBuffer(range(
                &uniform,
                0,
                u64::from(limits.max_uniform_buffer_size) + 256
            ))
        ),
        Err(GpuError::Invalid("a buffer binding larger than the limit"))
    );
}

#[test]
fn bindings_need_the_features_their_stages_and_textures_use() {
    let storage = |r: &mut Recorder, stages, writable| {
        let b = buffer(r, 256, BufferUsage::STORAGE);
        r.create_binding_set(&[BindingEntry {
            binding: 0,
            stages,
            resource: BindingResource::StorageBuffer {
                range: BufferRange {
                    buffer: &b,
                    offset: 0,
                    size: 256,
                },
                writable,
            },
        }])
    };
    let mut r = Recorder::with_caps(|c| c.features.remove(Features::FRAGMENT_STORAGE_WRITE));
    assert_eq!(
        storage(&mut r, StageSet::FRAGMENT, true),
        Err(GpuError::Absent(Features::FRAGMENT_STORAGE_WRITE))
    );
    let mut r = Recorder::new();
    assert!(!r.caps().features.contains(Features::VERTEX_STORAGE_READ));
    assert_eq!(
        storage(&mut r, StageSet::VERTEX, false),
        Err(GpuError::Absent(Features::VERTEX_STORAGE_READ))
    );

    let multisampled = |r: &mut Recorder| {
        let t = r
            .create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format: Format::Rgba8Unorm,
                width: 2,
                height: 2,
                depth_or_layers: 1,
                mip_levels: 1,
                samples: 4,
                usage: TextureUsage::RENDER | TextureUsage::SAMPLE,
                cube: false,
                reinterpret: false,
            })
            .unwrap();
        let v = view(r, &t, Format::Rgba8Unorm);
        r.create_binding_set(&[BindingEntry {
            binding: 32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(&v),
        }])
    };
    let mut r = Recorder::with_caps(|c| c.features.remove(Features::MULTISAMPLE_READ));
    assert_eq!(
        multisampled(&mut r),
        Err(GpuError::Absent(Features::MULTISAMPLE_READ))
    );
    assert!(multisampled(&mut Recorder::new()).is_ok());
}

#[test]
fn textures_take_only_the_usages_their_format_does() {
    let desc = |format, usage| TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width: 2,
        height: 2,
        depth_or_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage,
        cube: false,
        reinterpret: false,
    };
    let refused = Err(GpuError::Invalid(
        "a texture usage its format does not take",
    ));
    let mut r = Recorder::new();
    assert_eq!(
        r.create_texture(&desc(Format::Rgba8Unorm, TextureUsage::STORAGE)),
        refused
    );
    let mut r = Recorder::with_caps(|c| {
        let at = Format::R32Float.index();
        c.formats.uses[at].remove(FormatUse::RENDER);
    });
    assert_eq!(
        r.create_texture(&desc(Format::R32Float, TextureUsage::RENDER)),
        refused
    );
    assert!(
        r.create_texture(&desc(Format::R32Float, TextureUsage::SAMPLE))
            .is_ok()
    );
}

#[test]
fn viewports_keep_min_depth_at_most_max_depth() {
    const REVERSED: [Viewport; 1] = [Viewport {
        x: 0,
        y: 0,
        width: 0x4000_0000,
        height: 0x4000_0000,
        min_depth: 0x3f80_0000,
        max_depth: 0,
    }];
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c| {
        c.viewports = &REVERSED
    });
    d.refused("a viewport whose min_depth is above its max_depth");
}

#[test]
fn fractional_viewports_need_fractional_viewport() {
    const HALF: [Viewport; 1] = [Viewport {
        x: 0x3f00_0000,
        y: 0,
        width: 0x4000_0000,
        height: 0x4000_0000,
        min_depth: 0,
        max_depth: 0x3f80_0000,
    }];
    let mut d = Drawing::with_caps(|c| c.features.remove(Features::FRACTIONAL_VIEWPORT));
    let p = d.program("plain");
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c| {
        c.viewports = &HALF
    });
    d.refused("a fractional viewport without FRACTIONAL_VIEWPORT");
}

#[test]
fn draw_bases_need_base_vertex_instance() {
    let mut d = Drawing::with_caps(|c| c.features.remove(Features::BASE_VERTEX_INSTANCE));
    let p = d.program("plain");
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c| {
        c.range = DrawRange::Vertices {
            first: 0,
            count: 3,
            first_instance: 1,
            instances: 1,
        }
    });
    d.refused("a base vertex or first instance without BASE_VERTEX_INSTANCE");
}

#[test]
fn eight_bit_indices_need_index_u8() {
    let mut d = Drawing::with_caps(|c| c.features.remove(Features::INDEX_U8));
    let p = d.program("plain");
    let indices: &'static Id = Box::leak(Box::new(buffer(&mut d.r, 4, BufferUsage::INDEX)));
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c| {
        c.index = Some(IndexBinding {
            buffer: indices,
            offset: 0,
            format: IndexFormat::U8,
        });
        c.range = DrawRange::Indexed {
            first: 0,
            count: 3,
            base_vertex: 0,
            first_instance: 0,
            instances: 1,
        };
    });
    d.refused("8-bit indices without INDEX_U8");
}

#[test]
fn immediates_stay_within_the_limit() {
    let mut d = Drawing::new();
    assert_eq!(d.r.caps().limits.immediate_bytes, 0);
    let p = d.program("plain");
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c| {
        c.immediates = &[0; 4]
    });
    d.refused("immediates past the limit");
}

#[test]
fn draws_blend_only_into_formats_that_blend() {
    let mut d =
        Drawing::with_caps(|c| c.formats.uses[Format::Rgba8Unorm.index()].remove(FormatUse::BLEND));
    let add = component(BlendFactor::One, BlendFactor::One);
    d.blend = d.r.create_blend(&blend_desc(Some((add, add)))).unwrap();
    let p = d.program("plain");
    d.begin();
    d.draw(&p, None, Topology::TriangleList, false);
    d.refused("blending into Rgba8Unorm, which this back end does not blend");
}

#[test]
fn draws_filter_only_formats_that_filter() {
    let mut d =
        Drawing::with_caps(|c| c.formats.uses[Format::R32Float.index()].remove(FormatUse::FILTER));
    let t = texture(&mut d.r, Format::R32Float, 2, TextureUsage::SAMPLE);
    let v = view(&mut d.r, &t, Format::R32Float);
    let s =
        d.r.create_sampler(&SamplerDesc {
            min: Filter::Linear,
            mag: Filter::Linear,
            mip: Filter::Nearest,
            address: [AddressMode::Clamp; 3],
            lod_range: [0.0f32.to_bits(), 32.0f32.to_bits()],
            anisotropy: 1,
            compare: None,
            border: BorderColor::TransparentBlack,
        })
        .unwrap();
    let set =
        d.r.create_binding_set(&[
            BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&v),
            },
            BindingEntry {
                binding: 16,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Sampler(&s),
            },
        ])
        .unwrap();
    let p = d.program("sampled");
    d.begin();
    d.draw(&p, Some(&set), Topology::TriangleList, false);
    d.refused("R32Float filtered, which this back end does not filter");
}

#[test]
fn programs_run_per_sample_only_with_sample_shading() {
    let mut d = Drawing::with_caps(|c| c.features.remove(Features::SAMPLE_SHADING));
    let p = d.program("per_sample");
    d.begin();
    d.draw(&p, None, Topology::TriangleList, false);
    d.refused("a program run per sample without SAMPLE_SHADING");
}

#[test]
fn resolves_and_copies_out_need_the_formats_uses() {
    let mut r = Recorder::with_caps(|c| {
        c.formats.uses[Format::Rgba8Unorm.index()].remove(FormatUse::RESOLVE | FormatUse::COPY_OUT)
    });
    let many = r
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format: Format::Rgba8Unorm,
            width: 2,
            height: 2,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 4,
            usage: TextureUsage::RENDER,
            cube: false,
            reinterpret: false,
        })
        .unwrap();
    let one = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    r.resolve(&region(&many), &region(&one), Format::Rgba8Unorm);
    assert_eq!(
        r.violations,
        ["a resolve of a format that cannot be averaged"]
    );
    let mut r = Recorder::with_caps(|c| {
        c.formats.uses[Format::Rgba8Unorm.index()].remove(FormatUse::COPY_OUT)
    });
    let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    let b = buffer(&mut r, 16, COPIES);
    r.copy_texture_to_buffer(
        &region(&t),
        &CopyBufferTexture {
            buffer: &b,
            offset: 0,
            row_pitch: 8,
            rows_per_layer: 0,
        },
        extent(2),
    );
    assert_eq!(
        r.violations,
        ["Rgba8Unorm copied out, which this back end does not declare"]
    );
}

/// Buffer copies and read backs of a format with depth and stencil move the aspect named: the
/// stencil byte, or depth in four bytes; copies between textures move both, naming neither.
#[test]
fn copies_of_depth_and_stencil_move_the_aspect_named() {
    let mut r = Recorder::new();
    let t = texture(&mut r, Format::Depth24PlusStencil8, 2, ALL);
    let b = buffer(&mut r, 64, COPIES);
    let to_buffer = |stencil, row_pitch| {
        (
            TextureRegion {
                stencil,
                ..region(&t)
            },
            CopyBufferTexture {
                buffer: &b,
                offset: 0,
                row_pitch,
                rows_per_layer: 0,
            },
        )
    };
    // Stencil 1..4 and depth words 0x123456 + i written by aspect.
    let (s, from) = to_buffer(true, 2);
    r.write_texture(&s, extent(2), &[1, 2, 3, 4], 2, 0, WriteMode::Ordered);
    let depth: Vec<u8> = (0..4u32)
        .flat_map(|i| (0x0012_3456 + i).to_le_bytes())
        .collect();
    let (dr, _) = to_buffer(false, 8);
    r.write_texture(&dr, extent(2), &depth, 8, 0, WriteMode::Ordered);
    r.copy_texture_to_buffer(&s, &from, extent(2));
    let rb = r
        .read_back(&ReadSource::Buffer {
            buffer: &b,
            offset: 0,
            size: 4,
        })
        .unwrap();
    r.submit();
    r.wait_read(&rb);
    let mut got = [0u8; 4];
    r.finish_read(rb, &mut got);
    assert_eq!(got, [1, 2, 3, 4]);
    let rb = r
        .read_back(&ReadSource::Texture {
            region: dr,
            extent: extent(2),
        })
        .unwrap();
    r.submit();
    r.wait_read(&rb);
    let mut got = [0u8; 16];
    r.finish_read(rb, &mut got);
    assert_eq!(got.to_vec(), depth);
    assert!(r.violations.is_empty(), "{:?}", r.violations);

    let other = texture(&mut r, Format::Depth24PlusStencil8, 2, ALL);
    r.copy_texture(&s, &region(&other), extent(2));
    assert_eq!(
        r.violations,
        ["a copy between textures naming one aspect of depth and stencil"]
    );
    let mut r = Recorder::new();
    let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    r.write_texture(
        &TextureRegion {
            stencil: true,
            ..region(&t)
        },
        extent(2),
        &[0; 16],
        8,
        0,
        WriteMode::Ordered,
    );
    assert_eq!(
        r.violations,
        ["a stencil region of a format without stencil"]
    );
}

/// Without `TEXTURE_VIEWS` a back end makes views to draw into of one level and one layer (or all
/// layers) and views of all layers in the texture's own dimension; it binds only the latter.
#[test]
fn views_of_other_shapes_need_texture_views() {
    let mut r = Recorder::with_caps(|c| c.features.remove(Features::TEXTURE_VIEWS));
    let t = r
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format: Format::Rgba8Unorm,
            width: 4,
            height: 4,
            depth_or_layers: 3,
            mip_levels: 2,
            samples: 1,
            usage: ALL,
            cube: false,
            reinterpret: false,
        })
        .unwrap();
    let shape = |dimension, mip_count, base_layer, layer_count| TextureViewDesc {
        format: Format::Rgba8Unorm,
        dimension,
        base_mip: 0,
        mip_count,
        base_layer,
        layer_count,
        stencil: false,
    };
    let absent = Err(GpuError::Absent(Features::TEXTURE_VIEWS));
    // Two layers of three, and one layer over two levels.
    assert_eq!(
        r.create_view(&t, &shape(ViewDimension::D2Array, 1, 0, 2)),
        absent
    );
    assert_eq!(
        r.create_view(&t, &shape(ViewDimension::D2, 2, 1, 1)),
        absent
    );
    let whole = r
        .create_view(&t, &shape(ViewDimension::D2Array, 2, 0, 3))
        .unwrap();
    let layer = r
        .create_view(&t, &shape(ViewDimension::D2, 1, 1, 1))
        .unwrap();
    let bind = |r: &mut Recorder, v: &Id| {
        r.create_binding_set(&[BindingEntry {
            binding: 32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(v),
        }])
    };
    assert!(bind(&mut r, &whole).is_ok());
    assert_eq!(
        bind(&mut r, &layer),
        Err(GpuError::Absent(Features::TEXTURE_VIEWS))
    );
}
