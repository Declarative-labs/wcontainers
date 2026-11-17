//! The present-or-absent operations on the recording back end: dispatches, vertex capture and
//! timestamps run and give exact results, and each of their rules (plan 4.3, 4.7) broken once is
//! refused, as a violation that loses the device and `GpuError::Invalid`, or as `GpuError::Absent`.

use wcontainers_gpu::*;
use wcontainers_gpu_test::{Id, Recorder};

const PROGRAMS: &str = "
@group(0) @binding(0) var<storage, read_write> out: array<u32>;
@compute @workgroup_size(2, 2, 1)
fn ids(@builtin(global_invocation_id) g: vec3<u32>, @builtin(local_invocation_index) l: u32,
       @builtin(workgroup_id) w: vec3<u32>, @builtin(num_workgroups) n: vec3<u32>) {
    out[g.y * 4u + g.x] = 1000u * w.x + 100u * l + 10u * n.x + n.y;
}
@compute @workgroup_size(512, 1, 1) fn wide() { out[0] = 1u; }
@compute @workgroup_size(16, 16, 2) fn many() { out[0] = 1u; }
@compute @workgroup_size(1, 1, 65) fn deep() { out[0] = 1u; }
var<workgroup> shared_words: array<u32, 4>;
@compute @workgroup_size(1, 1, 1) fn shares() {
    shared_words[0] = 1u;
    out[0] = shared_words[0];
}
struct VOut {
    @builtin(position) p: vec4<f32>,
    @location(0) a: vec2<f32>,
    @location(1) @interpolate(flat) n: u32,
}
@vertex fn vs(@location(0) v: vec2<f32>, @builtin(vertex_index) i: u32,
              @builtin(instance_index) k: u32) -> VOut {
    return VOut(vec4<f32>(v, 0.5, 1.0), v, i + 10u * k);
}
@group(0) @binding(2) var<uniform> shift: vec4<f32>;
@vertex fn vs_reads(@location(0) v: vec2<f32>, @builtin(vertex_index) i: u32) -> VOut {
    return VOut(vec4<f32>(v, 0.5, 1.0), v + shift.xy, i);
}
@group(0) @binding(32) var texels: texture_2d<f32>;
@vertex fn vs_loads(@location(0) v: vec2<f32>, @builtin(vertex_index) i: u32) -> VOut {
    return VOut(vec4<f32>(v, 0.5, 1.0), v + textureLoad(texels, vec2<i32>(0, 0), 0).xy, i);
}
@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }
";

/// The corners of a strip over the whole target.
const CORNERS: [[f32; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]];

fn buffer(r: &mut Recorder, size: u64, usage: BufferUsage) -> Id {
    r.create_buffer(&BufferDesc {
        size,
        usage: usage | BufferUsage::COPY_SRC | BufferUsage::COPY_DST | BufferUsage::READ_BACK,
        intent: MapIntent::Rarely,
    })
    .unwrap()
}

fn read(r: &mut Recorder, buffer: &Id, size: u64) -> Vec<u8> {
    let mut out = vec![0; size as usize];
    r.read_back_now(
        &ReadSource::Buffer {
            buffer,
            offset: 0,
            size,
        },
        &mut out,
    )
    .unwrap();
    out
}

fn shader(r: &mut Recorder, entry: &str, stage: ShaderStage) -> Id {
    let module = naga::front::wgsl::parse_str(PROGRAMS).unwrap();
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
    r.create_shader(&ShaderModule {
        module,
        info,
        entry_point: entry.into(),
        stage,
        key: *blake3::hash(entry.as_bytes()).as_bytes(),
    })
    .unwrap()
}

fn compute(r: &mut Recorder, entry: &str) -> Result<Id, GpuError> {
    let s = shader(r, entry, ShaderStage::Compute);
    r.create_program(&Program::Compute(&s))
}

fn storage(r: &mut Recorder, buffer: &Id, stages: StageSet) -> Id {
    r.create_binding_set(&[BindingEntry {
        binding: 0,
        stages,
        resource: BindingResource::StorageBuffer {
            range: BufferRange {
                buffer,
                offset: 0,
                size: 32,
            },
            writable: true,
        },
    }])
    .unwrap()
}

