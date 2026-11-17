//! The interface's shared helpers (the pass tracker, the occlusion counter and the combination
//! tracker) driving the recording back end through scripted draws.

use wcontainers_gpu::*;
use wcontainers_gpu_test::{Id, Recorder};

const PROGRAMS: &str = "
struct Params { color: vec4<f32>, depth: f32 }
@group(0) @binding(0) var<uniform> params: Params;
@group(1) @binding(160) var<storage, read_write> written: array<u32>;
@group(1) @binding(161) var<storage, read> source: array<u32>;
@vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p, params.depth, 1.0);
}
@fragment fn color() -> @location(0) vec4<f32> {
    return params.color;
}
struct Two { @location(0) a: vec4<f32>, @location(1) b: vec4<f32> }
@fragment fn two() -> Two {
    return Two(params.color, params.color);
}
@fragment fn write() -> @location(0) vec4<f32> {
    written[0] = 7u;
    return params.color;
}
@fragment fn reads() -> @location(0) vec4<f32> {
    return vec4<f32>(f32(source[0]) / 255.0, 0.0, 0.0, 1.0);
}
@fragment fn counting() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0);
}
";

/// What the tests draw with: one recorder and the blocks they share.
struct Kit {
    r: Recorder,
    quad: Id,
    layout: Id,
    blend: Id,
    raster: Id,
    scissored: Id,
    uniforms: Id,
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

fn raster_desc(scissor: bool) -> RasterDesc {
    RasterDesc {
        fill: FillMode::Solid,
        cull: CullMode::None,
        front: FrontFace::CounterClockwise,
        depth_bias: 0,
        depth_bias_slope: 0,
        depth_bias_clamp: 0,
        depth_clip: true,
        scissor,
        line_width: 1.0f32.to_bits(),
        provoking: ProvokingVertex::First,
        depth_minus_one: false,
        sample_mask: !0,
    }
}

impl Kit {
    fn new() -> Kit {
        let mut r = Recorder::new();
        let quad = r
            .create_buffer(&BufferDesc {
                size: 32,
                usage: BufferUsage::VERTEX | BufferUsage::COPY_DST,
                intent: MapIntent::Rarely,
            })
            .unwrap();
        let corners: Vec<u8> = [-1.0f32, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        r.write_buffer(&quad, 0, &corners, WriteMode::Discard);
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
        let raster = r.create_raster(&raster_desc(false)).unwrap();
        let scissored = r.create_raster(&raster_desc(true)).unwrap();
        let uniforms = r
            .create_buffer(&BufferDesc {
                size: 256 * 8,
                usage: BufferUsage::UNIFORM | BufferUsage::COPY_DST,
                intent: MapIntent::Rarely,
            })
            .unwrap();
        Kit {
            r,
            quad,
            layout,
            blend,
            raster,
            scissored,
            uniforms,
        }
    }

    fn program(&mut self, fragment: &str) -> Id {
        let v = shader(&mut self.r, "vs", ShaderStage::Vertex);
        let f = shader(&mut self.r, fragment, ShaderStage::Fragment);
        self.r
            .create_program(&Program::Render {
                vertex: &v,
                fragment: Some(&f),
            })
            .unwrap()
    }

    fn depth(&mut self, compare: Option<CompareFn>, write: bool) -> Id {
        self.r
            .create_depth_stencil(&DepthStencilDesc {
                depth_compare: compare,
                depth_write: write,
                stencil: None,
                stencil_read_mask: [0xff; 2],
                stencil_write_mask: [0xff; 2],
                depth_bounds: None,
            })
            .unwrap()
    }

    /// The parameters of draw `slot`: a colour and a depth, in a binding set of their own.
    fn params(&mut self, slot: u64, color: [f32; 4], depth: f32) -> Id {
        let bytes: Vec<u8> = color
            .iter()
            .chain(&[depth])
            .flat_map(|f| f.to_le_bytes())
            .collect();
        self.r
            .write_buffer(&self.uniforms, slot * 256, &bytes, WriteMode::Ordered);
        self.r
            .create_binding_set(&[BindingEntry {
                binding: 0,
                stages: StageSet::VERTEX | StageSet::FRAGMENT,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &self.uniforms,
                    offset: slot * 256,
                    size: 32,
                }),
            }])
            .unwrap()
    }

