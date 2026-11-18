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
@group(0) @binding(33) var m: texture_multisampled_2d<f32>;
@fragment fn loads_sample() -> @location(0) vec4<f32> {
    return textureLoad(m, vec2<i32>(0, 0), 0);
}
@group(0) @binding(34) var dt: texture_depth_2d;
@group(0) @binding(17) var c: sampler_comparison;
@fragment fn compared() -> @location(0) vec4<f32> {
    return vec4<f32>(textureSampleCompareLevel(dt, c, vec2<f32>(0.5), 0.4));
}
struct Q { c: vec4<f32>, d: vec4<f32> }
@group(0) @binding(1) var<uniform> q: Q;
@fragment fn first_of_two() -> @location(0) vec4<f32> {
    return q.c;
}
struct R { n: u32, a: array<vec4<f32>> }
@group(0) @binding(160) var<storage, read> r: R;
@fragment fn first_element() -> @location(0) vec4<f32> {
    return r.a[0];
}
struct Out { @builtin(position) p: vec4<f32>, @location(0) c: vec4<f32> }
@vertex fn vs_out(@location(0) v: vec2<f32>) -> Out {
    return Out(vec4<f32>(v, 0.5, 1.0), vec4<f32>(1.0));
}
@vertex fn vs_reads(@location(0) v: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(v, p.c.z, 1.0);
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
        usage,
        ..texture_desc(format, size)
    })
    .unwrap()
}