fn dispatch(r: &mut Recorder, program: &Id, set: &Id, groups: [u32; 3]) -> Result<(), GpuError> {
    r.dispatch(&DispatchCall {
        program,
        bindings: [Some(set), None, None, None],
        immediates: &[],
        groups: DispatchGroups::Direct(groups),
    })
}

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|w| u32::from_le_bytes(*w))
        .collect()
}

#[test]
fn dispatches_run_every_invocation_with_its_ids() {
    let mut r = Recorder::new();
    let out = buffer(&mut r, 32, BufferUsage::STORAGE);
    let p = compute(&mut r, "ids").unwrap();
    let set = storage(&mut r, &out, StageSet::COMPUTE);
    dispatch(&mut r, &p, &set, [2, 1, 1]).unwrap();
    let mut expected = Vec::new();
    for y in 0..2 {
        for x in 0..4 {
            expected.push(1000 * (x / 2) + 100 * (y * 2 + x % 2) + 20 + 1);
        }
    }
    assert_eq!(words(&read(&mut r, &out, 32)), expected);
    assert!(r.violations.is_empty(), "{:?}", r.violations);
    assert_eq!(r.calls.iter().filter(|c| c.name() == "dispatch").count(), 1);
    // A dispatch is a command: a no-overwrite write over what it bound waits for it.
    dispatch(&mut r, &p, &set, [1; 3]).unwrap();
    r.write_buffer(&out, 0, &[0; 4], WriteMode::NoOverwrite);
    assert_eq!(r.violations.len(), 1);
    assert!(r.violations[0].starts_with("a NoOverwrite write over bytes of buffer"));
}

#[test]
fn dispatches_keep_to_their_rules() {
    let refused = |what: &str, run: &dyn Fn(&mut Recorder, &Id, &Id, &Id)| {
        let mut r = Recorder::new();
        let out = buffer(&mut r, 32, BufferUsage::STORAGE | BufferUsage::VERTEX);
        let p = compute(&mut r, "ids").unwrap();
        let set = storage(&mut r, &out, StageSet::COMPUTE);
        run(&mut r, &p, &set, &out);
        assert_eq!(r.violations, [what]);
    };
    refused("dispatch inside a pass", &|r, p, set, _| {
        r.begin_pass(&PassDesc {
            colors: Default::default(),
            depth: None,
            empty: Some(EmptyPass {
                width: 1,
                height: 1,
                samples: 1,
            }),
            occlusion: None,
        });
        assert_eq!(
            dispatch(r, p, set, [1; 3]),
            Err(GpuError::Invalid("dispatch refused"))
        );
        r.end_pass();
    });
    refused(
        "a dispatch of a program that is not a compute program",
        &|r, _, set, _| {
            let v = shader(r, "vs", ShaderStage::Vertex);
            let f = shader(r, "fs", ShaderStage::Fragment);
            let render = r
                .create_program(&Program::Render {
                    vertex: &v,
                    fragment: Some(&f),
                })
                .unwrap();
            assert!(dispatch(r, &render, set, [1; 3]).is_err());
        },
    );
    refused(
        "a dispatch of more groups than max_dispatch",
        &|r, p, set, _| {
            assert!(dispatch(r, p, set, [65536, 1, 1]).is_err());
        },
    );
    refused(
        "a binding the compute stage reads whose entry leaves that stage out",
        &|r, p, _, out| {
            let set = storage(r, out, StageSet::FRAGMENT);
            assert!(dispatch(r, p, &set, [1; 3]).is_err());
        },
    );
    refused(
        "a dispatch binding a buffer as writable storage and another way",
        &|r, p, _, out| {
            let range = BufferRange {
                buffer: out,
                offset: 0,
                size: 32,
            };
            let set = r
                .create_binding_set(&[
                    BindingEntry {
                        binding: 0,
                        stages: StageSet::COMPUTE,
                        resource: BindingResource::StorageBuffer {
                            range,
                            writable: true,
                        },
                    },
                    BindingEntry {
                        binding: 1,
                        stages: StageSet::COMPUTE,
                        resource: BindingResource::StorageBuffer {
                            range,
                            writable: false,
                        },
                    },
                ])
                .unwrap();
            assert!(dispatch(r, p, &set, [1; 3]).is_err());
        },
    );
    refused("immediates past the limit", &|r, p, set, _| {
        assert!(
            r.dispatch(&DispatchCall {
                program: p,
                bindings: [Some(set), None, None, None],
                immediates: &[0; 4],
                groups: DispatchGroups::Direct([1; 3]),
            })
            .is_err()
        );
    });
    // Arguments from a buffer need INDIRECT, which the recording back end does not declare.
    let mut r = Recorder::new();
    let out = buffer(&mut r, 32, BufferUsage::STORAGE | BufferUsage::INDIRECT);
    let p = compute(&mut r, "ids").unwrap();
    let set = storage(&mut r, &out, StageSet::COMPUTE);
    assert_eq!(
        r.dispatch(&DispatchCall {
            program: &p,
            bindings: [Some(&set), None, None, None],
            immediates: &[],
            groups: DispatchGroups::Indirect {
                args: &out,
                offset: 0,
            },
        }),
        Err(GpuError::Absent(Features::INDIRECT))
    );
    assert!(r.violations.is_empty());
}