    fn target(&mut self, width: u32, height: u32, format: Format) -> Target<Recorder> {
        let texture = self
            .r
            .create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width,
                height,
                depth_or_layers: 1,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::RENDER | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
                cube: false,
                reinterpret: false,
            })
            .unwrap();
        let view = self
            .r
            .create_view(
                &texture,
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
            .unwrap();
        Target {
            view,
            texture,
            format,
            mip: 0,
            layer: 0,
            depth_slice: 0,
            width,
            height,
            samples: 1,
        }
    }

    /// Draws the full-screen quad at the pass's size, with the scissors the tracker gave.
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &mut self,
        size: (u32, u32),
        area: Option<ScissorRect>,
        program: &Id,
        depth_stencil: &Id,
        sets: [Option<&Id>; MAX_BINDING_SETS],
    ) {
        let mut scissors = Vec::new();
        if let Some(area) = area {
            cut_scissors(&[], area, &mut scissors);
        }
        let viewport = [Viewport {
            x: 0f32.to_bits(),
            y: 0f32.to_bits(),
            width: (size.0 as f32).to_bits(),
            height: (size.1 as f32).to_bits(),
            min_depth: 0f32.to_bits(),
            max_depth: 1f32.to_bits(),
        }];
        let streams = [Some(VertexBinding {
            buffer: &self.quad,
            offset: 0,
        })];
        let call = DrawCall {
            program,
            blend: &self.blend,
            depth_stencil,
            raster: if area.is_some() {
                &self.scissored
            } else {
                &self.raster
            },
            vertex_layout: &self.layout,
            bindings: sets,
            vertex_buffers: &streams,
            index: None,
            topology: Topology::TriangleStrip,
            restart: false,
            viewports: &viewport,
            scissors: &scissors,
            blend_constant: [0; 4],
            stencil_reference: [0; 2],
            immediates: &[],
            range: DrawRange::Vertices {
                first: 0,
                count: 4,
                first_instance: 0,
                instances: 1,
            },
        };
        self.r.draw(&call);
    }

    fn read(&mut self, t: &Target<Recorder>, texel: usize) -> Vec<u8> {
        let mut out = vec![0; (t.width * t.height) as usize * texel];
        self.r
            .read_back_now(
                &ReadSource::Texture {
                    region: TextureRegion {
                        texture: &t.texture,
                        mip: 0,
                        layer: 0,
                        origin: Origin { x: 0, y: 0, z: 0 },
                        stencil: false,
                    },
                    extent: Extent {
                        width: t.width,
                        height: t.height,
                        depth_or_layers: 1,
                    },
                },
                &mut out,
            )
            .unwrap();
        out
    }

    fn calls(&self, name: &str) -> usize {
        self.r.calls.iter().filter(|c| c.name() == name).count()
    }

    fn healthy(&mut self) {
        assert_eq!(self.r.violations, Vec::<String>::new());
        assert_eq!(self.r.status(), DeviceStatus::Ok);
    }
}

fn colors(t: Target<Recorder>) -> PassTargets<Recorder> {
    let mut colors: [Option<ColorTarget<Recorder>>; MAX_COLOR_TARGETS] = Default::default();
    colors[0] = Some(ColorTarget {
        target: t,
        resolve: None,
    });
    PassTargets {
        colors,
        depth: None,
        empty: None,
    }
}

fn f32s(v: [f32; 4]) -> [u32; 4] {
    v.map(f32::to_bits)
}

#[test]
fn an_implicit_fronts_clear_becomes_the_passs_load() {
    let mut k = Kit::new();
    let mut passes = PassTracker::default();
    let t = k.target(2, 2, Format::Rgba8Unorm);
    passes.set_targets(&mut k.r, colors(t.clone())).unwrap();
    passes
        .clear_color(&mut k.r, 0, f32s([1.0, 0.0, 0.0, 1.0]))
        .unwrap();
    let p = k.program("color");
    let none = k.depth(None, false);
    let set = k.params(0, [0.0, 1.0, 0.0, 1.0], 0.5);
    let area = passes.before_draw(&mut k.r, &[]).unwrap();
    assert_eq!(area, None);
    k.draw((2, 1), None, &p, &none, [Some(&set), None, None, None]);
    passes.close(&mut k.r).unwrap();
    // One pass: the clear was its load, and the draw covered the top row.
    assert_eq!(k.calls("begin_pass"), 1);
    let got = k.read(&t, 4);
    assert_eq!(
        got,
        [
            [0, 255, 0, 255],
            [0, 255, 0, 255],
            [255, 0, 0, 255],
            [255, 0, 0, 255]
        ]
        .concat()
    );

    // A clear no draw follows still happens, in a pass of its own, before the read.
    passes
        .clear_color(&mut k.r, 0, f32s([0.0, 0.0, 1.0, 1.0]))
        .unwrap();
    passes.close(&mut k.r).unwrap();
    assert_eq!(k.calls("begin_pass"), 2);
    assert_eq!(k.read(&t, 4), [0, 0, 255, 255].repeat(4));
    k.healthy();
}