/// A square 2D texture of one level and layer with every use but storage.
fn texture_desc(format: Format, size: u32) -> TextureDesc {
    TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width: size,
        height: size,
        depth_or_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage: ALL,
        cube: false,
        reinterpret: false,
    }
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
    /// 8 bytes a draw's change may name as indices, vertices or indirect arguments.
    spare: Id,
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
        let uses = BufferUsage::VERTEX | BufferUsage::INDEX | BufferUsage::INDIRECT;
        let spare = buffer(&mut r, 8, uses | BufferUsage::COPY_DST);
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
            spare,
            layout,
            blend,
            depth,
            raster,
        }
    }

    fn program(&mut self, fragment: &str) -> Id {
        self.program_of("vs", fragment)
    }

    /// The program of the entry points named.
    fn program_of(&mut self, vertex: &str, fragment: &str) -> Id {
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
        let v = shader(vertex, ShaderStage::Vertex);
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
        self.draw_with(program, set, topology, restart, |_, _| {});
    }

    /// Draws as [`Drawing::draw`] with the call changed by `change` first, which is also given
    /// [`Drawing::spare`].
    fn draw_with(
        &mut self,
        program: &Id,
        set: Option<&Id>,
        topology: Topology,
        restart: bool,
        change: impl for<'c> FnOnce(&mut DrawCall<'c, Recorder>, &'c Id),
    ) {
        self.draw_sets(program, [set, None, None, None], topology, restart, change);
    }

    /// Draws as [`Drawing::draw_with`] with the binding sets given.
    fn draw_sets(
        &mut self,
        program: &Id,
        sets: [Option<&Id>; MAX_BINDING_SETS],
        topology: Topology,
        restart: bool,
        change: impl for<'c> FnOnce(&mut DrawCall<'c, Recorder>, &'c Id),
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
            bindings: sets,
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
        change(&mut call, &self.spare);
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
    // Before any other rule the draw breaks.
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.draw_with(&p, None, Topology::TriangleList, false, |c, _| {
        c.viewports = &[]
    });
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

    // Copies between buffers and textures, and resolves, each alone.
    let side = |b| CopyBufferTexture {
        buffer: b,
        offset: 0,
        row_pitch: 256,
        rows_per_layer: 0,
    };
    let mut d = Drawing::new();
    let other = texture(&mut d.r, Format::Rgba8Unorm, 2, ALL);
    let b = buffer(&mut d.r, 1024, COPIES);
    d.begin();
    d.r.copy_buffer_to_texture(&side(&b), &region(&other), extent(2));
    d.refused("copy_buffer_to_texture inside a pass");
    let mut d = Drawing::new();
    let other = texture(&mut d.r, Format::Rgba8Unorm, 2, ALL);
    let b = buffer(&mut d.r, 1024, COPIES);
    d.begin();
    d.r.copy_texture_to_buffer(&region(&other), &side(&b), extent(2));
    d.refused("copy_texture_to_buffer inside a pass");
    let mut d = Drawing::new();
    let many =
        d.r.create_texture(&TextureDesc {
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
    let other = texture(&mut d.r, Format::Rgba8Unorm, 2, ALL);
    d.begin();
    d.r.resolve(&region(&many), &region(&other), Format::Rgba8Unorm);
    d.refused("resolve inside a pass");
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

/// A query ended after its set was destroyed writes into a destroyed object.
#[test]
fn a_query_ends_into_a_live_set() {
    let mut d = Drawing::new();
    let set = d.r.create_query_set(QueryKind::OcclusionAny, 1).unwrap();
    let v = d.target_view;
    d.r.begin_pass(&PassDesc {
        occlusion: Some(&set),
        ..pass_of([color(&v)])
    });
    d.r.begin_query(0);
    d.r.destroy(Object::QuerySet(set));
    d.r.end_query();
    d.refused("the occlusion set was destroyed");
}

#[test]
fn objects_are_used_live_and_as_their_kind() {
    let mut r = Recorder::new();
    let b = buffer(&mut r, 16, COPIES);
    let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    r.write_texture(&region(&b), extent(1), &[0; 4], 4, 1, WriteMode::Ordered);
    r.write_buffer(&t, 0, &[0], WriteMode::Ordered);
    assert_eq!(
        r.create_view(
            &b,
            &TextureViewDesc {
                format: Format::Rgba8Unorm,
                dimension: ViewDimension::D2,
                base_mip: 0,
                mip_count: 1,
                base_layer: 0,
                layer_count: 1,
                stencil: false,
            }
        ),
        Err(GpuError::Invalid(
            "view of an object that is not a live texture"
        ))
    );
    r.destroy(Object::Buffer(b));
    r.write_buffer(&b, 0, &[0], WriteMode::Ordered);
    assert_eq!(
        r.violations,
        [
            format!("object {} is not a texture", n(&b)),
            format!("object {} is not a buffer", n(&t)),
            format!("object {} used after it was destroyed", n(&b)),
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

/// Step rates other than one, and typed texel buffers, are reported absent where their features
/// are, so fronts take their routes in the program.
#[test]
fn step_rates_and_texel_buffers_need_their_features() {
    let layout = |step| {
        let mut desc = VertexLayoutDesc {
            streams: [None; MAX_VERTEX_STREAMS],
            attributes: [None; MAX_VERTEX_STREAMS],
        };
        desc.streams[0] = Some(VertexStream { stride: 8, step });
        desc
    };
    let mut r = Recorder::new();
    for step in [StepMode::Instance(0), StepMode::Instance(2)] {
        assert!(r.create_vertex_layout(&layout(step)).is_ok());
    }
    let mut r = Recorder::with_caps(|c| c.features.remove(Features::INSTANCE_STEP_RATE));
    for step in [StepMode::Vertex, StepMode::Instance(1)] {
        assert!(r.create_vertex_layout(&layout(step)).is_ok());
    }
    for step in [StepMode::Instance(0), StepMode::Instance(2)] {
        assert_eq!(
            r.create_vertex_layout(&layout(step)),
            Err(GpuError::Absent(Features::INSTANCE_STEP_RATE))
        );
    }

    let mut r = Recorder::new();
    let b = buffer(&mut r, 64, BufferUsage::STORAGE);
    assert_eq!(
        r.create_binding_set(&[BindingEntry {
            binding: 32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::TexelBuffer {
                range: BufferRange {
                    buffer: &b,
                    offset: 0,
                    size: 64,
                },
                format: Format::R32Float,
                writable: false,
            },
        }]),
        Err(GpuError::Absent(Features::TEXEL_BUFFERS))
    );
}

#[test]
fn a_query_index_is_begun_once_in_a_pass() {
    let mut d = Drawing::new();
    let set = d.r.create_query_set(QueryKind::OcclusionAny, 2).unwrap();
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
    // Once in each of two passes is fine.
    for _ in 0..2 {
        d.r.begin_pass(&pass);
        d.r.begin_query(0);
        d.r.end_query();
        d.r.begin_query(1);
        d.r.end_query();
        d.r.end_pass();
    }
    assert!(d.r.violations.is_empty(), "{:?}", d.r.violations);
    d.r.begin_pass(&pass);
    d.r.begin_query(0);
    d.r.end_query();
    d.r.begin_query(0);
    d.refused("query 0 begun twice in one pass");
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

/// Programs converted apart meet here: the vertex shader writes every location the fragment shader
/// reads, of its type, interpolation and sampling, and may write more (`VariantKey::linked` and
/// `output_interpolation`).
#[test]
fn fragment_inputs_are_vertex_outputs_of_their_type_and_interpolation() {
    const LINKED: &str = "
struct V { @builtin(position) p: vec4<f32>, @location(0) a: vec4<f32>, @location(3) b: vec4<f32>,
    @location(5) @interpolate(flat) n: u32, @location(7) @interpolate(flat) f: f32 }
@vertex fn vs() -> V { return V(vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0), 0u, 0.0); }
struct Gap { @location(0) a: vec4<f32>, @location(5) @interpolate(flat) n: u32 }
@fragment fn gap(i: Gap) -> @location(0) vec4<f32> { return i.a; }
@fragment fn past(@location(6) c: vec4<f32>) -> @location(0) vec4<f32> { return c; }
@fragment fn narrower(@location(3) b: vec2<f32>) -> @location(0) vec4<f32> { return vec4<f32>(b, 0.0, 1.0); }
@fragment fn linear(@location(3) @interpolate(linear) b: vec4<f32>) -> @location(0) vec4<f32> { return b; }
@fragment fn centroid(@location(3) @interpolate(perspective, centroid) b: vec4<f32>) -> @location(0) vec4<f32> { return b; }
@fragment fn stated(@location(3) @interpolate(perspective, center) b: vec4<f32>) -> @location(0) vec4<f32> { return b; }
struct First { @location(5) @interpolate(flat, first) n: u32, @location(7) @interpolate(flat, first) f: f32 }
@fragment fn first_vertex(i: First) -> @location(0) vec4<f32> { return vec4<f32>(f32(i.n), i.f, 0.0, 1.0); }
@fragment fn either_vertex(@location(7) @interpolate(flat, either) f: f32) -> @location(0) vec4<f32> { return vec4<f32>(f); }
@fragment fn perspective_in(@location(7) f: f32) -> @location(0) vec4<f32> { return vec4<f32>(f); }
";
    let mut r = Recorder::new();
    let shader = |r: &mut Recorder, entry, stage| {
        r.create_shader(&ShaderModule::from_wgsl(LINKED, entry, stage).unwrap())
            .unwrap()
    };
    let v = shader(&mut r, "vs", ShaderStage::Vertex);
    let mut program = |entry| {
        let f = shader(&mut r, entry, ShaderStage::Fragment);
        r.create_program(&Program::Render {
            vertex: &v,
            fragment: Some(&f),
        })
        .map(|_| ())
    };
    // Reading fewer locations than are written, and stating the defaults (flat values are sampled
    // at the first vertex), are allowed.
    assert_eq!(program("gap"), Ok(()));
    assert_eq!(program("stated"), Ok(()));
    assert_eq!(program("first_vertex"), Ok(()));
    assert_eq!(
        program("past"),
        Err(GpuError::Invalid(
            "a fragment input the vertex shader does not write"
        ))
    );
    for entry in [
        "narrower",
        "linear",
        "centroid",
        "either_vertex",
        "perspective_in",
    ] {
        assert_eq!(
            program(entry),
            Err(GpuError::Invalid(
                "a fragment input of another type, interpolation or sampling than the vertex output"
            )),
            "{entry}"
        );
    }
}

#[test]
fn shaders_and_programs_hold_entry_points_of_their_stages() {
    let module = naga::front::wgsl::parse_str(PROGRAM).unwrap();
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::MULTISAMPLED_SHADING,
    )
    .validate(&module)
    .unwrap();
    let shader = |r: &mut Recorder, entry: &str, stage| {
        r.create_shader(&ShaderModule {
            module: module.clone(),
            info: info.clone(),
            entry_point: entry.into(),
            stage,
            key: [0; 32],
        })
    };
    let mut r = Recorder::new();
    let lacks = Err(GpuError::Invalid(
        "a shader naming an entry point of its stage its module lacks",
    ));
    assert_eq!(shader(&mut r, "none", ShaderStage::Vertex), lacks);
    assert_eq!(shader(&mut r, "plain", ShaderStage::Vertex), lacks);
    let v = shader(&mut r, "vs", ShaderStage::Vertex).unwrap();
    let f = shader(&mut r, "plain", ShaderStage::Fragment).unwrap();
    let other = Err(GpuError::Invalid("a program of shaders of other stages"));
    let program = |r: &mut Recorder, vertex, fragment| {
        r.create_program(&Program::Render {
            vertex,
            fragment: Some(fragment),
        })
    };
    assert_eq!(program(&mut r, &f, &f), other);
    assert_eq!(program(&mut r, &v, &v), other);
    assert!(program(&mut r, &v, &f).is_ok());
}

#[test]
fn a_vertex_program_reads_only_locations_its_layout_has() {
    let mut d = Drawing::new();
    let mut desc = VertexLayoutDesc {
        streams: [None; MAX_VERTEX_STREAMS],
        attributes: [None; MAX_VERTEX_STREAMS],
    };
    desc.streams[0] = Some(VertexStream {
        stride: 8,
        step: StepMode::Vertex,
    });
    desc.attributes[0] = Some(VertexAttribute {
        location: 1,
        stream: 0,
        offset: 0,
        format: VertexFormat::Float32x2,
    });
    d.layout = d.r.create_vertex_layout(&desc).unwrap();
    let p = d.program("plain");
    d.begin();
    d.draw(&p, None, Topology::TriangleList, false);
    d.refused("no attribute at location 0");
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

/// An object's number as the recording back end names it.
fn n(id: &Id) -> String {
    format!("{id:?}")
        .trim_start_matches("Id(")
        .trim_end_matches(')')
        .into()
}

/// Every other call that reads or writes a buffer or texture, each with an object made without
/// the one usage that call needs.
#[test]
fn each_use_needs_its_usage_flag() {
    let all = COPIES | BufferUsage::VERTEX | BufferUsage::INDEX | BufferUsage::INDIRECT;
    let refused = |r: &Recorder, what: &str, kind: &str, id: &Id, flag: String| {
        assert_eq!(
            r.violations,
            [format!("{what} of {kind} {} made without {flag}", n(id))]
        );
    };
    let buffer_flag = |u: BufferUsage| format!("{u:?}");
    let texture_flag = |u: TextureUsage| format!("{u:?}");
    let side = |b| CopyBufferTexture {
        buffer: b,
        offset: 0,
        row_pitch: 256,
        rows_per_layer: 0,
    };

    let mut r = Recorder::new();
    let b = buffer(&mut r, 64, all.difference(BufferUsage::COPY_SRC));
    let _ = r.read_back(&ReadSource::Buffer {
        buffer: &b,
        offset: 0,
        size: 4,
    });
    refused(
        &r,
        "a read back",
        "buffer",
        &b,
        buffer_flag(BufferUsage::COPY_SRC),
    );

    let mut r = Recorder::new();
    let b = buffer(&mut r, 64, all);
    let less = buffer(&mut r, 64, all.difference(BufferUsage::COPY_SRC));
    r.copy_buffer(&less, 0, &b, 0, 4);
    refused(
        &r,
        "copy_buffer",
        "buffer",
        &less,
        buffer_flag(BufferUsage::COPY_SRC),
    );
    let mut r = Recorder::new();
    let b = buffer(&mut r, 64, all);
    let less = buffer(&mut r, 64, all.difference(BufferUsage::COPY_DST));
    r.copy_buffer(&b, 0, &less, 0, 4);
    refused(
        &r,
        "copy_buffer",
        "buffer",
        &less,
        buffer_flag(BufferUsage::COPY_DST),
    );

    let mut r = Recorder::new();
    let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    let less = buffer(&mut r, 1024, all.difference(BufferUsage::COPY_SRC));
    r.copy_buffer_to_texture(&side(&less), &region(&t), extent(2));
    refused(
        &r,
        "copy_buffer_to_texture",
        "buffer",
        &less,
        buffer_flag(BufferUsage::COPY_SRC),
    );
    let mut r = Recorder::new();
    let t = texture(
        &mut r,
        Format::Rgba8Unorm,
        2,
        ALL.difference(TextureUsage::COPY_DST),
    );
    let b = buffer(&mut r, 1024, all);
    r.copy_buffer_to_texture(&side(&b), &region(&t), extent(2));
    refused(
        &r,
        "copy_buffer_to_texture",
        "texture",
        &t,
        texture_flag(TextureUsage::COPY_DST),
    );

    let mut r = Recorder::new();
    let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    let less = buffer(&mut r, 1024, all.difference(BufferUsage::COPY_DST));
    r.copy_texture_to_buffer(&region(&t), &side(&less), extent(2));
    refused(
        &r,
        "copy_texture_to_buffer",
        "buffer",
        &less,
        buffer_flag(BufferUsage::COPY_DST),
    );
    let mut r = Recorder::new();
    let t = texture(
        &mut r,
        Format::Rgba8Unorm,
        2,
        ALL.difference(TextureUsage::COPY_SRC),
    );
    let b = buffer(&mut r, 1024, all);
    r.copy_texture_to_buffer(&region(&t), &side(&b), extent(2));
    refused(
        &r,
        "copy_texture_to_buffer",
        "texture",
        &t,
        texture_flag(TextureUsage::COPY_SRC),
    );

    let mut r = Recorder::new();
    let less = buffer(&mut r, 64, all.difference(BufferUsage::COPY_DST));
    r.clear_buffer(&less, 0, 4, &[0]);
    refused(
        &r,
        "clear_buffer",
        "buffer",
        &less,
        buffer_flag(BufferUsage::COPY_DST),
    );

    let mut r = Recorder::new();
    let t = texture(
        &mut r,
        Format::Rgba8Unorm,
        2,
        ALL.difference(TextureUsage::COPY_DST),
    );
    r.write_texture(&region(&t), extent(2), &[0; 16], 8, 2, WriteMode::Ordered);
    refused(
        &r,
        "write_texture",
        "texture",
        &t,
        texture_flag(TextureUsage::COPY_DST),
    );

    let mut r = Recorder::new();
    let less = texture(
        &mut r,
        Format::Rgba8Unorm,
        2,
        ALL.difference(TextureUsage::COPY_SRC),
    );
    let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    r.copy_texture(&region(&less), &region(&t), extent(2));
    refused(
        &r,
        "copy_texture",
        "texture",
        &less,
        texture_flag(TextureUsage::COPY_SRC),
    );
    let mut r = Recorder::new();
    let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    let less = texture(
        &mut r,
        Format::Rgba8Unorm,
        2,
        ALL.difference(TextureUsage::COPY_DST),
    );
    r.copy_texture(&region(&t), &region(&less), extent(2));
    refused(
        &r,
        "copy_texture",
        "texture",
        &less,
        texture_flag(TextureUsage::COPY_DST),
    );

    // Both sides of a resolve are render targets.
    let many = |r: &mut Recorder, usage| {
        r.create_texture(&TextureDesc {
            samples: 4,
            usage,
            ..TextureDesc {
                dimension: TextureDimension::D2,
                format: Format::Rgba8Unorm,
                width: 2,
                height: 2,
                depth_or_layers: 1,
                mip_levels: 1,
                samples: 1,
                usage,
                cube: false,
                reinterpret: false,
            }
        })
        .unwrap()
    };
    let mut r = Recorder::new();
    let src = many(&mut r, TextureUsage::SAMPLE);
    let dst = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    r.resolve(&region(&src), &region(&dst), Format::Rgba8Unorm);
    refused(
        &r,
        "resolve",
        "texture",
        &src,
        texture_flag(TextureUsage::RENDER),
    );
    let mut r = Recorder::new();
    let src = many(&mut r, TextureUsage::RENDER);
    let dst = texture(
        &mut r,
        Format::Rgba8Unorm,
        2,
        ALL.difference(TextureUsage::RENDER),
    );
    r.resolve(&region(&src), &region(&dst), Format::Rgba8Unorm);
    refused(
        &r,
        "resolve",
        "texture",
        &dst,
        texture_flag(TextureUsage::RENDER),
    );

    // A draw's index binding and indirect arguments.
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.spare = buffer(&mut d.r, 8, all.difference(BufferUsage::INDEX));
    let spare = d.spare;
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c, spare| {
        c.index = Some(IndexBinding {
            buffer: spare,
            offset: 0,
            format: IndexFormat::U16,
        });
        c.range = DrawRange::Indexed {
            first: 0,
            count: 3,
            base_vertex: 0,
            first_instance: 0,
            instances: 1,
        };
    });
    d.refused(&format!(
        "an index binding of buffer {} made without {}",
        n(&spare),
        buffer_flag(BufferUsage::INDEX)
    ));
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.spare = buffer(&mut d.r, 32, all.difference(BufferUsage::INDIRECT));
    let spare = d.spare;
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c, spare| {
        c.range = DrawRange::Indirect {
            args: spare,
            offset: 0,
        }
    });
    d.refused(&format!(
        "indirect arguments of buffer {} made without {}",
        n(&spare),
        buffer_flag(BufferUsage::INDIRECT)
    ));
}

/// A copy may go between regions of one subresource that do not overlap (apart in columns, in rows,
/// or in slices of a 3D texture), and between layers or levels; regions that meet in all of them
/// overlap.
#[test]
fn texture_copies_within_one_texture_do_not_overlap() {
    let mut r = Recorder::new();
    let flat = shaped(&mut r, TextureDimension::D2, 4, 2, 2);
    let volume = shaped(&mut r, TextureDimension::D3, 4, 4, 1);
    let at = |texture, x, y, z, layer, mip| TextureRegion {
        texture,
        mip,
        layer,
        origin: Origin { x, y, z },
        stencil: false,
    };
    let two = Extent {
        width: 2,
        height: 2,
        depth_or_layers: 1,
    };
    let deep = Extent {
        depth_or_layers: 2,
        ..two
    };
    let from = at(&flat, 0, 0, 0, 0, 0);
    r.copy_texture(&from, &at(&flat, 2, 2, 0, 0, 0), two);
    r.copy_texture(&from, &at(&flat, 2, 0, 0, 0, 0), two);
    r.copy_texture(&from, &at(&flat, 0, 2, 0, 0, 0), two);
    r.copy_texture(&from, &at(&flat, 0, 0, 0, 1, 0), two);
    r.copy_texture(&from, &at(&flat, 0, 0, 0, 0, 1), two);
    r.copy_texture(
        &at(&volume, 0, 0, 0, 0, 0),
        &at(&volume, 0, 0, 2, 0, 0),
        deep,
    );
    assert!(r.violations.is_empty());
    let overlap = "a copy between overlapping regions of one texture";
    r.copy_texture(&from, &at(&flat, 1, 1, 0, 0, 0), two);
    assert_eq!(r.violations, [overlap]);
    let mut r = Recorder::new();
    let volume = shaped(&mut r, TextureDimension::D3, 4, 4, 1);
    r.copy_texture(
        &at(&volume, 0, 0, 0, 0, 0),
        &at(&volume, 0, 0, 1, 0, 0),
        deep,
    );
    assert_eq!(r.violations, [overlap]);
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
            "view format neither the texture's nor one of its size it may take"
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
    // Ranges whose ends pass the largest u32 are refused, not wrapped.
    for (base_mip, base_layer) in [(u32::MAX, 0), (0, u32::MAX)] {
        assert_eq!(
            r.create_view(
                &t,
                &TextureViewDesc {
                    format: Format::Rgba8Unorm,
                    base_mip,
                    base_layer,
                    ..desc
                }
            ),
            Err(GpuError::Invalid("view range outside the texture"))
        );
    }
}

/// A 2 by 2 `rgba8unorm` texture made to be viewed in other formats.
fn reinterpreting(r: &mut Recorder) -> Id {
    r.create_texture(&TextureDesc {
        dimension: TextureDimension::D2,
        format: Format::Rgba8Unorm,
        width: 2,
        height: 2,
        depth_or_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage: ALL,
        cube: false,
        reinterpret: true,
    })
    .unwrap()
}

#[test]
fn views_in_other_formats_of_the_same_size_need_format_reinterpret() {
    let other = Err(GpuError::Invalid(
        "view format neither the texture's nor one of its size it may take",
    ));
    let desc = |format| TextureViewDesc {
        format,
        dimension: ViewDimension::D2,
        base_mip: 0,
        mip_count: 1,
        base_layer: 0,
        layer_count: 1,
        stencil: false,
    };
    let mut r = Recorder::new();
    let t = reinterpreting(&mut r);
    assert!(r.create_view(&t, &desc(Format::Rgba8Uint)).is_ok());
    assert!(r.create_view(&t, &desc(Format::R32Float)).is_ok());
    // Not another size, nor a depth format of the same size.
    assert_eq!(r.create_view(&t, &desc(Format::R8Unorm)), other);
    assert_eq!(r.create_view(&t, &desc(Format::Depth32Float)), other);

    let mut r = Recorder::with_caps(|c| c.features.remove(Features::FORMAT_REINTERPRET));
    let t = reinterpreting(&mut r);
    assert_eq!(
        r.create_view(&t, &desc(Format::R32Float)),
        Err(GpuError::Absent(Features::FORMAT_REINTERPRET))
    );
    // The sRGB twin needs no feature.
    assert!(r.create_view(&t, &desc(Format::Rgba8UnormSrgb)).is_ok());
}

/// A draw reads a texture through a view in another format by that format's meaning of the bytes.
#[test]
fn a_view_in_another_format_reads_the_bytes_as_that_format() {
    let mut d = Drawing::new();
    let t = reinterpreting(&mut d.r);
    let one = 1.0f32.to_le_bytes().repeat(4);
    d.r.write_texture(&region(&t), extent(2), &one, 8, 2, WriteMode::Ordered);
    let as_float = view(&mut d.r, &t, Format::R32Float);
    let set =
        d.r.create_binding_set(&[BindingEntry {
            binding: 32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(&as_float),
        }])
        .unwrap();
    let p = d.program("loads");
    d.begin();
    d.draw(&p, Some(&set), Topology::TriangleList, false);
    d.r.end_pass();
    assert!(d.r.violations.is_empty());
    assert_eq!(
        d.r.texture_bytes(d.target).unwrap(),
        [255, 0, 0, 255].repeat(4)
    );
}

/// Without `TEXTURE_VIEWS` a draw binds each texture through one mip range.
#[test]
fn a_draw_binds_a_texture_through_one_mip_range_without_texture_views() {
    for has in [true, false] {
        let mut d = Drawing::with_caps(|c| {
            if !has {
                c.features.remove(Features::TEXTURE_VIEWS);
            }
        });
        let t =
            d.r.create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format: Format::Rgba8Unorm,
                width: 4,
                height: 4,
                depth_or_layers: 1,
                mip_levels: 2,
                samples: 1,
                usage: ALL,
                cube: false,
                reinterpret: false,
            })
            .unwrap();
        let level = |r: &mut Recorder, base_mip| {
            let v = r
                .create_view(
                    &t,
                    &TextureViewDesc {
                        format: Format::Rgba8Unorm,
                        dimension: ViewDimension::D2,
                        base_mip,
                        mip_count: 1,
                        base_layer: 0,
                        layer_count: 1,
                        stencil: false,
                    },
                )
                .unwrap();
            r.create_binding_set(&[BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&v),
            }])
            .unwrap()
        };
        let (first, again, second) = (level(&mut d.r, 0), level(&mut d.r, 0), level(&mut d.r, 1));
        let p = d.program("loads");
        d.begin();
        let sets = |a, b| [Some(a), Some(b), None, None];
        // One range through two views is one range.
        d.draw_sets(
            &p,
            sets(&first, &again),
            Topology::TriangleList,
            false,
            |_, _| {},
        );
        assert!(d.r.violations.is_empty());
        d.draw_sets(
            &p,
            sets(&first, &second),
            Topology::TriangleList,
            false,
            |_, _| {},
        );
        if has {
            assert!(d.r.violations.is_empty());
        } else {
            d.refused("one texture through two mip ranges without TEXTURE_VIEWS");
        }
    }
}