#[test]
fn compute_programs_keep_to_the_compute_limits() {
    let mut r = Recorder::new();
    // Past the invocations in all, and past one axis alone.
    for entry in ["wide", "many", "deep"] {
        assert_eq!(
            compute(&mut r, entry),
            Err(GpuError::Invalid("a workgroup past the compute limits")),
            "{entry}"
        );
    }
    assert_eq!(
        compute(&mut r, "shares"),
        Err(GpuError::Invalid("workgroup variables past the limit"))
    );
    let v = shader(&mut r, "vs", ShaderStage::Vertex);
    assert_eq!(
        r.create_program(&Program::Compute(&v)),
        Err(GpuError::Invalid("a program of shaders of other stages"))
    );
}

/// A strip of [`CORNERS`] and the blocks to draw or capture it, with a colour target.
struct Strip {
    r: Recorder,
    vertices: Id,
    program: Id,
    blend: Id,
    depth: Id,
    raster: Id,
    layout: Id,
    target: Id,
    view: Id,
}

impl Strip {
    fn new() -> Strip {
        Strip::on(Recorder::new())
    }

    fn on(mut r: Recorder) -> Strip {
        // Capture targets too, for the rule that a call uses a target no other way.
        let usage = BufferUsage::VERTEX | BufferUsage::COPY_DST | BufferUsage::CAPTURE;
        let vertices = buffer(&mut r, 32, usage);
        let bytes: Vec<u8> = CORNERS
            .iter()
            .flatten()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        r.write_buffer(&vertices, 0, &bytes, WriteMode::Ordered);
        let v = shader(&mut r, "vs", ShaderStage::Vertex);
        let f = shader(&mut r, "fs", ShaderStage::Fragment);
        let program = r
            .create_program(&Program::Render {
                vertex: &v,
                fragment: Some(&f),
            })
            .unwrap();
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
        let target = r
            .create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format: Format::Rgba8Unorm,
                width: 2,
                height: 2,
                depth_or_layers: 1,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::RENDER
                    | TextureUsage::SAMPLE
                    | TextureUsage::COPY_SRC
                    | TextureUsage::COPY_DST,
                cube: false,
                reinterpret: false,
            })
            .unwrap();
        let view = r
            .create_view(
                &target,
                &TextureViewDesc {
                    format: Format::Rgba8Unorm,
                    dimension: ViewDimension::D2,
                    base_mip: 0,
                    mip_count: 1,
                    base_layer: 0,
                    layer_count: 1,
                    stencil: false,
                },
            )
            .unwrap();
        Strip {
            r,
            vertices,
            program,
            blend,
            depth,
            raster,
            layout,
            target,
            view,
        }
    }

    fn begin(&mut self) {
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view: &self.view,
            depth_slice: 0,
            resolve: None,
            load: LoadOp::Clear([0; 4]),
            store: StoreOp::Store,
        });
        self.r.begin_pass(&pass);
    }

    /// Captures the strip, drawn twice as instances, into `targets`, with `topology`.
    fn capture(
        &mut self,
        targets: &[BufferRange<&Id>],
        topology: Topology,
        rasterize: bool,
    ) -> Result<(), GpuError> {
        self.capture_from(targets, topology, rasterize, Inputs::default())
    }

    /// Captures as [`Strip::capture`] with `inputs` in place of the strip's own.
    fn capture_from(
        &mut self,
        targets: &[BufferRange<&Id>],
        topology: Topology,
        rasterize: bool,
        inputs: Inputs,
    ) -> Result<(), GpuError> {
        let stream = inputs.stream.unwrap_or(self.vertices);
        let program = inputs.program.unwrap_or(self.program);
        let streams = [Some(VertexBinding {
            buffer: &stream,
            offset: 0,
        })];
        let immediates = vec![0; inputs.immediates];
        let viewport = [Viewport {
            x: 0,
            y: 0,
            width: 2.0f32.to_bits(),
            height: 2.0f32.to_bits(),
            min_depth: 0,
            max_depth: 1.0f32.to_bits(),
        }];
        let call = DrawCall {
            program: &program,
            blend: &self.blend,
            depth_stencil: &self.depth,
            raster: &self.raster,
            vertex_layout: &self.layout,
            bindings: [inputs.set.as_ref(), None, None, None],
            vertex_buffers: &streams,
            index: None,
            topology,
            restart: false,
            viewports: if rasterize { &viewport } else { &[] },
            scissors: &[],
            blend_constant: [0; 4],
            stencil_reference: [0; 2],
            immediates: &immediates,
            range: DrawRange::Vertices {
                first: 0,
                count: 4,
                first_instance: 0,
                instances: 2,
            },
        };
        self.r.capture_vertices(&call, targets, rasterize)
    }
}