#[test]
fn a_larger_depth_buffer_is_fitted() {
    let mut k = Kit::new();
    let mut passes = PassTracker::default();
    let color = k.target(4, 4, Format::Rgba8Unorm);
    let depth = k.target(8, 8, Format::Depth32Float);
    let mut targets = colors(color.clone());
    targets.depth = Some(depth.clone());
    passes.set_targets(&mut k.r, targets).unwrap();
    passes
        .clear_color(&mut k.r, 0, f32s([0.0, 0.0, 0.0, 1.0]))
        .unwrap();
    passes
        .clear_depth_stencil(&mut k.r, Some(1.0f32.to_bits()), None)
        .unwrap();
    let p = k.program("color");
    let less = k.depth(Some(CompareFn::Less), true);
    let set = k.params(0, [1.0, 1.0, 1.0, 1.0], 0.5);
    let textures = k.calls("create_texture");
    let area = passes.before_draw(&mut k.r, &[]).unwrap();
    assert_eq!(
        area,
        Some(ScissorRect {
            x: 0,
            y: 0,
            width: 4,
            height: 4
        })
    );
    // The colour target went to an 8 by 8 temporary; cleared, so nothing copied in.
    assert_eq!(k.calls("create_texture"), textures + 1);
    assert_eq!(k.calls("copy_texture"), 0);
    k.draw((8, 8), area, &p, &less, [Some(&set), None, None, None]);
    passes.close(&mut k.r).unwrap();
    // Stored, so the common area went back.
    assert_eq!(k.calls("copy_texture"), 1);
    assert_eq!(k.read(&color, 4), [255; 4].repeat(16));
    let d = k.read(&depth, 4);
    for y in 0..8 {
        for x in 0..8 {
            let at = (y * 8 + x) * 4;
            let z = f32::from_le_bytes(d[at..at + 4].try_into().unwrap());
            let want = if x < 4 && y < 4 { 0.5 } else { 1.0 };
            assert_eq!(z, want, "depth at ({x}, {y})");
        }
    }
    assert_eq!(k.r.slow_paths().count(SlowPath::AttachmentsFitted), 1);

    // The next pass loads the colour target: its common area is copied in, and the pooled
    // temporary is used again.
    let nearer = k.params(1, [1.0, 0.0, 0.0, 1.0], 0.25);
    let area = passes.before_draw(&mut k.r, &[]).unwrap();
    assert_eq!(k.calls("copy_texture"), 2);
    assert_eq!(k.calls("create_texture"), textures + 1);
    k.draw((8, 8), area, &p, &less, [Some(&nearer), None, None, None]);
    passes.close(&mut k.r).unwrap();
    assert_eq!(k.calls("copy_texture"), 3);
    assert_eq!(k.read(&color, 4), [255, 0, 0, 255].repeat(16));
    k.healthy();
}

#[test]
fn colour_targets_of_different_sizes_are_fitted() {
    let mut k = Kit::new();
    let mut passes = PassTracker::default();
    let small = k.target(2, 2, Format::Rgba8Unorm);
    let large = k.target(3, 3, Format::Rgba8Unorm);
    let before: Vec<u8> = (0..36).collect();
    k.r.write_texture(
        &TextureRegion {
            texture: &large.texture,
            mip: 0,
            layer: 0,
            origin: Origin { x: 0, y: 0, z: 0 },
            stencil: false,
        },
        Extent {
            width: 3,
            height: 3,
            depth_or_layers: 1,
        },
        &before,
        12,
        3,
        WriteMode::Ordered,
    );
    let mut targets = colors(small.clone());
    targets.colors[1] = Some(ColorTarget {
        target: large.clone(),
        resolve: None,
    });
    passes.set_targets(&mut k.r, targets).unwrap();
    let p = k.program("two");
    let none = k.depth(None, false);
    let set = k.params(0, [0.0, 0.0, 1.0, 1.0], 0.5);
    let area = passes.before_draw(&mut k.r, &[]).unwrap();
    // The pass takes colour target 0's size; the larger one is loaded into a temporary.
    assert_eq!(area.map(|a| (a.width, a.height)), Some((2, 2)));
    assert_eq!(k.calls("copy_texture"), 1);
    k.draw((2, 2), area, &p, &none, [Some(&set), None, None, None]);
    passes.close(&mut k.r).unwrap();
    assert_eq!(k.read(&small, 4), [0, 0, 255, 255].repeat(4));
    let mut want = before.clone();
    for y in 0..2 {
        for x in 0..2 {
            want[y * 12 + x * 4..][..4].copy_from_slice(&[0, 0, 255, 255]);
        }
    }
    assert_eq!(k.read(&large, 4), want);
    k.healthy();
}