#[test]
fn writes_copies_and_clears_stay_inside_their_buffers() {
    let mut r = Recorder::new();
    let b = buffer(&mut r, 16, COPIES);
    r.write_buffer(&b, 12, &[0; 8], WriteMode::Ordered);
    r.copy_buffer(&b, 0, &b, 4, 8);
    r.clear_buffer(&b, 0, 6, &[1, 2, 3, 4]);
    r.clear_buffer(&b, 0, 6, &[1, 2, 3]);
    assert_eq!(
        r.violations,
        [
            "write past the buffer's end",
            "copy between overlapping ranges of one buffer",
            "clear pattern of a size the interface does not take",
            "clear pattern of a size the interface does not take",
        ]
    );
}

#[test]
fn buffer_texture_copies_take_pitches_covering_the_region() {
    // A row of two texels is 8 bytes, and the region two rows: a pitch of 4 bytes, or of 1 row.
    for (row_pitch, rows_per_layer) in [(4, 2), (8, 1), (8, 2)] {
        let mut r = Recorder::new();
        let b = buffer(&mut r, 256, COPIES);
        let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
        r.copy_texture_to_buffer(
            &region(&t),
            &CopyBufferTexture {
                buffer: &b,
                offset: 0,
                row_pitch,
                rows_per_layer,
            },
            extent(2),
        );
        let want: &[&str] = if (row_pitch, rows_per_layer) == (8, 2) {
            &[]
        } else {
            &["copy pitch smaller than the region"]
        };
        assert_eq!(r.violations, want, "{row_pitch} {rows_per_layer}");
    }
}

#[test]
fn buffer_texture_copies_take_offsets_and_pitches_of_whole_blocks() {
    // 4-byte blocks of rgba8unorm; of depth24plus-stencil8, 4 bytes of depth and 1 of stencil.
    let pitch = "a buffer-texture copy's row pitch is not a whole number of blocks";
    let offset = "a buffer-texture copy's offset is not a whole number of blocks";
    let cases: [(Format, bool, u64, u32, &[&str]); 8] = [
        (Format::Rgba8Unorm, false, 4, 12, &[]),
        (Format::Rgba8Unorm, false, 0, 9, &[pitch]),
        (Format::Rgba8Unorm, false, 2, 8, &[offset]),
        (Format::Depth24PlusStencil8, false, 4, 12, &[]),
        (Format::Depth24PlusStencil8, false, 0, 10, &[pitch]),
        (Format::Depth24PlusStencil8, false, 2, 8, &[offset]),
        (Format::Depth24PlusStencil8, true, 0, 3, &[]),
        (Format::Depth24PlusStencil8, true, 1, 2, &[]),
    ];
    for (format, stencil, at, row_pitch, want) in cases {
        for to_texture in [false, true] {
            let mut r = Recorder::new();
            let b = buffer(&mut r, 64, COPIES);
            let t = texture(&mut r, format, 2, ALL);
            let side = CopyBufferTexture {
                buffer: &b,
                offset: at,
                row_pitch,
                rows_per_layer: 2,
            };
            let texels = TextureRegion {
                stencil,
                ..region(&t)
            };
            if to_texture {
                r.copy_buffer_to_texture(&side, &texels, extent(2));
            } else {
                r.copy_texture_to_buffer(&texels, &side, extent(2));
            }
            assert_eq!(
                r.violations, want,
                "{format:?} stencil {stencil} offset {at} pitch {row_pitch} to texture {to_texture}"
            );
        }
    }
}