/// What a capture takes in place of the strip's own: a program, a binding set, the stream's buffer,
/// and bytes of immediates.
#[derive(Default)]
struct Inputs {
    program: Option<Id>,
    set: Option<Id>,
    stream: Option<Id>,
    immediates: usize,
}

/// The program reading a uniform at binding 2, and a set binding a buffer there for `stages`.
fn reading(s: &mut Strip, stages: StageSet) -> (Id, Id, Id) {
    let v = shader(&mut s.r, "vs_reads", ShaderStage::Vertex);
    let program =
        s.r.create_program(&Program::Render {
            vertex: &v,
            fragment: None,
        })
        .unwrap();
    let uniform = buffer(&mut s.r, 16, BufferUsage::UNIFORM);
    let set =
        s.r.create_binding_set(&[BindingEntry {
            binding: 2,
            stages,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &uniform,
                offset: 0,
                size: 16,
            }),
        }])
        .unwrap();
    (program, set, uniform)
}

fn whole(buffer: &Id, size: u64) -> BufferRange<&Id> {
    BufferRange {
        buffer,
        offset: 0,
        size,
    }
}

/// The captured outputs of the strip's two instances: each vertex's position and its index plus
/// ten times its instance, vertices in `order`.
fn captured(order: &[u32]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let (mut both, mut a, mut n) = (Vec::new(), Vec::new(), Vec::new());
    for k in 0..2 {
        for &i in order {
            let p: Vec<u8> = CORNERS[i as usize]
                .iter()
                .flat_map(|f| f.to_le_bytes())
                .collect();
            let w = (i + 10 * k).to_le_bytes();
            both.extend(&p);
            both.extend(w);
            a.extend(&p);
            n.extend(w);
        }
    }
    (both, a, n)
}