/// A storage buffer written by one draw and read by the next: the tracker splits the pass there,
/// carrying a running query across, and the recording back end accepts it. Drawn in one pass, the
/// recording back end refuses it.
#[test]
fn a_pass_is_split_where_a_draw_reads_what_one_wrote_as_storage() {
    for tracked in [true, false] {
        let mut k = Kit::new();
        let t = k.target(2, 2, Format::Rgba8Unorm);
        let storage =
            k.r.create_buffer(&BufferDesc {
                size: 16,
                usage: BufferUsage::STORAGE | BufferUsage::COPY_DST,
                intent: MapIntent::Rarely,
            })
            .unwrap();
        let entry = |binding, writable| BindingEntry {
            binding,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::StorageBuffer {
                range: BufferRange {
                    buffer: &storage,
                    offset: 0,
                    size: 16,
                },
                writable,
            },
        };
        let writes = k.r.create_binding_set(&[entry(160, true)]).unwrap();
        let reads = k.r.create_binding_set(&[entry(161, false)]).unwrap();
        let write = k.program("write");
        let read = k.program("reads");
        let none = k.depth(None, false);
        let set = k.params(0, [0.0, 1.0, 0.0, 1.0], 0.5);
        let queries = k.r.create_query_set(QueryKind::OcclusionAny, 4).unwrap();
        let mut passes = PassTracker::default();
        if tracked {
            passes.set_targets(&mut k.r, colors(t.clone())).unwrap();
            passes.set_occlusion(&mut k.r, queries, 4).unwrap();
            passes.begin_query(&mut k.r).unwrap();
            let area = passes
                .before_draw(
                    &mut k.r,
                    &[ScopeUse {
                        resource: 1,
                        storage_write: true,
                    }],
                )
                .unwrap();
            k.draw(
                (2, 2),
                area,
                &write,
                &none,
                [Some(&set), Some(&writes), None, None],
            );
            let area = passes
                .before_draw(
                    &mut k.r,
                    &[ScopeUse {
                        resource: 1,
                        storage_write: false,
                    }],
                )
                .unwrap();
            k.draw(
                (2, 2),
                area,
                &read,
                &none,
                [Some(&set), Some(&reads), None, None],
            );
            assert_eq!(passes.end_query(&mut k.r).unwrap(), 0..2);
            passes.close(&mut k.r).unwrap();
            assert_eq!(k.r.slow_paths().count(SlowPath::UsageScopeSplit), 1);
            assert_eq!(k.calls("begin_pass"), 2);
            assert_eq!(k.read(&t, 4), [7, 0, 0, 255].repeat(4));
            k.healthy();
        } else {
            let mut pass = PassDesc {
                colors: Default::default(),
                depth: None,
                empty: None,
                occlusion: None,
            };
            pass.colors[0] = Some(ColorAttachment {
                view: &t.view,
                depth_slice: 0,
                resolve: None,
                load: LoadOp::Load,
                store: StoreOp::Store,
            });
            k.r.begin_pass(&pass);
            k.draw(
                (2, 2),
                None,
                &write,
                &none,
                [Some(&set), Some(&writes), None, None],
            );
            k.draw(
                (2, 2),
                None,
                &read,
                &none,
                [Some(&set), Some(&reads), None, None],
            );
            k.r.end_pass();
            assert_eq!(
                k.r.violations,
                [format!(
                    "a pass uses buffer {} as writable storage and another way",
                    format!("{storage:?}")
                        .trim_start_matches("Id(")
                        .trim_end_matches(')')
                )]
            );
        }
    }
}