#[test]
fn reads_back_stay_inside_their_buffers() {
    let mut r = Recorder::new();
    let b = buffer(&mut r, 16, COPIES);
    let read = |r: &mut Recorder, offset, size| {
        r.read_back(&ReadSource::Buffer {
            buffer: &b,
            offset,
            size,
        })
    };
    assert!(read(&mut r, 8, 8).is_ok());
    assert!(r.violations.is_empty());
    assert!(read(&mut r, 12, 8).is_err());
    assert_eq!(r.violations, ["read back past the buffer's end"]);
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
fn a_discard_write_gives_the_buffer_fresh_contents() {
    // After a discard, earlier commands keep the old bytes, so no-overwrite writes may follow over
    // the bytes they read; the bytes the discard did not write are undefined.
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw(&p, None, Topology::TriangleList, false);
    d.r.end_pass();
    let vertices = d.vertices;
    d.r.write_buffer(&vertices, 0, &[7; 4], WriteMode::Ordered);
    d.r.write_buffer(&vertices, 4, &[1; 4], WriteMode::Discard);
    d.r.write_buffer(&vertices, 0, &[2; 4], WriteMode::NoOverwrite);
    d.r.write_buffer(&vertices, 8, &[3; 8], WriteMode::NoOverwrite);
    assert!(d.r.violations.is_empty(), "{:?}", d.r.violations);
    let at: usize = n(&vertices).parse().unwrap();
    let got = d.r.contents().0[at].clone().unwrap();
    assert_eq!(got[..16], [2, 2, 2, 2, 1, 1, 1, 1, 3, 3, 3, 3, 3, 3, 3, 3]);
    assert_eq!(
        got[16..],
        [0xcd; 8],
        "bytes no write since the discard made"
    );
    // A draw after the discard counts again.
    d.begin();
    d.draw(&p, None, Topology::TriangleList, false);
    d.r.end_pass();
    d.r.write_buffer(&vertices, 20, &[0; 4], WriteMode::NoOverwrite);
    assert_eq!(d.r.violations.len(), 1);
}

#[test]
fn a_discard_texture_write_gives_the_texture_fresh_contents() {
    // A 2 by 2 texture of two levels: a copy reads level 0, then a discard writes one texel of it.
    let mut r = Recorder::new();
    let t = r
        .create_texture(&TextureDesc {
            mip_levels: 2,
            ..texture_desc(Format::Rgba8Unorm, 2)
        })
        .unwrap();
    let b = buffer(&mut r, 16, COPIES);
    let level = |mip| TextureRegion { mip, ..region(&t) };
    r.write_texture(&level(0), extent(2), &[9; 16], 8, 2, WriteMode::Ordered);
    r.write_texture(&level(1), extent(1), &[8; 4], 4, 1, WriteMode::Ordered);
    let side = CopyBufferTexture {
        buffer: &b,
        offset: 0,
        row_pitch: 8,
        rows_per_layer: 2,
    };
    r.copy_texture_to_buffer(&level(0), &side, extent(2));
    let corner = TextureRegion {
        origin: Origin { x: 1, y: 1, z: 0 },
        ..region(&t)
    };
    r.write_texture(&corner, extent(1), &[1; 4], 4, 1, WriteMode::Discard);
    // Nothing earlier uses the fresh texture, so no-overwrite writes may follow over the copy's texels.
    r.write_texture(
        &region(&t),
        extent(1),
        &[2; 4],
        4,
        1,
        WriteMode::NoOverwrite,
    );
    assert!(r.violations.is_empty(), "{:?}", r.violations);
    let (objects, _) = r.contents();
    let mut want = [0xcd; 20];
    want[..4].copy_from_slice(&[2; 4]);
    want[12..16].copy_from_slice(&[1; 4]);
    assert_eq!(
        objects[n(&t).parse::<usize>().unwrap()].as_deref(),
        Some(&want[..])
    );
    assert_eq!(
        objects[n(&b).parse::<usize>().unwrap()].as_deref(),
        Some(&[9; 16][..]),
        "the copy recorded before the discard"
    );
}

#[test]
fn a_no_overwrite_texture_write_keeps_off_texels_in_use() {
    let refusal = |t: &Id| {
        format!(
            "a NoOverwrite write over texels of texture {} an unfinished command or an ordered write touches",
            n(t)
        )
    };
    fn at(t: &Id, x: u32, y: u32) -> TextureRegion<'_, Recorder> {
        TextureRegion {
            origin: Origin { x, y, z: 0 },
            ..region(t)
        }
    }
    let one = extent(1);
    // An ordered write not waited for, then texels beside it and over it.
    let mut r = Recorder::new();
    let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    r.write_texture(&at(&t, 0, 0), one, &[1; 4], 4, 1, WriteMode::Ordered);
    r.write_texture(&at(&t, 1, 0), one, &[2; 4], 4, 1, WriteMode::NoOverwrite);
    r.write_texture(&at(&t, 0, 1), one, &[2; 4], 4, 1, WriteMode::NoOverwrite);
    assert!(r.violations.is_empty(), "{:?}", r.violations);
    r.write_texture(&at(&t, 0, 0), one, &[2; 4], 4, 1, WriteMode::NoOverwrite);
    assert_eq!(r.violations, [refusal(&t)]);
    // Each command that reads or writes texel (1, 1), then writes beside it and over it; and over
    // it after a wait.
    type Use = fn(&mut Recorder, &Id, &Id, &Id);
    let uses: [(&str, Use); 5] = [
        ("copy_texture_to_buffer", |r, t, _, b| {
            r.copy_texture_to_buffer(
                &TextureRegion {
                    origin: Origin { x: 1, y: 1, z: 0 },
                    ..region(t)
                },
                &CopyBufferTexture {
                    buffer: b,
                    offset: 0,
                    row_pitch: 4,
                    rows_per_layer: 1,
                },
                extent(1),
            )
        }),
        ("copy_buffer_to_texture", |r, t, _, b| {
            r.copy_buffer_to_texture(
                &CopyBufferTexture {
                    buffer: b,
                    offset: 0,
                    row_pitch: 4,
                    rows_per_layer: 1,
                },
                &TextureRegion {
                    origin: Origin { x: 1, y: 1, z: 0 },
                    ..region(t)
                },
                extent(1),
            )
        }),
        ("copy_texture from", |r, t, other, _| {
            r.copy_texture(
                &TextureRegion {
                    origin: Origin { x: 1, y: 1, z: 0 },
                    ..region(t)
                },
                &region(other),
                extent(1),
            )
        }),
        ("copy_texture to", |r, t, other, _| {
            r.copy_texture(
                &region(other),
                &TextureRegion {
                    origin: Origin { x: 1, y: 1, z: 0 },
                    ..region(t)
                },
                extent(1),
            )
        }),
        ("read_back", |r, t, _, _| {
            r.read_back(&ReadSource::Texture {
                region: TextureRegion {
                    origin: Origin { x: 1, y: 1, z: 0 },
                    ..region(t)
                },
                extent: extent(1),
            })
            .unwrap();
        }),
    ];
    for (name, use_texels) in uses {
        for wait in [false, true] {
            let mut r = Recorder::new();
            let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
            let other = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
            let b = buffer(&mut r, 64, COPIES);
            use_texels(&mut r, &t, &other, &b);
            if wait {
                let done = r.submit();
                r.wait_fence(&done);
            }
            r.write_texture(&at(&t, 0, 1), one, &[2; 4], 4, 1, WriteMode::NoOverwrite);
            assert!(r.violations.is_empty(), "{name}: {:?}", r.violations);
            r.write_texture(&at(&t, 1, 1), one, &[2; 4], 4, 1, WriteMode::NoOverwrite);
            let want = if wait { vec![] } else { vec![refusal(&t)] };
            assert_eq!(r.violations, want, "{name}, waited {wait}");
        }
    }
    // A resolve writes all of its destination.
    let mut r = Recorder::new();
    let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    let ms = r
        .create_texture(&TextureDesc {
            samples: 4,
            usage: TextureUsage::RENDER,
            ..texture_desc(Format::Rgba8Unorm, 2)
        })
        .unwrap();
    r.resolve(&region(&ms), &region(&t), Format::Rgba8Unorm);
    r.write_texture(&at(&t, 1, 1), one, &[2; 4], 4, 1, WriteMode::NoOverwrite);
    assert_eq!(r.violations, [refusal(&t)]);
    // A pass attaching slice 1 of level 1 (2 by 2 by 2) of a 3D texture uses that slice alone.
    let mut r = Recorder::new();
    let t = shaped(&mut r, TextureDimension::D3, 4, 4, 2);
    let level = view_of(&mut r, &t, ViewDimension::D3, (1, 1), (0, 1));
    r.begin_pass(&pass_of([ColorAttachment {
        depth_slice: 1,
        ..color(&level)
    }]));
    r.end_pass();
    let slice = |z| TextureRegion {
        mip: 1,
        origin: Origin { x: 0, y: 0, z },
        ..region(&t)
    };
    r.write_texture(&slice(0), extent(2), &[2; 16], 8, 2, WriteMode::NoOverwrite);
    assert!(r.violations.is_empty(), "{:?}", r.violations);
    r.write_texture(&slice(1), extent(2), &[2; 16], 8, 2, WriteMode::NoOverwrite);
    assert_eq!(r.violations, [refusal(&t)]);
    // A draw sampling the texture, and a pass attaching it, use all its texels.
    let mut d = Drawing::new();
    let sampled = texture(&mut d.r, Format::Rgba8Unorm, 2, ALL);
    let sampled_view = view(&mut d.r, &sampled, Format::Rgba8Unorm);
    let set =
        d.r.create_binding_set(&[BindingEntry {
            binding: 32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(&sampled_view),
        }])
        .unwrap();
    let p = d.program("loads");
    d.begin();
    d.draw(&p, Some(&set), Topology::TriangleList, false);
    d.r.end_pass();
    let target = d.target;
    for t in [&sampled, &target] {
        d.r.write_texture(&at(t, 1, 1), one, &[2; 4], 4, 1, WriteMode::NoOverwrite);
    }
    assert_eq!(d.r.violations, [refusal(&sampled), refusal(&target)]);
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
        assert_eq!(
            r.create_texture(&broken),
            Err(GpuError::Invalid(
                "a multisampled texture that is not 2D with one level and one layer, or is storage"
            )),
            "{broken:?}"
        );
    }
    // A count the format does not take.
    assert_eq!(
        r.create_texture(&TextureDesc { samples: 2, ..desc }),
        Err(GpuError::Invalid(
            "texture sample count the format does not take"
        ))
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
    // Whole on one side only: all of a 2 by 2 into part of a 4 by 4, and part of the 4 by 4 into
    // all of the 2 by 2.
    for small_to_large in [true, false] {
        let mut r = Recorder::new();
        let small = multisampled(&mut r, Format::Rgba8Unorm, 2).unwrap();
        let large = multisampled(&mut r, Format::Rgba8Unorm, 4).unwrap();
        let (src, dst) = if small_to_large {
            (&small, &large)
        } else {
            (&large, &small)
        };
        r.copy_texture(&region(src), &region(dst), extent(2));
        assert_eq!(r.violations, [not_whole], "{small_to_large}");
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

    // The depth attachment too.
    let mut r = Recorder::new();
    let ms = multisampled(&mut r, Format::Depth32Float, 2).unwrap();
    let ms = view(&mut r, &ms, Format::Depth32Float);
    let single = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    let single = view(&mut r, &single, Format::Rgba8Unorm);
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: Some(DepthAttachment {
            view: &ms,
            depth: Some(AspectOps {
                load: LoadOp::Load,
                store: StoreOp::Store,
                read_only: false,
            }),
            stencil: None,
        }),
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &single,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Load,
        store: StoreOp::Store,
    });
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
    let cases: [(&str, Breaks); 8] = [
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
            "a resolve from a single-sampled attachment or into a multisampled one",
            |r| {
                let a = multisampled(r, Format::Rgba8Unorm, 2).unwrap();
                let a = view(r, &a, Format::Rgba8Unorm);
                let b = multisampled(r, Format::Rgba8Unorm, 2).unwrap();
                let b = view(r, &b, Format::Rgba8Unorm);
                pass_resolving(r, &a, &b);
            },
        ),
        // The destination outside the source's format and its twin, with the format named the
        // source's.
        (
            "a resolve between formats other than one format and its sRGB twin",
            |r| {
                let a = multisampled(r, Format::Rgba8Unorm, 2).unwrap();
                let b = texture(r, Format::Bgra8Unorm, 2, ALL);
                r.resolve(&region(&a), &region(&b), Format::Rgba8Unorm);
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
    // `resolve` from one sample, or into many.
    for samples in [(1, 1), (4, 4)] {
        let mut r = Recorder::new();
        let mut make = |n| {
            if n > 1 {
                multisampled(&mut r, Format::Rgba8Unorm, 2).unwrap()
            } else {
                texture(&mut r, Format::Rgba8Unorm, 2, ALL)
            }
        };
        let (a, b) = (make(samples.0), make(samples.1));
        r.resolve(&region(&a), &region(&b), Format::Rgba8Unorm);
        assert_eq!(
            r.violations,
            ["a resolve from a single-sampled texture or into a multisampled one"]
        );
    }
    // `resolve` into level 1 of a 4 by 4 texture of two layers and two levels fits; past its levels
    // (level 2 of an 8 by 8 one, whose size would fit) or layers, or from a region other than the
    // source's one level and layer, it does not.
    let at = [
        ([0, 0, 1, 1], 4, true),
        ([0, 0, 2, 0], 8, false),
        ([0, 0, 1, 2], 4, false),
        ([1, 0, 1, 0], 4, false),
        ([0, 1, 1, 0], 4, false),
    ];
    for ([src_mip, src_layer, mip, layer], size, fits) in at {
        let mut r = Recorder::new();
        let a = multisampled(&mut r, Format::Rgba8Unorm, 2).unwrap();
        let b = shaped(&mut r, TextureDimension::D2, size, 2, 2);
        let src = TextureRegion {
            mip: src_mip,
            layer: src_layer,
            ..region(&a)
        };
        let dst = TextureRegion {
            mip,
            layer,
            ..region(&b)
        };
        r.resolve(&src, &dst, Format::Rgba8Unorm);
        let want: &[&str] = if fits {
            &[]
        } else {
            &["a resolve between subresources of different sizes"]
        };
        assert_eq!(r.violations, want, "{src_mip} {src_layer} {mip} {layer}");
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
    d.draw_with(&p, None, Topology::TriangleList, false, |c, _| {
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
    let cases: [(Features, Change); 11] = [
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
        (Features::POLYGON_MODE, |d| d.fill = FillMode::Line),
        (Features::POLYGON_MODE_POINT, |d| d.fill = FillMode::Point),
        (Features::DEPTH_CLAMP, |d| d.depth_clip = false),
        (Features::CLIP_DEPTH_MINUS_ONE, |d| d.depth_minus_one = true),
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
        bind(&mut r, storage_at(256, 6)),
        Err(GpuError::Invalid(
            "a storage binding whose size is not a multiple of four"
        ))
    );
    assert!(
        bind(
            &mut r,
            BindingResource::UniformBuffer(range(&uniform, 0, 6))
        )
        .is_ok()
    );
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
    let mut r = Recorder::with_caps(|c| c.features.remove(Features::VERTEX_STORAGE_READ));
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
    let mut r = Recorder::with_caps(|c| {
        let at = Format::R32Float.index();
        c.formats.uses[at].remove(FormatUse::SAMPLE);
    });
    assert_eq!(
        r.create_texture(&desc(Format::R32Float, TextureUsage::SAMPLE)),
        refused
    );
    assert!(
        r.create_texture(&desc(Format::R32Float, TextureUsage::RENDER))
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
    d.draw_with(&p, None, Topology::TriangleList, false, |c, _| {
        c.viewports = &REVERSED
    });
    d.refused("a viewport whose min_depth is above its max_depth");
}

/// WebGPU's bounds with the recorder's `max_texture_2d` of 16384: sizes to 16384, edges from -32768
/// to 32767, depths 0 to 1.
#[test]
fn viewports_keep_within_their_bounds() {
    let vp = |x: f32, y: f32, width: f32, height: f32, depths: [f32; 2]| Viewport {
        x: x.to_bits(),
        y: y.to_bits(),
        width: width.to_bits(),
        height: height.to_bits(),
        min_depth: depths[0].to_bits(),
        max_depth: depths[1].to_bits(),
    };
    let run = |v: Viewport| {
        let mut d = Drawing::new();
        let p = d.program("plain");
        d.begin();
        // The draw's changes take borrows of any lifetime; one viewport a case is leaked.
        let one: &'static [Viewport] = Box::leak(Box::new([v]));
        d.draw_with(&p, None, Topology::TriangleList, false, |c, _| {
            c.viewports = one
        });
        d
    };
    let unit = [0.0, 1.0];
    for v in [
        vp(0.0, 0.0, 16384.0, 16384.0, unit),
        vp(16383.0, 16383.0, 16384.0, 16384.0, unit),
        vp(-32768.0, -32768.0, 16384.0, 16384.0, unit),
        vp(0.0, 0.0, 0.0, 0.0, [0.5, 0.5]),
    ] {
        let d = run(v);
        assert!(d.r.violations.is_empty(), "{v:?}: {:?}", d.r.violations);
    }
    for v in [
        vp(0.0, 0.0, 16385.0, 4.0, unit),
        vp(0.0, 0.0, 4.0, 16385.0, unit),
        vp(16384.0, 0.0, 16384.0, 4.0, unit),
        vp(0.0, 16384.0, 4.0, 16384.0, unit),
        vp(-32769.0, 0.0, 4.0, 4.0, unit),
        vp(0.0, -32769.0, 4.0, 4.0, unit),
        vp(0.0, 0.0, -1.0, 4.0, unit),
        vp(0.0, 0.0, f32::NAN, 4.0, unit),
    ] {
        run(v).refused("a viewport past its bounds, by max_texture_2d");
    }
    for depths in [[-0.5, 1.0], [0.0, 1.5]] {
        run(vp(0.0, 0.0, 4.0, 4.0, depths)).refused("a viewport depth outside 0 to 1");
    }
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
    d.draw_with(&p, None, Topology::TriangleList, false, |c, _| {
        c.viewports = &HALF
    });
    d.refused("a fractional viewport without FRACTIONAL_VIEWPORT");
}

#[test]
fn draw_bases_need_base_vertex_instance() {
    let mut d = Drawing::with_caps(|c| c.features.remove(Features::BASE_VERTEX_INSTANCE));
    let p = d.program("plain");
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c, _| {
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
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c, spare| {
        c.index = Some(IndexBinding {
            buffer: spare,
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
    d.draw_with(&p, None, Topology::TriangleList, false, |c, _| {
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
    // A read back copies out too.
    let mut r = Recorder::with_caps(|c| {
        c.formats.uses[Format::Rgba8Unorm.index()].remove(FormatUse::COPY_OUT)
    });
    let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    let _ = r.read_back(&ReadSource::Texture {
        region: region(&t),
        extent: extent(2),
    });
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

    // A copy between textures moves both aspects, over a copy holding other values.
    let copy = texture(&mut r, Format::Depth24PlusStencil8, 2, ALL);
    let copy_stencil = TextureRegion {
        stencil: true,
        ..region(&copy)
    };
    r.write_texture(&copy_stencil, extent(2), &[9; 4], 2, 0, WriteMode::Ordered);
    let other_depth = [0x77, 0x77, 0x77, 0].repeat(4);
    r.write_texture(
        &region(&copy),
        extent(2),
        &other_depth,
        8,
        0,
        WriteMode::Ordered,
    );
    r.copy_texture(&region(&t), &region(&copy), extent(2));
    let mut read = |region, len| {
        let rb = r
            .read_back(&ReadSource::Texture {
                region,
                extent: extent(2),
            })
            .unwrap();
        r.submit();
        r.wait_read(&rb);
        let mut got = vec![0u8; len];
        r.finish_read(rb, &mut got);
        got
    };
    assert_eq!(read(copy_stencil, 4), [1, 2, 3, 4]);
    assert_eq!(read(region(&copy), 16), depth);
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

/// A colour attachment that loads and stores `view`.
fn color(view: &Id) -> ColorAttachment<'_, Recorder> {
    ColorAttachment {
        view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Load,
        store: StoreOp::Store,
    }
}

fn pass_of<'a, const N: usize>(
    colors: [ColorAttachment<'a, Recorder>; N],
) -> PassDesc<'a, Recorder> {
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    for (slot, c) in pass.colors.iter_mut().zip(colors) {
        *slot = Some(c);
    }
    pass
}

/// An `rgba8unorm` texture of the shape given, with every usage but storage and one sample.
fn shaped(
    r: &mut Recorder,
    dimension: TextureDimension,
    size: u32,
    depth_or_layers: u32,
    mips: u32,
) -> Id {
    r.create_texture(&TextureDesc {
        dimension,
        format: Format::Rgba8Unorm,
        width: size,
        height: size,
        depth_or_layers,
        mip_levels: mips,
        samples: 1,
        usage: ALL,
        cube: false,
        reinterpret: false,
    })
    .unwrap()
}

fn view_of(
    r: &mut Recorder,
    t: &Id,
    dimension: ViewDimension,
    (base_mip, mip_count): (u32, u32),
    (base_layer, layer_count): (u32, u32),
) -> Id {
    r.create_view(
        t,
        &TextureViewDesc {
            format: Format::Rgba8Unorm,
            dimension,
            base_mip,
            mip_count,
            base_layer,
            layer_count,
            stencil: false,
        },
    )
    .unwrap()
}

#[test]
fn an_attachment_view_is_one_level_and_one_layer() {
    let one = "an attachment view must be one level and one layer";
    let mut r = Recorder::new();
    let t = shaped(&mut r, TextureDimension::D2, 4, 1, 2);
    let levels = view_of(&mut r, &t, ViewDimension::D2, (0, 2), (0, 1));
    r.begin_pass(&pass_of([color(&levels)]));
    assert_eq!(r.violations, [one]);

    let mut r = Recorder::new();
    let t = shaped(&mut r, TextureDimension::D2, 4, 2, 1);
    let layers = view_of(&mut r, &t, ViewDimension::D2Array, (0, 1), (0, 2));
    r.begin_pass(&pass_of([color(&layers)]));
    assert_eq!(r.violations, [one]);
}

/// A 3D texture's depth halves with each level: level 1 of a 4-deep texture has slices 0 and 1.
#[test]
fn a_3d_attachment_slice_lies_inside_its_levels_depth() {
    let mut r = Recorder::new();
    let t = shaped(&mut r, TextureDimension::D3, 4, 4, 2);
    let level = view_of(&mut r, &t, ViewDimension::D3, (1, 1), (0, 1));
    let slice = |depth_slice| ColorAttachment {
        depth_slice,
        ..color(&level)
    };
    r.begin_pass(&pass_of([slice(1)]));
    r.end_pass();
    assert!(r.violations.is_empty());
    r.begin_pass(&pass_of([slice(2)]));
    assert_eq!(r.violations, ["an attachment's slice outside its level"]);
}

#[test]
fn a_subresource_is_attached_once() {
    let mut d = Drawing::new();
    let v = d.target_view;
    d.r.begin_pass(&pass_of([color(&v), color(&v)]));
    d.refused("one subresource attached twice");

    // Two resolves into one view.
    let mut r = Recorder::new();
    let a = multisampled(&mut r, Format::Rgba8Unorm, 2).unwrap();
    let a = view(&mut r, &a, Format::Rgba8Unorm);
    let b = multisampled(&mut r, Format::Rgba8Unorm, 2).unwrap();
    let b = view(&mut r, &b, Format::Rgba8Unorm);
    let into = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    let into = view(&mut r, &into, Format::Rgba8Unorm);
    let resolved = |v| ColorAttachment {
        resolve: Some(&into),
        ..color(v)
    };
    r.begin_pass(&pass_of([resolved(&a), resolved(&b)]));
    assert_eq!(r.violations, ["one subresource attached twice"]);
}

#[test]
fn a_pass_has_attachments_or_empty_but_not_both() {
    let empty = EmptyPass {
        width: 2,
        height: 2,
        samples: 1,
    };
    let mut d = Drawing::new();
    let v = d.target_view;
    d.r.begin_pass(&PassDesc {
        empty: Some(empty),
        ..pass_of([color(&v)])
    });
    d.refused("a pass with attachments and `empty`");

    let mut r = Recorder::new();
    r.begin_pass(&pass_of([]));
    assert_eq!(r.violations, ["a pass with no attachment and no `empty`"]);
    // `empty` alone is a pass.
    let mut r = Recorder::new();
    r.begin_pass(&PassDesc {
        empty: Some(empty),
        ..pass_of([])
    });
    r.end_pass();
    assert!(r.violations.is_empty());
}

#[test]
fn the_depth_attachment_has_a_depth_format() {
    let mut d = Drawing::new();
    let other = texture(&mut d.r, Format::Rgba8Unorm, 2, ALL);
    let other = view(&mut d.r, &other, Format::Rgba8Unorm);
    let v = d.target_view;
    d.r.begin_pass(&PassDesc {
        depth: Some(DepthAttachment {
            view: &other,
            depth: Some(AspectOps {
                load: LoadOp::Load,
                store: StoreOp::Store,
                read_only: false,
            }),
            stencil: None,
        }),
        ..pass_of([color(&v)])
    });
    d.refused("a colour format as the depth attachment");
}

/// The depth attachment names exactly the aspects its format has (the stencil aspect's half is in
/// `tests/recorder.rs`).
#[test]
fn the_depth_attachment_names_depth_exactly_when_its_format_has_it() {
    let cases = [
        (Format::Depth32Float, false, false, false),
        (Format::Depth32Float, true, false, true),
        (Format::Stencil8, true, true, false),
        (Format::Stencil8, false, true, true),
    ];
    for (format, depth, stencil, fits) in cases {
        let depth = depth.then_some(AspectOps {
            load: LoadOp::Load,
            store: StoreOp::Store,
            read_only: false,
        });
        let stencil = stencil.then_some(AspectOps {
            load: LoadOp::Load,
            store: StoreOp::Store,
            read_only: false,
        });
        let mut d = Drawing::new();
        let t = texture(&mut d.r, format, 2, TextureUsage::RENDER);
        let v = view(&mut d.r, &t, format);
        let c = d.target_view;
        d.r.begin_pass(&PassDesc {
            depth: Some(DepthAttachment {
                view: &v,
                depth,
                stencil,
            }),
            ..pass_of([color(&c)])
        });
        let want: &[&str] = if fits {
            &[]
        } else {
            &["depth attachment aspects differ from the format's"]
        };
        assert_eq!(d.r.violations, want, "{format:?}");
    }
}

/// A depth-stencil block that writes depth (compare `Always`) or stencil (replace on pass).
fn writing(d: &mut Drawing, depth: bool, stencil: bool) -> Id {
    let face = StencilFace {
        compare: CompareFn::Always,
        fail: StencilOp::Keep,
        depth_fail: StencilOp::Keep,
        pass: if stencil {
            StencilOp::Replace
        } else {
            StencilOp::Keep
        },
    };
    d.r.create_depth_stencil(&DepthStencilDesc {
        depth_compare: Some(CompareFn::Always),
        depth_write: depth,
        stencil: Some((face, face)),
        stencil_read_mask: [0xff; 2],
        stencil_write_mask: [0xff; 2],
        depth_bounds: None,
    })
    .unwrap()
}

/// Draws with `block` in a pass whose depth-stencil attachment has its depth or its stencil
/// read-only.
fn draw_read_only(depth_read_only: bool, depth: bool, stencil: bool) -> Drawing {
    let mut d = Drawing::new();
    let ds = texture(&mut d.r, Format::Depth24PlusStencil8, 2, ALL);
    let ds = view(&mut d.r, &ds, Format::Depth24PlusStencil8);
    let block = writing(&mut d, depth, stencil);
    d.depth = block;
    let p = d.program("plain");
    let v = d.target_view;
    d.r.begin_pass(&PassDesc {
        depth: Some(DepthAttachment {
            view: &ds,
            depth: Some(AspectOps {
                load: LoadOp::Load,
                store: StoreOp::Store,
                read_only: depth_read_only,
            }),
            stencil: Some(AspectOps {
                load: LoadOp::Load,
                store: StoreOp::Store,
                read_only: !depth_read_only,
            }),
        }),
        ..pass_of([color(&v)])
    });
    d.draw(&p, None, Topology::TriangleList, false);
    d
}

#[test]
fn a_draw_writes_no_read_only_aspect() {
    draw_read_only(true, true, false).refused("a draw writing depth into a read-only attachment");
    draw_read_only(false, false, true)
        .refused("a draw writing stencil into a read-only attachment");
    // Each aspect written where the other is read-only.
    for (depth_read_only, depth, stencil) in [(true, false, true), (false, true, false)] {
        let mut d = draw_read_only(depth_read_only, depth, stencil);
        d.r.end_pass();
        assert!(d.r.violations.is_empty());
    }
}

#[test]
fn a_pass_ends_with_no_query_running() {
    let mut d = Drawing::new();
    let set = d.r.create_query_set(QueryKind::OcclusionAny, 1).unwrap();
    let v = d.target_view;
    d.r.begin_pass(&PassDesc {
        occlusion: Some(&set),
        ..pass_of([color(&v)])
    });
    d.r.begin_query(0);
    d.r.end_pass();
    d.refused("a pass ended with its query running");
}

#[test]
fn present_happens_outside_a_pass() {
    let mut d = Drawing::new();
    let s = d.r.create_surface(1).unwrap();
    d.r.configure_surface(&s, Format::Rgba8Unorm, 2, 2, true)
        .unwrap();
    d.r.current_texture(&s).unwrap();
    d.begin();
    let _ = d.r.present(&s);
    // Presenting submits the frame's work, so the submit is refused too.
    assert_eq!(
        d.r.violations,
        ["present inside a pass", "submit inside a pass"]
    );
}

#[test]
fn a_surface_takes_a_format_it_can_present() {
    let mut r = Recorder::new();
    let s = r.create_surface(1).unwrap();
    assert_eq!(
        r.configure_surface(&s, Format::R32Float, 2, 2, true),
        Err(GpuError::Invalid("surface format without PRESENT"))
    );
}

#[test]
fn regions_lie_inside_their_texture() {
    let write = |r: &mut Recorder, t: &Id, mip, layer, origin, n: u32, layers: u32| {
        r.write_texture(
            &TextureRegion {
                texture: t,
                mip,
                layer,
                origin,
                stencil: false,
            },
            Extent {
                width: n,
                height: n,
                depth_or_layers: layers,
            },
            &vec![0; (n * n * 4 * layers) as usize],
            n * 4,
            n,
            WriteMode::Ordered,
        );
    };
    let at = |x, y, z| Origin { x, y, z };
    let mut r = Recorder::new();
    let flat = shaped(&mut r, TextureDimension::D2, 4, 2, 2);
    let deep = shaped(&mut r, TextureDimension::D3, 4, 4, 2);
    // Inside: level 1 is 2 by 2 (by 2 slices for the 3D one).
    write(&mut r, &flat, 1, 1, at(0, 0, 0), 2, 1);
    write(&mut r, &deep, 1, 0, at(0, 0, 1), 2, 1);
    assert!(r.violations.is_empty());
    write(&mut r, &flat, 2, 0, at(0, 0, 0), 1, 1);
    write(&mut r, &flat, 0, 1, at(0, 0, 0), 4, 2);
    write(&mut r, &deep, 1, 0, at(0, 0, 1), 2, 2);
    write(&mut r, &flat, 1, 0, at(1, 0, 0), 2, 1);
    write(&mut r, &flat, 1, 0, at(0, 1, 0), 2, 1);
    assert_eq!(
        r.violations,
        [
            "a region past the last level",
            "a region past the last layer",
            "a region past the last slice",
            "a region outside its level or off the block grid",
            "a region outside its level or off the block grid",
        ]
    );
    // Ends past the largest u32 are refused, not wrapped.
    let one = |r: &mut Recorder, t: &Id, layer, origin, width, layers: u32| {
        r.write_texture(
            &TextureRegion {
                texture: t,
                mip: 0,
                layer,
                origin,
                stencil: false,
            },
            Extent {
                width,
                height: 1,
                depth_or_layers: layers,
            },
            &vec![0; (width * 4 * layers) as usize],
            width * 4,
            1,
            WriteMode::Ordered,
        );
    };
    let mut r = Recorder::new();
    let flat = shaped(&mut r, TextureDimension::D2, 4, 2, 2);
    let deep = shaped(&mut r, TextureDimension::D3, 4, 4, 2);
    one(&mut r, &flat, 0, at(u32::MAX - 1, 0, 0), 2, 1);
    one(&mut r, &flat, u32::MAX, at(0, 0, 0), 1, 2);
    one(&mut r, &deep, 0, at(0, 0, u32::MAX), 1, 2);
    assert_eq!(
        r.violations,
        [
            "a region outside its level or off the block grid",
            "a region past the last layer",
            "a region past the last slice",
        ]
    );
}

#[test]
fn a_load_names_a_sample_exactly_when_its_texture_is_multisampled() {
    let mismatch =
        "a load naming a sample from a single-sampled texture, or none from a multisampled one";
    for (fragment, binding, samples) in [("loads", 32, 4), ("loads_sample", 33, 1)] {
        let mut d = Drawing::new();
        let t = if samples > 1 {
            multisampled(&mut d.r, Format::Rgba8Unorm, 2).unwrap()
        } else {
            texture(&mut d.r, Format::Rgba8Unorm, 2, ALL)
        };
        let v = view(&mut d.r, &t, Format::Rgba8Unorm);
        let set =
            d.r.create_binding_set(&[BindingEntry {
                binding,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&v),
            }])
            .unwrap();
        let p = d.program(fragment);
        d.begin();
        d.draw(&p, Some(&set), Topology::TriangleList, false);
        d.refused(mismatch);
    }
}

#[test]
fn buffers_and_textures_stay_within_the_size_limits() {
    let mut r = Recorder::with_caps(|c| {
        c.limits.max_buffer_size = 64;
        c.limits.max_texture_2d = 8;
        c.limits.max_texture_cube = 4;
        c.limits.max_texture_3d = 2;
        c.limits.max_texture_layers = 6;
    });
    let buffer = |size| BufferDesc {
        size,
        usage: COPIES,
        intent: MapIntent::Rarely,
    };
    assert!(r.create_buffer(&buffer(64)).is_ok());
    assert_eq!(
        r.create_buffer(&buffer(65)),
        Err(GpuError::Invalid("buffer larger than the limit"))
    );
    let desc = |dimension, width, height, depth_or_layers, cube| TextureDesc {
        dimension,
        format: Format::Rgba8Unorm,
        width,
        height,
        depth_or_layers,
        mip_levels: 1,
        samples: 1,
        usage: ALL,
        cube,
        reinterpret: false,
    };
    use TextureDimension::{D2, D3};
    for fits in [
        desc(D2, 8, 8, 6, false),
        desc(D2, 4, 4, 6, true),
        desc(D3, 2, 2, 2, false),
    ] {
        assert!(r.create_texture(&fits).is_ok(), "{fits:?}");
    }
    for past in [
        desc(D2, 9, 1, 1, false),
        desc(D2, 1, 9, 1, false),
        desc(D2, 1, 1, 7, false),
        desc(D2, 8, 8, 6, true),
        desc(D3, 3, 1, 1, false),
        desc(D3, 1, 1, 3, false),
    ] {
        assert_eq!(
            r.create_texture(&past),
            Err(GpuError::Invalid("a texture past the size limits")),
            "{past:?}"
        );
    }
}

#[test]
fn vertex_layouts_stay_within_the_limits() {
    let mut r = Recorder::with_caps(|c| {
        c.limits.vertex_streams = 2;
        c.limits.vertex_attributes = 2;
        c.limits.max_vertex_stride = 16;
    });
    let stream = |stride| {
        Some(VertexStream {
            stride,
            step: StepMode::Vertex,
        })
    };
    let attribute = |location, stream| {
        Some(VertexAttribute {
            location,
            stream,
            offset: 0,
            format: VertexFormat::Float32x2,
        })
    };
    let layout = |streams: &[(usize, u32)], attributes: &[(u32, u8)]| {
        let mut desc = VertexLayoutDesc {
            streams: [None; MAX_VERTEX_STREAMS],
            attributes: [None; MAX_VERTEX_STREAMS],
        };
        for &(at, stride) in streams {
            desc.streams[at] = stream(stride);
        }
        for (slot, &(location, s)) in desc.attributes.iter_mut().zip(attributes) {
            *slot = attribute(location, s);
        }
        desc
    };
    assert!(
        r.create_vertex_layout(&layout(&[(1, 16)], &[(0, 1), (1, 1)]))
            .is_ok()
    );
    let more = Err(GpuError::Invalid(
        "a vertex layout of more streams or attributes than the limits",
    ));
    assert_eq!(r.create_vertex_layout(&layout(&[(2, 8)], &[(0, 2)])), more);
    assert_eq!(
        r.create_vertex_layout(&layout(&[(0, 8)], &[(0, 0), (1, 0), (2, 0)])),
        more
    );
    assert_eq!(
        r.create_vertex_layout(&layout(&[(0, 20)], &[(0, 0)])),
        Err(GpuError::Invalid("a vertex stride past the limit"))
    );
}

#[test]
fn samplers_stay_within_the_anisotropy_limit() {
    let mut r = Recorder::new();
    let sampler = |anisotropy| SamplerDesc {
        min: Filter::Linear,
        mag: Filter::Linear,
        mip: Filter::Linear,
        address: [AddressMode::Clamp; 3],
        lod_range: [0.0f32.to_bits(), 32.0f32.to_bits()],
        anisotropy,
        compare: None,
        border: BorderColor::TransparentBlack,
    };
    let most = r.caps().limits.max_anisotropy;
    assert!(r.create_sampler(&sampler(most)).is_ok());
    assert_eq!(
        r.create_sampler(&sampler(most + 1)),
        Err(GpuError::Invalid("a sampler's anisotropy past the limit"))
    );
}

#[test]
fn a_programs_vertex_outputs_stay_within_the_inter_stage_limit() {
    let mut d = Drawing::with_caps(|c| c.limits.inter_stage_slots = 1);
    d.program_of("vs_out", "plain");
    let mut d = Drawing::with_caps(|c| c.limits.inter_stage_slots = 0);
    let module = naga::front::wgsl::parse_str(PROGRAM).unwrap();
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::MULTISAMPLED_SHADING,
    )
    .validate(&module)
    .unwrap();
    let v =
        d.r.create_shader(&ShaderModule {
            module,
            info,
            entry_point: "vs_out".into(),
            stage: ShaderStage::Vertex,
            key: *blake3::hash(b"vs_out").as_bytes(),
        })
        .unwrap();
    assert_eq!(
        d.r.create_program(&Program::Render {
            vertex: &v,
            fragment: None,
        }),
        Err(GpuError::Invalid(
            "vertex outputs past the inter-stage limit"
        ))
    );
}

#[test]
fn passes_stay_within_the_colour_target_limits() {
    let mut d = Drawing::with_caps(|c| c.limits.color_targets = 1);
    let other = texture(&mut d.r, Format::Rgba8Unorm, 2, ALL);
    let other = view(&mut d.r, &other, Format::Rgba8Unorm);
    let v = d.target_view;
    d.r.begin_pass(&pass_of([color(&v), color(&other)]));
    d.refused("a colour attachment past the limit");

    // Bytes a sample counted as `color_target_bytes` counts them: `rgba8unorm` as 8, and
    // `r32float` from a multiple of 4 after `r8unorm`'s 1.
    for (pair, limit) in [
        ([Format::Rgba8Unorm, Format::Rgba8Unorm], 16),
        ([Format::R8Unorm, Format::R32Float], 8),
    ] {
        assert_eq!(color_target_bytes(pair), Some(limit));
        for (limit, fits) in [(limit - 1, false), (limit, true)] {
            let mut d = Drawing::with_caps(|c| c.limits.color_bytes_per_sample = limit);
            let [a, b] = pair.map(|f| {
                let t = texture(&mut d.r, f, 2, ALL);
                view(&mut d.r, &t, f)
            });
            d.r.begin_pass(&pass_of([color(&a), color(&b)]));
            if fits {
                assert!(d.r.violations.is_empty(), "{:?}", d.r.violations);
            } else {
                d.refused("colour attachments of more bytes a sample than the limit");
            }
        }
    }
}

#[test]
fn a_draws_bindings_stay_within_the_limits_per_stage() {
    let mut d = Drawing::with_caps(|c| c.limits.uniform_buffers_per_stage = 1);
    let b = buffer(&mut d.r, 512, BufferUsage::UNIFORM);
    let mut set = |stages, binding| {
        d.r.create_binding_set(&[BindingEntry {
            binding,
            stages,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &b,
                offset: 256,
                size: 16,
            }),
        }])
        .unwrap()
    };
    let (vertex, fragment, also) = (
        set(StageSet::VERTEX, 0),
        set(StageSet::FRAGMENT, 0),
        set(StageSet::FRAGMENT, 1),
    );
    let p = d.program("colored");
    d.begin();
    // One for each stage fits.
    let list = Topology::TriangleList;
    let sets = |other| [Some(&fragment), Some(other), None, None];
    d.draw_sets(&p, sets(&vertex), list, false, |_, _| {});
    assert!(d.r.violations.is_empty());
    d.draw_sets(&p, sets(&also), list, false, |_, _| {});
    d.refused("bindings of one stage past a limit per stage");
}

#[test]
fn texture_writes_take_pitches_covering_the_region() {
    let short = "write or copy pitch smaller than the region";
    let mut r = Recorder::new();
    let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    let ds = texture(&mut r, Format::Depth24PlusStencil8, 2, ALL);
    let b = buffer(&mut r, 64, COPIES);
    // A row of two texels is 8 bytes; two rows per layer.
    r.write_texture(&region(&t), extent(2), &[0; 16], 8, 2, WriteMode::Ordered);
    assert!(r.violations.is_empty());
    r.write_texture(&region(&t), extent(2), &[0; 16], 4, 2, WriteMode::Ordered);
    r.write_texture(&region(&t), extent(2), &[0; 16], 8, 1, WriteMode::Ordered);
    let from = |row_pitch| CopyBufferTexture {
        buffer: &b,
        offset: 0,
        row_pitch,
        rows_per_layer: 2,
    };
    r.copy_buffer_to_texture(&from(4), &region(&t), extent(2));
    // The stencil aspect moves one byte a texel: a row of two is 2 bytes.
    let stencil = TextureRegion {
        stencil: true,
        ..region(&ds)
    };
    r.write_texture(&stencil, extent(2), &[0; 4], 1, 2, WriteMode::Ordered);
    r.write_texture(&stencil, extent(2), &[0; 4], 2, 1, WriteMode::Ordered);
    assert_eq!(r.violations, [short; 5]);
    r.violations.clear();
    r.write_texture(&stencil, extent(2), &[0; 4], 2, 2, WriteMode::Ordered);
    assert!(r.violations.is_empty());
    // The data covers the region too: three bytes for two rows of two.
    r.write_texture(&stencil, extent(2), &[0; 3], 2, 2, WriteMode::Ordered);
    assert_eq!(r.violations, ["data shorter than the region"]);
}

#[test]
fn colour_attachments_have_colour_formats() {
    let mut r = Recorder::new();
    let t = texture(&mut r, Format::Depth32Float, 2, ALL);
    let v = view(&mut r, &t, Format::Depth32Float);
    r.begin_pass(&pass_of([color(&v)]));
    assert_eq!(r.violations, ["a depth format as a colour attachment"]);
}

#[test]
fn textures_are_of_held_formats_and_not_empty() {
    let mut r =
        Recorder::with_caps(|c| c.formats.uses[Format::R8Unorm.index()] = FormatUse::empty());
    let desc = TextureDesc {
        dimension: TextureDimension::D2,
        format: Format::Rgba8Unorm,
        width: 2,
        height: 2,
        depth_or_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage: ALL,
        cube: false,
        reinterpret: false,
    };
    assert_eq!(
        r.create_texture(&TextureDesc {
            format: Format::R8Unorm,
            ..desc
        }),
        Err(GpuError::Invalid(
            "texture of a format this back end does not hold"
        ))
    );
    for empty in [
        TextureDesc { width: 0, ..desc },
        TextureDesc { height: 0, ..desc },
        TextureDesc {
            depth_or_layers: 0,
            ..desc
        },
        TextureDesc {
            mip_levels: 0,
            ..desc
        },
    ] {
        assert_eq!(
            r.create_texture(&empty),
            Err(GpuError::Invalid("empty texture")),
            "{empty:?}"
        );
    }
    assert_eq!(
        r.create_texture(&TextureDesc {
            cube: true,
            depth_or_layers: 4,
            ..desc
        }),
        Err(GpuError::Invalid(
            "cube texture that is not 2D and square with whole cubes of layers"
        ))
    );
    assert!(
        r.create_texture(&TextureDesc {
            cube: true,
            depth_or_layers: 12,
            ..desc
        })
        .is_ok()
    );
}

#[test]
fn cube_textures_are_square_and_levels_end_at_the_full_chain() {
    let mut r = Recorder::new();
    let cube = TextureDesc {
        cube: true,
        depth_or_layers: 6,
        ..texture_desc(Format::Rgba8Unorm, 4)
    };
    assert!(r.create_texture(&cube).is_ok());
    for wrong in [
        TextureDesc { height: 2, ..cube },
        TextureDesc {
            dimension: TextureDimension::D3,
            ..cube
        },
    ] {
        assert_eq!(
            r.create_texture(&wrong),
            Err(GpuError::Invalid(
                "cube texture that is not 2D and square with whole cubes of layers"
            )),
            "{wrong:?}"
        );
    }
    // 4x4 has three levels; a 2x8 2D array four, its layers not counting; a 4x4x16 3D texture five.
    let square = texture_desc(Format::Rgba8Unorm, 4);
    let tall = TextureDesc {
        width: 2,
        height: 8,
        depth_or_layers: 16,
        ..square
    };
    let deep = TextureDesc {
        dimension: TextureDimension::D3,
        depth_or_layers: 16,
        ..square
    };
    for (desc, chain) in [(square, 3), (tall, 4), (deep, 5)] {
        assert!(
            r.create_texture(&TextureDesc {
                mip_levels: chain,
                ..desc
            })
            .is_ok(),
            "{desc:?}"
        );
        for past in [chain + 1, 40] {
            assert_eq!(
                r.create_texture(&TextureDesc {
                    mip_levels: past,
                    ..desc
                }),
                Err(GpuError::Invalid("more levels than the full chain")),
                "{desc:?} {past}"
            );
        }
    }
}

#[test]
fn view_dimensions_fit_their_texture_and_layers() {
    let mut r = Recorder::new();
    let array = shaped(&mut r, TextureDimension::D2, 4, 12, 1);
    let cube = r
        .create_texture(&TextureDesc {
            cube: true,
            depth_or_layers: 12,
            ..texture_desc(Format::Rgba8Unorm, 4)
        })
        .unwrap();
    let volume = shaped(&mut r, TextureDimension::D3, 4, 4, 1);
    let desc = |dimension, layer_count| TextureViewDesc {
        format: Format::Rgba8Unorm,
        dimension,
        base_mip: 0,
        mip_count: 1,
        base_layer: 0,
        layer_count,
        stencil: false,
    };
    use ViewDimension::*;
    let fitting = [
        (&array, desc(D2, 1)),
        (&array, desc(D2Array, 3)),
        (&cube, desc(D2, 1)),
        (&cube, desc(D2Array, 12)),
        (&cube, desc(Cube, 6)),
        (&cube, desc(CubeArray, 12)),
        (&volume, desc(D3, 1)),
    ];
    for (t, view) in fitting {
        assert!(r.create_view(t, &view).is_ok(), "{view:?}");
    }
    let unfitting = [
        (&array, desc(D2, 3)),
        (&array, desc(Cube, 6)),
        (&array, desc(CubeArray, 12)),
        (&array, desc(D3, 1)),
        (&cube, desc(Cube, 4)),
        (&cube, desc(CubeArray, 8)),
        (&volume, desc(D2, 1)),
        (&volume, desc(D2Array, 1)),
        (&volume, desc(Cube, 1)),
    ];
    for (t, view) in unfitting {
        assert_eq!(
            r.create_view(t, &view),
            Err(GpuError::Invalid(
                "view dimension that does not fit its texture and layers"
            )),
            "{view:?}"
        );
    }
}

#[test]
fn views_cover_at_least_one_level_and_layer_inside_the_texture() {
    let mut r = Recorder::new();
    let t = shaped(&mut r, TextureDimension::D2, 4, 2, 2);
    let desc = TextureViewDesc {
        format: Format::Rgba8Unorm,
        dimension: ViewDimension::D2Array,
        base_mip: 0,
        mip_count: 2,
        base_layer: 0,
        layer_count: 2,
        stencil: false,
    };
    assert!(r.create_view(&t, &desc).is_ok());
    for outside in [
        TextureViewDesc {
            base_layer: 1,
            ..desc
        },
        TextureViewDesc {
            mip_count: 0,
            ..desc
        },
        TextureViewDesc {
            layer_count: 0,
            ..desc
        },
    ] {
        assert_eq!(
            r.create_view(&t, &outside),
            Err(GpuError::Invalid("view range outside the texture")),
            "{outside:?}"
        );
    }
}

#[test]
fn depth_bounds_and_border_and_mirror_once_samplers_are_absent() {
    let mut r = Recorder::new();
    assert_eq!(
        r.create_depth_stencil(&DepthStencilDesc {
            depth_compare: None,
            depth_write: false,
            stencil: None,
            stencil_read_mask: [0xff; 2],
            stencil_write_mask: [0xff; 2],
            depth_bounds: Some([0.0f32.to_bits(), 1.0f32.to_bits()]),
        }),
        Err(GpuError::Absent(Features::DEPTH_BOUNDS))
    );
    // Each mode names the feature it needs, and only that one.
    for (address, absent) in [
        (
            [AddressMode::Clamp, AddressMode::Clamp, AddressMode::Border],
            Features::BORDER_COLOR,
        ),
        (
            [
                AddressMode::MirrorOnce,
                AddressMode::Clamp,
                AddressMode::Clamp,
            ],
            Features::MIRROR_ONCE,
        ),
        (
            [
                AddressMode::Border,
                AddressMode::MirrorOnce,
                AddressMode::Clamp,
            ],
            Features::BORDER_COLOR | Features::MIRROR_ONCE,
        ),
    ] {
        assert_eq!(
            r.create_sampler(&SamplerDesc {
                min: Filter::Nearest,
                mag: Filter::Nearest,
                mip: Filter::Nearest,
                address,
                lod_range: [0.0f32.to_bits(), 32.0f32.to_bits()],
                anisotropy: 1,
                compare: None,
                border: BorderColor::TransparentBlack,
            }),
            Err(GpuError::Absent(absent)),
            "{address:?}"
        );
    }
}

#[test]
fn storage_bindings_need_storage_usage() {
    let mut r = Recorder::new();
    let b = buffer(&mut r, 256, BufferUsage::UNIFORM);
    assert_eq!(
        r.create_binding_set(&[BindingEntry {
            binding: 160,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::StorageBuffer {
                range: BufferRange {
                    buffer: &b,
                    offset: 0,
                    size: 16,
                },
                writable: false,
            },
        }]),
        Err(GpuError::Invalid(
            "a storage binding of a buffer without STORAGE"
        ))
    );
}

#[test]
fn texture_writes_carry_the_whole_region() {
    let mut r = Recorder::new();
    let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
    r.write_texture(&region(&t), extent(2), &[0; 15], 8, 2, WriteMode::Ordered);
    assert_eq!(r.violations, ["data shorter than the region"]);
}

#[test]
fn passes_and_queries_end_only_where_they_began() {
    let mut r = Recorder::new();
    r.end_pass();
    assert_eq!(r.violations, ["end_pass without a pass"]);
    let mut r = Recorder::new();
    r.end_query();
    assert_eq!(r.violations, ["end_query outside a pass"]);
}

#[test]
fn query_sets_of_absent_kinds_are_refused() {
    let mut r = Recorder::new();
    assert_eq!(
        r.create_query_set(QueryKind::Occlusion, 1),
        Err(GpuError::Absent(Features::OCCLUSION_COUNT))
    );
    let mut r = Recorder::with_caps(|c| c.features.remove(Features::TIMESTAMPS));
    assert_eq!(
        r.create_query_set(QueryKind::Timestamp, 1),
        Err(GpuError::Absent(Features::TIMESTAMPS))
    );
}

#[test]
fn queries_are_read_from_inside_their_set_outside_a_pass() {
    let refused = "read_queries of a range outside a live set, or inside a pass";
    let mut d = Drawing::new();
    let set = d.r.create_query_set(QueryKind::OcclusionAny, 2).unwrap();
    assert!(d.r.read_queries(&set, 0, 2).is_ok());
    assert!(d.r.read_queries(&set, 1, 2).is_err());
    assert_eq!(d.r.violations, [refused]);
    let mut d = Drawing::new();
    let set = d.r.create_query_set(QueryKind::OcclusionAny, 2).unwrap();
    d.begin();
    assert!(d.r.read_queries(&set, 0, 1).is_err());
    assert_eq!(d.r.violations, [refused]);
}

#[test]
fn buffer_copies_and_clears_stay_inside_their_buffers() {
    type Copies = fn(&mut Recorder, &Id, &Id, &Id);
    let cases: [(&str, Copies); 6] = [
        ("copy from past the buffer's end", |r, a, b, _| {
            r.copy_buffer(a, 12, b, 0, 8)
        }),
        ("copy to past the buffer's end", |r, a, b, _| {
            r.copy_buffer(a, 0, b, 12, 8)
        }),
        ("copy from past the buffer's end", |r, a, _, t| {
            let from = CopyBufferTexture {
                buffer: a,
                offset: 20,
                row_pitch: 8,
                rows_per_layer: 2,
            };
            r.copy_buffer_to_texture(&from, &region(t), extent(2))
        }),
        ("data shorter than the region", |r, a, _, t| {
            let from = CopyBufferTexture {
                buffer: a,
                offset: 4,
                row_pitch: 8,
                rows_per_layer: 2,
            };
            r.copy_buffer_to_texture(&from, &region(t), extent(2))
        }),
        ("copy to past the buffer's end", |r, a, _, t| {
            let to = CopyBufferTexture {
                buffer: a,
                offset: 4,
                row_pitch: 8,
                rows_per_layer: 2,
            };
            r.copy_texture_to_buffer(&region(t), &to, extent(2))
        }),
        ("clear past the buffer's end", |r, a, _, _| {
            r.clear_buffer(a, 8, 12, &[0])
        }),
    ];
    for (violation, f) in cases {
        let mut r = Recorder::new();
        let a = buffer(&mut r, 16, COPIES);
        let b = buffer(&mut r, 16, COPIES);
        let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
        f(&mut r, &a, &b, &t);
        assert_eq!(r.violations, [violation]);
    }
}

#[test]
fn a_surface_gives_a_texture_once_configured() {
    let mut r = Recorder::new();
    let s = r.create_surface(1).unwrap();
    assert_eq!(
        r.current_texture(&s),
        Err(GpuError::Invalid(
            "current_texture of an unconfigured surface"
        ))
    );
}

#[test]
fn bindings_hold_the_kind_of_resource_the_program_reads() {
    // Each binding holds a texture (true) or a uniform buffer (false): a buffer where a texture is
    // read, a texture where a buffer is, a texture where a sampler is.
    type Case = (&'static str, &'static [(u32, bool)], &'static str);
    let cases: [Case; 3] = [
        ("loads", &[(32, false)], "no texture at (0, 32)"),
        ("colored", &[(0, true)], "no buffer at group 0 binding 0"),
        (
            "sampled",
            &[(32, true), (16, true)],
            "no sampler at (0, 16)",
        ),
    ];
    for (fragment, bindings, violation) in cases {
        let mut d = Drawing::new();
        let b = buffer(&mut d.r, 256, BufferUsage::UNIFORM);
        let t = texture(&mut d.r, Format::Rgba8Unorm, 2, ALL);
        let v = view(&mut d.r, &t, Format::Rgba8Unorm);
        let entries: Vec<_> = bindings
            .iter()
            .map(|&(binding, texture)| BindingEntry {
                binding,
                stages: StageSet::FRAGMENT,
                resource: if texture {
                    BindingResource::Texture(&v)
                } else {
                    BindingResource::UniformBuffer(BufferRange {
                        buffer: &b,
                        offset: 0,
                        size: 16,
                    })
                },
            })
            .collect();
        let set = d.r.create_binding_set(&entries).unwrap();
        let p = d.program(fragment);
        d.begin();
        d.draw(&p, Some(&set), Topology::TriangleList, false);
        d.refused(violation);
    }
}

#[test]
fn draw_ranges_end_within_a_u32() {
    let range = "a draw's range past the largest vertex or index";
    let instances = "a draw's instances past the largest instance index";
    let vertices = |first, first_instance| DrawRange::Vertices {
        first,
        count: 3,
        first_instance,
        instances: 2,
    };
    let indexed = |first, first_instance| DrawRange::Indexed {
        first,
        count: 3,
        base_vertex: 0,
        first_instance,
        instances: 2,
    };
    let cases = [
        (vertices(u32::MAX - 1, 0), range),
        (vertices(0, u32::MAX), instances),
        (indexed(u32::MAX - 1, 0), range),
        (indexed(0, u32::MAX), instances),
    ];
    for (r, violation) in cases {
        let mut d = Drawing::new();
        let p = d.program("plain");
        d.begin();
        d.draw_with(&p, None, Topology::TriangleList, false, |c, spare| {
            if matches!(r, DrawRange::Indexed { .. }) {
                c.index = Some(IndexBinding {
                    buffer: spare,
                    offset: 0,
                    format: IndexFormat::U16,
                });
            }
            c.range = r;
        });
        d.refused(violation);
    }
}

#[test]
fn draws_read_vertices_and_indices_inside_their_buffers() {
    // The spare buffer's 8 bytes hold one vertex of two floats: vertex 2 starts past them, vertex 1
    // at their end has none of its bytes.
    let cases: [(&str, u64); 2] = [
        ("a vertex past the buffer's end", 2),
        ("a vertex read past the buffer's end", 0),
    ];
    for (violation, first) in cases {
        let mut d = Drawing::new();
        d.vertices = d.spare;
        let p = d.program("plain");
        d.begin();
        d.draw_with(&p, None, Topology::TriangleList, false, |c, _| {
            c.range = DrawRange::Vertices {
                first: first as u32,
                count: 3,
                first_instance: 0,
                instances: 1,
            }
        });
        d.refused(violation);
    }
    let indexed = |count| DrawRange::Indexed {
        first: 0,
        count,
        base_vertex: 0,
        first_instance: 0,
        instances: 1,
    };
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c, _| {
        c.range = indexed(3)
    });
    d.refused("an indexed draw without indices");
    // Five 16-bit indices are 10 bytes.
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c, spare| {
        c.index = Some(IndexBinding {
            buffer: spare,
            offset: 0,
            format: IndexFormat::U16,
        });
        c.range = indexed(6);
    });
    d.refused("an index past the buffer's end");
}

#[test]
fn an_indexed_draw_fetching_past_a_stream_is_dropped_whole() {
    // The 24 bytes of vertices hold vertices 0 to 2; vertex 3 starts at their end.
    for (last, dropped) in [(2u16, 0), (3, 1)] {
        let mut d = Drawing::new();
        let indices: Vec<u8> = [0u16, 1, last, 0]
            .iter()
            .flat_map(|i| i.to_le_bytes())
            .collect();
        d.r.write_buffer(&d.spare, 0, &indices, WriteMode::Ordered);
        let before = d.r.texture_bytes(d.target);
        let p = d.program("plain");
        d.begin();
        d.draw_with(&p, None, Topology::TriangleList, false, |c, spare| {
            c.index = Some(IndexBinding {
                buffer: spare,
                offset: 0,
                format: IndexFormat::U16,
            });
            c.range = DrawRange::Indexed {
                first: 0,
                count: 3,
                base_vertex: 0,
                first_instance: 0,
                instances: 1,
            };
        });
        d.r.end_pass();
        assert!(d.r.violations.is_empty(), "{:?}", d.r.violations);
        assert_eq!(d.r.dropped_draws, dropped, "index {last}");
        assert_eq!(
            d.r.texture_bytes(d.target) == before,
            dropped == 1,
            "index {last}"
        );
    }
}

#[test]
fn an_indexed_draw_fetching_past_a_stream_still_reads_no_subresource_it_writes() {
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
    let indices: Vec<u8> = [0u16, 1, 3, 0]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    d.r.write_buffer(&d.spare, 0, &indices, WriteMode::Ordered);
    d.begin();
    d.draw_with(&p, Some(&set), Topology::TriangleList, false, |c, spare| {
        c.index = Some(IndexBinding {
            buffer: spare,
            offset: 0,
            format: IndexFormat::U16,
        });
        c.range = DrawRange::Indexed {
            first: 0,
            count: 3,
            base_vertex: 0,
            first_instance: 0,
            instances: 1,
        };
    });
    assert_eq!(d.r.dropped_draws, 0);
    d.refused("a subresource bound for reading while it is a written attachment");
}

#[test]
fn a_draw_names_its_viewport_scissor_streams_and_buffers() {
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c, _| {
        c.viewports = &[]
    });
    d.refused("a draw without a viewport");

    let mut d = Drawing::new();
    d.raster =
        d.r.create_raster(&RasterDesc {
            fill: FillMode::Solid,
            cull: CullMode::None,
            front: FrontFace::CounterClockwise,
            depth_bias: 0,
            depth_bias_slope: 0,
            depth_bias_clamp: 0,
            depth_clip: true,
            scissor: true,
            line_width: 1.0f32.to_bits(),
            provoking: ProvokingVertex::First,
            depth_minus_one: false,
            sample_mask: !0,
        })
        .unwrap();
    let p = d.program("plain");
    d.begin();
    d.draw(&p, None, Topology::TriangleList, false);
    d.refused("scissor on without a scissor");

    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c, _| {
        c.vertex_buffers = &[]
    });
    d.refused("an attribute of a stream with no buffer bound");

    // A layout whose attribute names a stream it does not describe.
    let mut d = Drawing::new();
    let mut desc = VertexLayoutDesc {
        streams: [None; MAX_VERTEX_STREAMS],
        attributes: [None; MAX_VERTEX_STREAMS],
    };
    desc.attributes[0] = Some(VertexAttribute {
        location: 0,
        stream: 1,
        offset: 0,
        format: VertexFormat::Float32x2,
    });
    d.layout = d.r.create_vertex_layout(&desc).unwrap();
    let p = d.program("plain");
    d.begin();
    d.draw(&p, None, Topology::TriangleList, false);
    d.refused("an attribute of an unused stream");
}

#[test]
fn draws_need_the_topologies_and_indirect_draws_the_back_end_has() {
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw(&p, None, Topology::TriangleFan, false);
    d.refused("fans and loops go through draw_fan_or_loop on a back end without FAN_AND_LOOP");

    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c, spare| {
        c.range = DrawRange::Indirect {
            args: spare,
            offset: 0,
        }
    });
    d.refused("indirect draws, which this back end reports absent");
}

/// A sampler of the filters given, comparing with `compare` if given.
fn sampler_of(r: &mut Recorder, filters: [Filter; 3], compare: Option<CompareFn>) -> Id {
    r.create_sampler(&SamplerDesc {
        min: filters[0],
        mag: filters[1],
        mip: filters[2],
        address: [AddressMode::Clamp; 3],
        lod_range: [0.0f32.to_bits(), 32.0f32.to_bits()],
        anisotropy: 1,
        compare,
        border: BorderColor::TransparentBlack,
    })
    .unwrap()
}

#[test]
fn samplers_compare_exactly_where_the_program_compares() {
    use Filter::{Linear as L, Nearest as N};
    // A depth32float texture cleared to 0.5, compared with 0.4 through binding 17; binding 32's
    // rgba8unorm texture sampled plainly through binding 16. A comparison filters linearly a format
    // without FILTER (depth here), as WebGPU's comparison samplers do.
    type Case<'a> = (&'a str, [Filter; 3], Option<CompareFn>, Option<&'a str>, u8);
    let cases: [Case; 6] = [
        ("compared", [N; 3], Some(CompareFn::Less), None, 255),
        ("compared", [L; 3], Some(CompareFn::Less), None, 255),
        ("compared", [N; 3], Some(CompareFn::Greater), None, 0),
        (
            "compared",
            [N; 3],
            None,
            Some("a program comparing through a sampler that does not compare"),
            0,
        ),
        ("sampled", [N; 3], None, None, 0),
        (
            "sampled",
            [N; 3],
            Some(CompareFn::Less),
            Some("a comparison sampler where the program samples without comparing"),
            0,
        ),
    ];
    for (fragment, filters, compare, violation, want) in cases {
        let mut d = Drawing::new();
        let depth = texture(
            &mut d.r,
            Format::Depth32Float,
            2,
            TextureUsage::RENDER | TextureUsage::SAMPLE,
        );
        let depth_view = view(&mut d.r, &depth, Format::Depth32Float);
        d.r.begin_pass(&PassDesc {
            depth: Some(DepthAttachment {
                view: &depth_view,
                depth: Some(AspectOps {
                    load: LoadOp::Clear(0.5f32.to_bits()),
                    store: StoreOp::Store,
                    read_only: false,
                }),
                stencil: None,
            }),
            ..pass_of([])
        });
        d.r.end_pass();
        let t = texture(&mut d.r, Format::Rgba8Unorm, 2, TextureUsage::SAMPLE);
        let v = view(&mut d.r, &t, Format::Rgba8Unorm);
        let s = sampler_of(&mut d.r, filters, compare);
        let (texture_at, sampler_at) = if fragment == "compared" {
            (34, 17)
        } else {
            (32, 16)
        };
        let set =
            d.r.create_binding_set(&[
                BindingEntry {
                    binding: texture_at,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Texture(if fragment == "compared" {
                        &depth_view
                    } else {
                        &v
                    }),
                },
                BindingEntry {
                    binding: sampler_at,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Sampler(&s),
                },
            ])
            .unwrap();
        let p = d.program(fragment);
        d.begin();
        d.draw(&p, Some(&set), Topology::TriangleList, false);
        match violation {
            Some(v) => d.refused(v),
            None => {
                assert!(d.r.violations.is_empty(), "{:?}", d.r.violations);
                let got = d.r.texture_bytes(d.target).unwrap();
                assert!(got.iter().all(|&b| b == want), "{compare:?}: {got:?}");
            }
        }
    }
}

#[test]
fn linear_filters_pair_only_with_textures_the_back_end_filters() {
    use Filter::{Linear as L, Nearest as N};
    // Any linear filter, whichever level the draw samples, against r32float without FILTER.
    for (filters, refused) in [
        ([L, N, N], true),
        ([N, L, N], true),
        ([N, N, L], true),
        ([N, N, N], false),
    ] {
        let mut d = Drawing::with_caps(|c| {
            c.formats.uses[Format::R32Float.index()].remove(FormatUse::FILTER)
        });
        let t = texture(&mut d.r, Format::R32Float, 2, TextureUsage::SAMPLE);
        let v = view(&mut d.r, &t, Format::R32Float);
        let s = sampler_of(&mut d.r, filters, None);
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
        if refused {
            d.refused("R32Float filtered, which this back end does not filter");
        } else {
            assert!(d.r.violations.is_empty(), "{:?}", d.r.violations);
        }
    }
}

#[test]
fn buffer_bindings_cover_the_programs_blocks() {
    // A uniform block of 32 bytes read only at its start, and a storage block of a u32 and a
    // runtime-sized array of vec4, 32 bytes with one element.
    let cases: [(&str, u32, bool, u64, Option<&str>); 4] = [
        (
            "first_of_two",
            1,
            false,
            16,
            Some("a buffer binding of 16 bytes for a block of 32"),
        ),
        ("first_of_two", 1, false, 32, None),
        (
            "first_element",
            160,
            true,
            16,
            Some("a buffer binding of 16 bytes for a block of 32"),
        ),
        ("first_element", 160, true, 32, None),
    ];
    for (fragment, binding, storage, size, violation) in cases {
        let mut d = Drawing::new();
        let b = buffer(&mut d.r, 256, BufferUsage::UNIFORM | BufferUsage::STORAGE);
        let range = BufferRange {
            buffer: &b,
            offset: 0,
            size,
        };
        let set =
            d.r.create_binding_set(&[BindingEntry {
                binding,
                stages: StageSet::FRAGMENT,
                resource: if storage {
                    BindingResource::StorageBuffer {
                        range,
                        writable: false,
                    }
                } else {
                    BindingResource::UniformBuffer(range)
                },
            }])
            .unwrap();
        let p = d.program(fragment);
        d.begin();
        d.draw(&p, Some(&set), Topology::TriangleList, false);
        match violation {
            Some(v) => d.refused(v),
            None => assert!(d.r.violations.is_empty(), "{:?}", d.r.violations),
        }
    }
}

#[test]
fn a_draw_has_one_viewport_and_at_most_one_scissor() {
    const fn viewport(x: f32) -> Viewport {
        Viewport {
            x: x.to_bits(),
            y: 0f32.to_bits(),
            width: 1f32.to_bits(),
            height: 1f32.to_bits(),
            min_depth: 0f32.to_bits(),
            max_depth: 1f32.to_bits(),
        }
    }
    const TWO: [Viewport; 2] = [viewport(0.0), viewport(1.0)];
    const SCISSORS: [ScissorRect; 2] = [
        ScissorRect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        ScissorRect {
            x: 1,
            y: 1,
            width: 1,
            height: 1,
        },
    ];
    let violation =
        "several viewports or scissors in one draw, which fronts split (MultiViewportSplit)";
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c, _| {
        c.viewports = &TWO
    });
    d.refused(violation);

    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c, _| {
        c.scissors = &SCISSORS
    });
    d.refused(violation);

    // One scissor with the raster block's scissor off is a draw.
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c, _| {
        c.scissors = &SCISSORS[..1]
    });
    assert!(d.r.violations.is_empty(), "{:?}", d.r.violations);
}