#[test]
fn captures_write_each_vertex_of_each_primitive_in_order() {
    // The strip's odd triangle in the order it is drawn with the first vertex provoking.
    let order = [0, 1, 2, 1, 3, 2];
    let (both, a, n) = captured(&order);
    let mut s = Strip::new();
    let one = buffer(&mut s.r, 144, BufferUsage::CAPTURE);
    s.capture(&[whole(&one, 144)], Topology::TriangleStrip, false)
        .unwrap();
    assert_eq!(read(&mut s.r, &one, 144), both);
    let (ta, tn) = (
        buffer(&mut s.r, 96, BufferUsage::CAPTURE),
        buffer(&mut s.r, 48, BufferUsage::CAPTURE),
    );
    s.capture(
        &[whole(&ta, 96), whole(&tn, 48)],
        Topology::TriangleStrip,
        false,
    )
    .unwrap();
    assert_eq!(read(&mut s.r, &ta, 96), a);
    assert_eq!(read(&mut s.r, &tn, 48), n);
    // Points and line strips, which the recording back end captures but does not draw.
    let points = buffer(&mut s.r, 96, BufferUsage::CAPTURE);
    s.capture(&[whole(&points, 96)], Topology::PointList, false)
        .unwrap();
    assert_eq!(read(&mut s.r, &points, 96), captured(&[0, 1, 2, 3]).0);
    let lines = buffer(&mut s.r, 144, BufferUsage::CAPTURE);
    s.capture(&[whole(&lines, 144)], Topology::LineStrip, false)
        .unwrap();
    assert_eq!(read(&mut s.r, &lines, 144), captured(&[0, 1, 1, 2, 2, 3]).0);
    // Rasterised, inside a pass: the strip is also drawn.
    let drawn = buffer(&mut s.r, 144, BufferUsage::CAPTURE);
    s.begin();
    s.capture(&[whole(&drawn, 144)], Topology::TriangleStrip, true)
        .unwrap();
    s.r.end_pass();
    assert_eq!(read(&mut s.r, &drawn, 144), both);
    let mut pixels = vec![0; 16];
    let target = s.target;
    s.r.read_back_now(
        &ReadSource::Texture {
            region: TextureRegion {
                texture: &target,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width: 2,
                height: 2,
                depth_or_layers: 1,
            },
        },
        &mut pixels,
    )
    .unwrap();
    assert_eq!(pixels, [255; 16]);
    assert!(s.r.violations.is_empty(), "{:?}", s.r.violations);
    // A capture is a command: a no-overwrite write over its target, or over the vertices it
    // read, waits for it.
    let vertices = s.vertices;
    for written in [one, vertices] {
        let f = s.r.submit();
        s.r.wait_fence(&f);
        s.capture(&[whole(&one, 144)], Topology::TriangleStrip, false)
            .unwrap();
        s.r.write_buffer(&written, 0, &[0; 4], WriteMode::NoOverwrite);
        assert_eq!(s.r.violations.len(), 1);
        assert!(s.r.violations[0].starts_with("a NoOverwrite write over bytes of buffer"));
        s.r.violations.clear();
    }
    // The same for a buffer the capture's binding set binds.
    let (program, set, uniform) = reading(&mut s, StageSet::VERTEX);
    let f = s.r.submit();
    s.r.wait_fence(&f);
    s.capture_from(
        &[whole(&one, 144)],
        Topology::TriangleStrip,
        false,
        Inputs {
            program: Some(program),
            set: Some(set),
            ..Inputs::default()
        },
    )
    .unwrap();
    s.r.write_buffer(&uniform, 0, &[0; 4], WriteMode::NoOverwrite);
    assert_eq!(s.r.violations.len(), 1);
    assert!(s.r.violations[0].starts_with("a NoOverwrite write over bytes of buffer"));
    s.r.violations.clear();
    // And for a texture it binds.
    let v = shader(&mut s.r, "vs_loads", ShaderStage::Vertex);
    let program =
        s.r.create_program(&Program::Render {
            vertex: &v,
            fragment: None,
        })
        .unwrap();
    let texture = s.target;
    let texels = s.view;
    let set =
        s.r.create_binding_set(&[BindingEntry {
            binding: 32,
            stages: StageSet::VERTEX,
            resource: BindingResource::Texture(&texels),
        }])
        .unwrap();
    let f = s.r.submit();
    s.r.wait_fence(&f);
    s.capture_from(
        &[whole(&one, 144)],
        Topology::TriangleStrip,
        false,
        Inputs {
            program: Some(program),
            set: Some(set),
            ..Inputs::default()
        },
    )
    .unwrap();
    s.r.write_texture(
        &TextureRegion {
            texture: &texture,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        },
        Extent {
            width: 1,
            height: 1,
            depth_or_layers: 1,
        },
        &[0; 4],
        4,
        1,
        WriteMode::NoOverwrite,
    );
    assert_eq!(s.r.violations.len(), 1, "{:?}", s.r.violations);
    assert!(
        s.r.violations[0].contains("NoOverwrite"),
        "{:?}",
        s.r.violations
    );
}