/// Exact counts by replay: draws that write depth are counted against the depth as it was at the
/// query's begin (the second quad, behind the first, passes nowhere), and draws that do not are
/// counted against the target itself.
#[test]
fn the_occlusion_counter_counts_exactly() {
    let mut k = Kit::new();
    let mut passes = PassTracker::default();
    let mut counter = OcclusionCounter::default();
    let color = k.target(4, 4, Format::Rgba8Unorm);
    let depth = k.target(4, 4, Format::Depth32Float);
    let mut targets = colors(color.clone());
    targets.depth = Some(depth.clone());
    passes.set_targets(&mut k.r, targets).unwrap();
    passes
        .clear_depth_stencil(&mut k.r, Some(0.6f32.to_bits()), None)
        .unwrap();
    let p = k.program("color");
    let counting = k.program("counting");
    let writes = k.depth(Some(CompareFn::Less), true);
    let tests = k.depth(Some(CompareFn::Less), false);
    let near = k.params(0, [1.0; 4], 0.5);
    let far = k.params(1, [1.0; 4], 0.55);
    let behind = k.params(2, [1.0; 4], 0.75);

    let mut counted = |k: &mut Kit, draws: &[(&Id, &Id)]| {
        counter.begin(&mut k.r, &mut passes).unwrap();
        for &(ds, set) in draws {
            let viewport = [Viewport {
                x: 0f32.to_bits(),
                y: 0f32.to_bits(),
                width: 4f32.to_bits(),
                height: 4f32.to_bits(),
                min_depth: 0f32.to_bits(),
                max_depth: 1f32.to_bits(),
            }];
            let streams = [Some(VertexBinding {
                buffer: &k.quad,
                offset: 0,
            })];
            let call = DrawCall {
                program: &p,
                blend: &k.blend,
                depth_stencil: ds,
                raster: &k.raster,
                vertex_layout: &k.layout,
                bindings: [Some(set), None, None, None],
                vertex_buffers: &streams,
                index: None,
                topology: Topology::TriangleStrip,
                restart: false,
                viewports: &viewport,
                scissors: &[],
                blend_constant: [0; 4],
                stencil_reference: [0; 2],
                immediates: &[],
                range: DrawRange::Vertices {
                    first: 0,
                    count: 4,
                    first_instance: 0,
                    instances: 1,
                },
            };
            let writes_depth = std::ptr::eq(ds, &writes);
            counter
                .draw(&mut k.r, &mut passes, &call, &counting, false, writes_depth)
                .unwrap();
            passes.before_draw(&mut k.r, &[]).unwrap();
            k.r.draw(&call);
        }
        counter.end(&mut k.r, &mut passes).unwrap()
    };
    // Depth 0.6 everywhere: the quad at 0.5 passes its 16 samples, the one at 0.75 none.
    assert_eq!(counted(&mut k, &[(&tests, &near), (&tests, &behind)]), 16);
    let copies = k.calls("copy_texture");
    assert_eq!(copies, 0);
    // Writing 0.5 everywhere, then a quad at 0.55 behind it: 16, then 0.
    assert_eq!(counted(&mut k, &[(&writes, &near), (&tests, &far)]), 16);
    assert_eq!(k.calls("copy_texture"), 1);
    // The depth is now 0.5: nothing passes.
    assert_eq!(counted(&mut k, &[(&tests, &far)]), 0);
    assert_eq!(k.r.slow_paths().count(SlowPath::OcclusionCountReplay), 3);
    k.healthy();
}

#[test]
fn the_combination_tracker_picks_the_tuples_drawn() {
    // Vertex shaders are letters, programs pairs, tuples numbers; tuple 30 has a vertex layout the
    // shader "c" cannot take.
    let mut t = CombinationTracker::<char, (char, char), u32>::default();
    for (vertex, program, tuple, draws) in [
        ('a', ('a', 'x'), 10, 3),
        ('a', ('a', 'x'), 20, 1),
        ('b', ('b', 'y'), 30, 5),
        ('b', ('b', 'y'), 20, 1),
    ] {
        for _ in 0..draws {
            t.drawn(&vertex, &program, &tuple);
        }
    }
    // A new pairing of a drawn vertex shader takes every tuple it was drawn with.
    assert_eq!(t.pick(&'a', &('a', 'z'), |_| true), [10, 20]);
    // An undrawn one takes the most drawn tuple that fits: 30 (5 draws) does not, 10 (3) does.
    assert_eq!(t.pick(&'c', &('c', 'z'), |&tuple| tuple != 30), [10]);
    // Equal counts pick the smaller tuple.
    t.record(&'d', &40, 3);
    assert_eq!(t.pick(&'e', &('e', 'z'), |&tuple| tuple != 30), [10]);
    // The cache's record counts as drawn for its vertex shader.
    assert_eq!(t.pick(&'d', &('d', 'z'), |_| true), [40]);
    // Of the five prepared combinations, two are drawn later.
    t.drawn(&'a', &('a', 'z'), &20);
    t.drawn(&'c', &('c', 'z'), &10);
    t.drawn(&'c', &('c', 'z'), &10);
    assert_eq!(t.prepared_use(), (2, 3));
    // Preparing a combination again counts it once.
    t.pick(&'a', &('a', 'z'), |_| true);
    assert_eq!(t.prepared_use(), (2, 3));
}