#[test]
fn an_empty_pass_has_a_size_and_a_sample_count_r8unorm_takes() {
    let size = "an empty pass of no size or wider or taller than max_texture_2d";
    let samples = "an empty pass of a sample count r8unorm does not take";
    let most = Recorder::new().caps().limits.max_texture_2d;
    let cases = [
        (0, 2, 1, Some(size)),
        (2, 0, 1, Some(size)),
        (most + 1, 2, 1, Some(size)),
        (2, most + 1, 1, Some(size)),
        (2, 2, 0, Some(samples)),
        (2, 2, 3, Some(samples)),
        (2, 2, 8, Some(samples)),
        (2, 2, 1, None),
        (2, 2, 4, None),
        (most, most, 1, None),
    ];
    for (width, height, count, violation) in cases {
        let mut r = Recorder::new();
        r.begin_pass(&PassDesc {
            empty: Some(EmptyPass {
                width,
                height,
                samples: count,
            }),
            ..pass_of([])
        });
        assert_eq!(
            r.violations,
            violation.as_slice(),
            "{width}x{height}, {count} samples"
        );
    }
}

#[test]
fn ranges_ending_past_the_largest_offset_are_refused() {
    // Offsets near 2^64, where offset plus size does not fit: refused as past the buffer, not
    // wrapped. A binding set's refusal is its error; the others are violations.
    type Call = fn(&mut Recorder, &Id, &Id, &Id) -> Option<GpuError>;
    const HUGE: u64 = u64::MAX - 3;
    let cases: [(&str, Call); 8] = [
        ("write past the buffer's end", |r, a, _, _| {
            r.write_buffer(a, HUGE, &[0; 8], WriteMode::Ordered);
            None
        }),
        ("clear past the buffer's end", |r, a, _, _| {
            r.clear_buffer(a, HUGE, 8, &[0]);
            None
        }),
        ("copy from past the buffer's end", |r, a, b, _| {
            r.copy_buffer(a, HUGE, b, 0, 8);
            None
        }),
        ("copy to past the buffer's end", |r, a, b, _| {
            r.copy_buffer(a, 0, b, HUGE, 8);
            None
        }),
        ("read back past the buffer's end", |r, a, _, _| {
            r.read_back(&ReadSource::Buffer {
                buffer: a,
                offset: HUGE,
                size: 8,
            })
            .err()
        }),
        ("copy from past the buffer's end", |r, a, _, t| {
            let from = CopyBufferTexture {
                buffer: a,
                offset: HUGE,
                row_pitch: 8,
                rows_per_layer: 2,
            };
            r.copy_buffer_to_texture(&from, &region(t), extent(2));
            None
        }),
        ("copy to past the buffer's end", |r, a, _, t| {
            let to = CopyBufferTexture {
                buffer: a,
                offset: HUGE,
                row_pitch: 8,
                rows_per_layer: 2,
            };
            r.copy_texture_to_buffer(&region(t), &to, extent(2));
            None
        }),
        ("", |r, a, _, _| {
            r.create_binding_set(&[BindingEntry {
                binding: 0,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::StorageBuffer {
                    range: BufferRange {
                        buffer: a,
                        offset: u64::MAX - 255,
                        size: 512,
                    },
                    writable: false,
                },
            }])
            .err()
        }),
    ];
    for (violation, call) in cases {
        let mut r = Recorder::new();
        let uses = COPIES | BufferUsage::STORAGE;
        let a = buffer(&mut r, 16, uses);
        let b = buffer(&mut r, 16, uses);
        let t = texture(&mut r, Format::Rgba8Unorm, 2, ALL);
        let error = call(&mut r, &a, &b, &t);
        if violation.is_empty() {
            assert_eq!(
                error,
                Some(GpuError::Invalid("a buffer binding past the buffer's end"))
            );
            assert!(r.violations.is_empty());
        } else {
            assert_eq!(r.violations, [violation]);
        }
    }

    // A draw's index and vertex bindings at such offsets.
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    d.draw_with(&p, None, Topology::TriangleList, false, |c, spare| {
        c.index = Some(IndexBinding {
            buffer: spare,
            offset: HUGE,
            format: IndexFormat::U32,
        });
        c.range = DrawRange::Indexed {
            first: 0,
            count: 3,
            base_vertex: 0,
            first_instance: 0,
            instances: 1,
        };
    });
    d.refused("an index past the buffer's end");
    let mut d = Drawing::new();
    let p = d.program("plain");
    d.begin();
    // The call's change outlives this function's borrows, so the stream is leaked.
    let vertices: &'static Id = Box::leak(Box::new(d.vertices));
    let streams: &'static [Option<VertexBinding<'static, Recorder>>] =
        Box::leak(Box::new([Some(VertexBinding {
            buffer: vertices,
            offset: HUGE,
        })]));
    d.draw_with(&p, None, Topology::TriangleList, false, |c, _| {
        c.vertex_buffers = streams
    });
    d.refused("a vertex past the buffer's end");
}