#[test]
fn captures_keep_to_their_rules() {
    let refused = |what: &str, run: &dyn Fn(&mut Strip) -> Result<(), GpuError>| {
        let mut s = Strip::new();
        assert_eq!(
            run(&mut s),
            Err(GpuError::Invalid("capture_vertices refused")),
            "{what}"
        );
        assert!(
            s.r.violations.len() == 1 && s.r.violations[0].starts_with(what),
            "{what}: {:?}",
            s.r.violations
        );
    };
    let target = |s: &mut Strip, size: u64| buffer(&mut s.r, size, BufferUsage::CAPTURE);
    refused("a capture without rasterising inside a pass", &|s| {
        let t = target(s, 144);
        s.begin();
        s.capture(&[whole(&t, 144)], Topology::TriangleStrip, false)
    });
    refused("a rasterised capture outside a pass", &|s| {
        let t = target(s, 144);
        s.capture(&[whole(&t, 144)], Topology::TriangleStrip, true)
    });
    refused(
        "capture targets of none or more than capture_buffers",
        &|s| s.capture(&[], Topology::TriangleStrip, false),
    );
    refused(
        "capture targets of none or more than capture_buffers",
        &|s| {
            let t = target(s, 1024);
            let five: Vec<_> = (0..5)
                .map(|i| BufferRange {
                    buffer: &t,
                    offset: i * 200,
                    size: 200,
                })
                .collect();
            s.capture(&five, Topology::TriangleStrip, false)
        },
    );
    refused(
        "several capture targets, not one for each output the program writes",
        &|s| {
            let t = target(s, 600);
            let three: Vec<_> = (0..3)
                .map(|i| BufferRange {
                    buffer: &t,
                    offset: i * 200,
                    size: 200,
                })
                .collect();
            s.capture(&three, Topology::TriangleStrip, false)
        },
    );
    refused("a capture target of buffer", &|s| {
        let t = buffer(&mut s.r, 144, BufferUsage::STORAGE);
        s.capture(&[whole(&t, 144)], Topology::TriangleStrip, false)
    });
    refused("a capture target past its buffer's end", &|s| {
        let t = target(s, 144);
        s.capture(
            &[BufferRange {
                buffer: &t,
                offset: 8,
                size: 144,
            }],
            Topology::TriangleStrip,
            false,
        )
    });
    refused(
        "capture targets that do not hold every vertex the draw makes",
        &|s| {
            let t = target(s, 140);
            s.capture(&[whole(&t, 140)], Topology::TriangleStrip, false)
        },
    );
    refused("a capture target the call also uses another way", &|s| {
        let both = s.vertices;
        s.capture(&[whole(&both, 32)], Topology::TriangleStrip, false)
    });
    refused("immediates past the limit", &|s| {
        let t = target(s, 144);
        s.capture_from(
            &[whole(&t, 144)],
            Topology::TriangleStrip,
            false,
            Inputs {
                immediates: 4,
                ..Inputs::default()
            },
        )
    });
    refused(
        "a binding the vertex stage reads whose entry leaves that stage out",
        &|s| {
            let t = target(s, 144);
            let (program, set, _) = reading(s, StageSet::FRAGMENT);
            s.capture_from(
                &[whole(&t, 144)],
                Topology::TriangleStrip,
                false,
                Inputs {
                    program: Some(program),
                    set: Some(set),
                    ..Inputs::default()
                },
            )
        },
    );
    refused("a vertex stream of buffer", &|s| {
        let t = target(s, 144);
        let stream = buffer(&mut s.r, 32, BufferUsage::STORAGE);
        s.capture_from(
            &[whole(&t, 144)],
            Topology::TriangleStrip,
            false,
            Inputs {
                stream: Some(stream),
                ..Inputs::default()
            },
        )
    });
    refused(
        "fans and loops go through draw_fan_or_loop on a back end without FAN_AND_LOOP",
        &|s| {
            let t = target(s, 144);
            s.capture(&[whole(&t, 144)], Topology::TriangleFan, false)
        },
    );
}