#[test]
fn textures_paired_with_samplers_lie_in_the_sampled_texture_range() {
    // Textures at the edges of 32..160 and outside it, sampled through a sampler (directly or in a
    // function the entry point calls) or only loaded.
    let source = "
@group(0) @binding(16) var s: sampler;
@group(3) @binding(5) var low: texture_2d<f32>;
@group(0) @binding(31) var below: texture_2d<f32>;
@group(0) @binding(32) var first: texture_2d<f32>;
@group(1) @binding(159) var last: texture_2d<f32>;
@group(0) @binding(160) var past: texture_2d<f32>;
fn sample_low() -> vec4<f32> {
    return textureSampleLevel(low, s, vec2<f32>(0.5), 0.0);
}
@fragment fn pairs_5() -> @location(0) vec4<f32> { return sample_low(); }
@fragment fn pairs_31() -> @location(0) vec4<f32> {
    return textureSampleLevel(below, s, vec2<f32>(0.5), 0.0);
}
@fragment fn pairs_32() -> @location(0) vec4<f32> {
    return textureSampleLevel(first, s, vec2<f32>(0.5), 0.0);
}
@fragment fn pairs_159() -> @location(0) vec4<f32> {
    return textureSampleLevel(last, s, vec2<f32>(0.5), 0.0);
}
@fragment fn pairs_160() -> @location(0) vec4<f32> {
    return textureSampleLevel(past, s, vec2<f32>(0.5), 0.0);
}
@fragment fn loads_5() -> @location(0) vec4<f32> {
    return textureLoad(low, vec2<i32>(0, 0), 0) + textureLoad(past, vec2<i32>(0, 0), 0);
}
@vertex fn vs_pairs_5() -> @builtin(position) vec4<f32> {
    return textureSampleLevel(low, s, vec2<f32>(0.5), 0.0);
}
@vertex fn vs_pairs_32() -> @builtin(position) vec4<f32> {
    return textureSampleLevel(first, s, vec2<f32>(0.5), 0.0);
}
";
    let module = naga::front::wgsl::parse_str(source).unwrap();
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
    let mut r = Recorder::new();
    let outside = Err(GpuError::Invalid(
        "a texture paired with a sampler outside the sampled-texture bindings",
    ));
    for (entry, stage, made) in [
        ("pairs_5", ShaderStage::Fragment, false),
        ("pairs_31", ShaderStage::Fragment, false),
        ("pairs_32", ShaderStage::Fragment, true),
        ("pairs_159", ShaderStage::Fragment, true),
        ("pairs_160", ShaderStage::Fragment, false),
        ("loads_5", ShaderStage::Fragment, true),
        ("vs_pairs_5", ShaderStage::Vertex, false),
        ("vs_pairs_32", ShaderStage::Vertex, true),
    ] {
        let got = r
            .create_shader(&ShaderModule {
                module: module.clone(),
                info: info.clone(),
                entry_point: entry.into(),
                stage,
                key: *blake3::hash(entry.as_bytes()).as_bytes(),
            })
            .map(|_| ());
        if made {
            assert_eq!(got, Ok(()), "{entry}");
        } else {
            assert_eq!(got, outside, "{entry}");
        }
    }
}

#[test]
fn binding_entries_name_every_stage_that_reads_them() {
    // A texture and sampler the fragment program samples, and a uniform the vertex program reads,
    // each made for the stages given; naming a stage that does not read is allowed.
    let f = StageSet::FRAGMENT;
    let v = StageSet::VERTEX;
    let both = v | f;
    let fragment_left_out = "a binding the fragment stage reads whose entry leaves that stage out";
    let vertex_left_out = "a binding the vertex stage reads whose entry leaves that stage out";
    type Case<'a> = (&'a str, &'a str, [StageSet; 3], Option<&'a str>);
    let cases: [Case; 7] = [
        ("vs", "sampled", [f, f, f], None),
        ("vs", "sampled", [both, both, both], None),
        ("vs", "sampled", [v, f, f], Some(fragment_left_out)),
        ("vs", "sampled", [f, v, f], Some(fragment_left_out)),
        ("vs_reads", "plain", [f, f, v], None),
        ("vs_reads", "plain", [f, f, both], None),
        ("vs_reads", "plain", [f, f, f], Some(vertex_left_out)),
    ];
    for (vs, fs, [texture_stages, sampler_stages, uniform_stages], violation) in cases {
        let mut d = Drawing::new();
        let t = texture(&mut d.r, Format::Rgba8Unorm, 2, TextureUsage::SAMPLE);
        let tv = view(&mut d.r, &t, Format::Rgba8Unorm);
        let s = sampler_of(&mut d.r, [Filter::Nearest; 3], None);
        let u = buffer(&mut d.r, 16, BufferUsage::UNIFORM);
        let set =
            d.r.create_binding_set(&[
                BindingEntry {
                    binding: 32,
                    stages: texture_stages,
                    resource: BindingResource::Texture(&tv),
                },
                BindingEntry {
                    binding: 16,
                    stages: sampler_stages,
                    resource: BindingResource::Sampler(&s),
                },
                BindingEntry {
                    binding: 0,
                    stages: uniform_stages,
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &u,
                        offset: 0,
                        size: 16,
                    }),
                },
            ])
            .unwrap();
        let p = d.program_of(vs, fs);
        d.begin();
        d.draw(&p, Some(&set), Topology::TriangleList, false);
        match violation {
            Some(violation) => d.refused(violation),
            None => assert!(d.r.violations.is_empty(), "{vs} {fs}: {:?}", d.r.violations),
        }
    }
}