#[test]
fn timestamps_count_the_calls_run_and_keep_to_their_rules() {
    let mut r = Recorder::new();
    let set = r.create_query_set(QueryKind::Timestamp, 2).unwrap();
    r.write_timestamp(&set, 0).unwrap();
    let scratch = buffer(&mut r, 4, BufferUsage::COPY_DST);
    r.write_buffer(&scratch, 0, &[1; 4], WriteMode::Ordered);
    r.write_timestamp(&set, 1).unwrap();
    assert_eq!(r.timestamps_disjoint(), Ok(false));
    let mut bytes = vec![0; 16];
    let rb = r.read_queries(&set, 0, 2).unwrap();
    r.submit();
    r.wait_read(&rb);
    r.finish_read(rb, &mut bytes);
    let first = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let second = u64::from_le_bytes(bytes[8..].try_into().unwrap());
    // The first write, and the buffer made and written after it, each advance the clock by one.
    assert_eq!(second - first, 3);
    assert!(r.violations.is_empty(), "{:?}", r.violations);

    let refused = |what: &str, run: &dyn Fn(&mut Recorder, &Id) -> Result<(), GpuError>| {
        let mut r = Recorder::new();
        let set = r.create_query_set(QueryKind::Timestamp, 2).unwrap();
        assert_eq!(
            run(&mut r, &set),
            Err(GpuError::Invalid("write_timestamp refused")),
            "{what}"
        );
        assert_eq!(r.violations, [what]);
    };
    refused("write_timestamp inside a pass", &|r, set| {
        r.begin_pass(&PassDesc {
            colors: Default::default(),
            depth: None,
            empty: Some(EmptyPass {
                width: 1,
                height: 1,
                samples: 1,
            }),
            occlusion: None,
        });
        r.write_timestamp(set, 0)
    });
    refused("a timestamp index outside its set", &|r, set| {
        r.write_timestamp(set, 2)
    });
    refused(
        "a timestamp written into what is not a live timestamp set",
        &|r, _| {
            let occlusion = r.create_query_set(QueryKind::OcclusionAny, 2).unwrap();
            r.write_timestamp(&occlusion, 0)
        },
    );
    // A timestamp set is not a pass's occlusion set.
    let mut r = Recorder::new();
    let set = r.create_query_set(QueryKind::Timestamp, 2).unwrap();
    r.begin_pass(&PassDesc {
        colors: Default::default(),
        depth: None,
        empty: Some(EmptyPass {
            width: 1,
            height: 1,
            samples: 1,
        }),
        occlusion: Some(&set),
    });
    assert_eq!(
        r.violations,
        ["the pass's occlusion set is not an occlusion set"]
    );
}

#[test]
fn resource_tables_are_absent() {
    let mut r = Recorder::new();
    assert_eq!(
        r.create_resource_table(16).err(),
        Some(GpuError::Absent(Features::RESOURCE_TABLES))
    );
    assert!(!r.caps().has(Features::RESOURCE_TABLES));
}

#[test]
fn each_operation_is_absent_without_its_feature() {
    let mut r = Recorder::with_caps(|c| c.features.remove(Features::COMPUTE));
    let s = shader(&mut r, "ids", ShaderStage::Compute);
    assert_eq!(
        r.create_program(&Program::Compute(&s)),
        Err(GpuError::Absent(Features::COMPUTE))
    );
    let out = buffer(&mut r, 32, BufferUsage::STORAGE);
    let set = storage(&mut r, &out, StageSet::COMPUTE);
    assert_eq!(
        dispatch(&mut r, &s, &set, [1; 3]),
        Err(GpuError::Absent(Features::COMPUTE))
    );
    let mut s = Strip::on(Recorder::with_caps(|c| {
        c.features.remove(Features::TRANSFORM_FEEDBACK)
    }));
    let t = buffer(&mut s.r, 144, BufferUsage::CAPTURE);
    assert_eq!(
        s.capture(&[whole(&t, 144)], Topology::TriangleStrip, false),
        Err(GpuError::Absent(Features::TRANSFORM_FEEDBACK))
    );
    let mut t = Recorder::with_caps(|c| c.features.remove(Features::TIMESTAMPS));
    let occlusion = t.create_query_set(QueryKind::OcclusionAny, 1).unwrap();
    assert_eq!(
        t.write_timestamp(&occlusion, 0),
        Err(GpuError::Absent(Features::TIMESTAMPS))
    );
    assert_eq!(
        t.timestamps_disjoint(),
        Err(GpuError::Absent(Features::TIMESTAMPS))
    );
    assert!(r.violations.is_empty() && s.r.violations.is_empty() && t.violations.is_empty());
}
