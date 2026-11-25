//! The interface's shared helpers (the pass tracker, the occlusion counter and the combination
//! tracker) driving the recording back end through scripted draws.

use wcontainers_gpu::*;
use wcontainers_gpu_test::{Call, Id, Recorder};
use wcontainers_shaders::front::cut_scissors;

const PROGRAMS: &str = "
struct Params { color: vec4<f32>, depth: f32 }
@group(0) @binding(0) var<uniform> params: Params;
@group(1) @binding(160) var<storage, read_write> written: array<u32>;
@group(1) @binding(161) var<storage, read> source: array<u32>;
@vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p, params.depth, 1.0);
}
@vertex fn shifted(@location(0) p: vec2<f32>, @location(1) s: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p + s, params.depth, 1.0);
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
@fragment fn per_sample(@builtin(sample_index) i: u32) -> @location(0) vec4<f32> {
    return vec4<f32>(f32(i) / 3.0, 0.0, 0.0, 1.0);
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
        naga::valid::Capabilities::MULTISAMPLED_SHADING,
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
        Kit::on(Recorder::new())
    }

    fn on(mut r: Recorder) -> Kit {
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
        self.target_of(width, height, format, 1)
    }

    fn target_of(
        &mut self,
        width: u32,
        height: u32,
        format: Format,
        samples: u32,
    ) -> Target<Recorder> {
        let texture = self
            .r
            .create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width,
                height,
                depth_or_layers: 1,
                mip_levels: 1,
                samples,
                usage: TextureUsage::RENDER
                    | TextureUsage::COPY_SRC
                    | TextureUsage::COPY_DST
                    | TextureUsage::SAMPLE,
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
            samples,
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

/// A multisampled colour target larger than the pass, its common area loaded into and stored from a
/// temporary by draws: each sample read and written where the back end reads samples in programs,
/// otherwise through the resolved value (the listed loss of plan 4.7). Each sample of the larger
/// target is read afterwards by a program loading it.
fn multisampled_targets_fitted(direct: bool) {
    let mut k = if direct {
        Kit::new()
    } else {
        Kit::on(Recorder::with_caps(|c| {
            c.features.remove(Features::MULTISAMPLE_READ)
        }))
    };
    let mut passes = PassTracker::default();
    let small = k.target_of(2, 2, Format::Rgba8Unorm, 4);
    let large = k.target_of(3, 3, Format::Rgba8Unorm, 4);
    let none = k.depth(None, false);
    let set = k.params(0, [1.0, 1.0, 1.0, 1.0], 0.5);
    // Sample i of every texel of the larger target holds i / 3 in red.
    let per_sample = k.program("per_sample");
    passes.set_targets(&mut k.r, colors(large.clone())).unwrap();
    let area = passes.before_draw(&mut k.r, &[]).unwrap();
    k.draw(
        (3, 3),
        area,
        &per_sample,
        &none,
        [Some(&set), None, None, None],
    );

    // Both as targets of one pass of the smaller one's size; a draw covers the top row.
    let mut targets = colors(small.clone());
    targets.colors[1] = Some(ColorTarget {
        target: large.clone(),
        resolve: None,
    });
    passes.set_targets(&mut k.r, targets).unwrap();
    let two = k.program("two");
    let area = passes.before_draw(&mut k.r, &[]).unwrap();
    assert_eq!(area.map(|a| (a.width, a.height)), Some((2, 2)));
    k.draw((2, 1), area, &two, &none, [Some(&set), None, None, None]);
    passes.close(&mut k.r).unwrap();
    assert_eq!(k.calls("copy_texture"), 0);
    assert_eq!(k.r.slow_paths().count(SlowPath::AttachmentsFitted), 1);
    let resolved = k.r.slow_paths().count(SlowPath::MultisampleCopyResolved);
    assert_eq!(resolved, if direct { 0 } else { 2 });

    // Every sample of the larger target, as the recording back end holds them: sample i's red in
    // channel i (a back end without MULTISAMPLE_READ cannot load them in a program).
    let held = k.r.texture_bytes(large.texture).unwrap();
    let got: Vec<u8> = (0..9)
        .flat_map(|t| (0..4).map(move |s| (s, t)))
        .map(|(s, t)| held[s * 36 + t * 4])
        .collect();
    for y in 0..3 {
        for x in 0..3 {
            let at = (y * 3 + x) * 4;
            let texel = &got[at..at + 4];
            if x < 2 && y == 0 {
                assert_eq!(texel, [255; 4], "drawn at ({x}, {y})");
            } else if x < 2 && y == 1 && !direct {
                // Loaded and stored through the resolved value, the same in every sample.
                assert!(
                    texel.iter().all(|&v| v == texel[0]) && (127..=128).contains(&texel[0]),
                    "resolved at ({x}, {y}): {texel:?}"
                );
            } else {
                assert_eq!(texel, [0, 85, 170, 255], "kept at ({x}, {y})");
            }
        }
    }
    k.healthy();
}

#[test]
fn multisampled_targets_of_different_sizes_are_fitted_sample_by_sample() {
    multisampled_targets_fitted(true);
}

#[test]
fn multisampled_targets_of_different_sizes_are_fitted_through_the_resolved_value() {
    multisampled_targets_fitted(false);
}

/// Integer samples cannot be resolved, so without reading samples in programs the tracker reports
/// the fitting absent.
#[test]
fn integer_multisampled_targets_need_multisample_reads_to_be_fitted() {
    let mut k = Kit::on(Recorder::with_caps(|c| {
        c.features.remove(Features::MULTISAMPLE_READ)
    }));
    let mut passes = PassTracker::default();
    let small = k.target_of(2, 2, Format::Rgba8Uint, 4);
    let large = k.target_of(3, 3, Format::Rgba8Uint, 4);
    let mut targets = colors(small);
    targets.colors[1] = Some(ColorTarget {
        target: large,
        resolve: None,
    });
    passes.set_targets(&mut k.r, targets).unwrap();
    assert_eq!(
        passes.before_draw(&mut k.r, &[]),
        Err(GpuError::Absent(Features::MULTISAMPLE_READ))
    );
}

/// An explicit pass on a colour target and a depth buffer cleared to 1.0 with `depth` ops, whose
/// first draw writes storage and depth 0.25 in the left column and whose second reads the storage
/// with depth Less at 0.5 over everything, writing (7, 0, 0). Gives the tracker's answer to the
/// second draw, the colour target and the kit.
fn split_explicit(
    depth: AspectOps<u32>,
) -> (Result<Option<ScissorRect>, GpuError>, Target<Recorder>, Kit) {
    let mut k = Kit::new();
    let color = k.target(2, 2, Format::Rgba8Unorm);
    let depth_target = k.target(2, 2, Format::Depth32Float);
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
    let less = k.depth(Some(CompareFn::Less), !depth.read_only);
    let near = k.params(0, [0.0, 1.0, 0.0, 1.0], 0.25);
    let far = k.params(1, [0.0, 0.0, 1.0, 1.0], 0.5);
    let mut targets = colors(color.clone());
    targets.depth = Some(depth_target);
    let mut ops = PassOps::LOAD_STORE;
    ops.colors[0] = (LoadOp::Clear(f32s([0.0, 0.0, 0.0, 1.0])), StoreOp::Store);
    ops.depth = depth;
    let mut passes = PassTracker::default();
    passes.begin(&mut k.r, targets, ops).unwrap();
    let area = passes
        .before_draw(
            &mut k.r,
            &[ScopeUse {
                resource: 1,
                storage_write: true,
            }],
        )
        .unwrap();
    assert_eq!(area, None);
    k.draw(
        (2, 2),
        Some(ScissorRect {
            x: 0,
            y: 0,
            width: 1,
            height: 2,
        }),
        &write,
        &less,
        [Some(&near), Some(&writes), None, None],
    );
    let answer = passes.before_draw(
        &mut k.r,
        &[ScopeUse {
            resource: 1,
            storage_write: false,
        }],
    );
    if let Ok(area) = answer {
        assert_eq!(area, None);
        k.draw(
            (2, 2),
            area,
            &read,
            &less,
            [Some(&far), Some(&reads), None, None],
        );
        passes.end(&mut k.r).unwrap();
    }
    (answer, color, k)
}

/// A split explicit pass ends its first part with Store and begins the second with Load and the
/// pass's own store and read-only aspects, so the second draw is tested against the depth the first
/// wrote.
#[test]
fn a_split_explicit_pass_keeps_its_depth_and_ops_between_parts() {
    let (answer, color, mut k) = split_explicit(AspectOps {
        load: LoadOp::Clear(1.0f32.to_bits()),
        store: StoreOp::Store,
        read_only: false,
    });
    assert!(answer.is_ok());
    assert_eq!(k.r.slow_paths().count(SlowPath::UsageScopeSplit), 1);
    let mut want = Vec::new();
    for _y in 0..2 {
        for x in 0..2 {
            want.extend_from_slice(if x == 0 {
                &[0, 255, 0, 255]
            } else {
                &[7, 0, 0, 255]
            });
        }
    }
    assert_eq!(k.read(&color, 4), want);
    k.healthy();

    // Read-only depth stays read-only in the second part.
    let (answer, _, mut k) = split_explicit(AspectOps {
        load: LoadOp::Load,
        store: StoreOp::Store,
        read_only: true,
    });
    assert!(answer.is_ok());
    let passes: Vec<_> =
        k.r.calls
            .iter()
            .filter_map(|c| match c {
                Call::BeginPass(p) => Some(p.clone()),
                _ => None,
            })
            .collect();
    assert_eq!(passes.len(), 2);
    let depth = passes[1].depth.unwrap().1.unwrap();
    assert!(depth.read_only);
    assert_eq!(depth.load, LoadOp::Load);
    assert_eq!(passes[1].colors[0].unwrap().3, LoadOp::Load);
    k.healthy();
}

/// A split of an explicit pass that discards an attachment would lose what its first part drew, so
/// the tracker refuses it.
#[test]
fn a_split_of_a_pass_that_discards_is_refused() {
    let (answer, _, mut k) = split_explicit(AspectOps {
        load: LoadOp::Clear(1.0f32.to_bits()),
        store: StoreOp::Discard,
        read_only: false,
    });
    assert!(matches!(answer, Err(GpuError::Invalid(_))));
    assert_eq!(k.r.slow_paths().count(SlowPath::UsageScopeSplit), 0);
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
                .draw(
                    &mut k.r,
                    &mut passes,
                    &call,
                    &facts(&counting, writes_depth),
                )
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

/// `end_into` writes the count into a buffer of the front's as a u64 in order with the writes
/// around it: an earlier write there is replaced, a later one replaces it, and the words beside it
/// stay. The recorder computes with no workgroup memory, so this is the default (read back).
#[test]
fn the_occlusion_counter_writes_its_count_into_a_buffer_in_order() {
    let mut k = Kit::new();
    let mut passes = PassTracker::default();
    let mut counter = OcclusionCounter::default();
    let color = k.target(4, 4, Format::Rgba8Unorm);
    passes.set_targets(&mut k.r, colors(color)).unwrap();
    let p = k.program("color");
    let counting = k.program("counting");
    let tests = k.depth(None, false);
    let set = k.params(0, [1.0; 4], 0.5);
    let heap =
        k.r.create_buffer(&BufferDesc {
            size: 24,
            usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .unwrap();
    k.r.write_buffer(&heap, 0, &[0xab; 24], WriteMode::Ordered);
    counter.begin(&mut k.r, &mut passes).unwrap();
    let viewport = [Viewport {
        x: 0f32.to_bits(),
        y: 0f32.to_bits(),
        width: 4f32.to_bits(),
        height: 2f32.to_bits(),
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
        depth_stencil: &tests,
        raster: &k.raster,
        vertex_layout: &k.layout,
        bindings: [Some(&set), None, None, None],
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
    counter
        .draw(&mut k.r, &mut passes, &call, &facts(&counting, false))
        .unwrap();
    passes.before_draw(&mut k.r, &[]).unwrap();
    k.r.draw(&call);
    counter.end_into(&mut k.r, &mut passes, &heap, 8).unwrap();
    passes.close(&mut k.r).unwrap();
    let read = |k: &mut Kit| {
        let mut bytes = [0u8; 24];
        let src = ReadSource::Buffer {
            buffer: &heap,
            offset: 0,
            size: 24,
        };
        k.r.read_back_now(&src, &mut bytes).unwrap();
        bytes
    };
    let mut expected = [0xab; 24];
    expected[8..16].copy_from_slice(&8u64.to_le_bytes());
    assert_eq!(read(&mut k), expected);
    k.r.write_buffer(&heap, 12, &[1; 4], WriteMode::Ordered);
    expected[12..16].fill(1);
    assert_eq!(read(&mut k), expected);
    k.healthy();
}

/// Where a pixel of the counting target reaches the counting format's exact bound (2048 in
/// `r16float`, which a back end without blendable `r32float` counts in), the draws are counted again
/// in halves: of the draws, of one draw's instances, or of one list's primitives.
#[test]
fn the_occlusion_counter_counts_past_its_formats_bound_in_halves() {
    let mut k = Kit::on(Recorder::with_caps(|c| {
        let at = Format::R32Float as usize;
        c.formats.uses[at].remove(FormatUse::BLEND);
    }));
    let mut passes = PassTracker::default();
    let mut counter = OcclusionCounter::default();
    let color = k.target(2, 2, Format::Rgba8Unorm);
    passes.set_targets(&mut k.r, colors(color)).unwrap();
    let p = k.program("color");
    let counting = k.program("counting");
    let none = k.depth(None, false);
    let set = k.params(0, [1.0; 4], 0.5);
    // 2,500 copies of one triangle over the whole target, as a list.
    let corners: Vec<u8> = [-1.0f32, -1.0, 3.0, -1.0, -1.0, 3.0]
        .repeat(2500)
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let list =
        k.r.create_buffer(&BufferDesc {
            size: corners.len() as u64,
            usage: BufferUsage::VERTEX | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .unwrap();
    k.r.write_buffer(&list, 0, &corners, WriteMode::Discard);

    // Each draw: (vertex buffer, topology, vertices, instances).
    let mut counted = |k: &mut Kit, draws: &[(&Id, Topology, u32, u32)]| {
        counter.begin(&mut k.r, &mut passes).unwrap();
        for &(vertices, topology, count, instances) in draws {
            let viewport = [Viewport {
                x: 0f32.to_bits(),
                y: 0f32.to_bits(),
                width: 2f32.to_bits(),
                height: 2f32.to_bits(),
                min_depth: 0f32.to_bits(),
                max_depth: 1f32.to_bits(),
            }];
            let streams = [Some(VertexBinding {
                buffer: vertices,
                offset: 0,
            })];
            let call = DrawCall {
                program: &p,
                blend: &k.blend,
                depth_stencil: &none,
                raster: &k.raster,
                vertex_layout: &k.layout,
                bindings: [Some(&set), None, None, None],
                vertex_buffers: &streams,
                index: None,
                topology,
                restart: false,
                viewports: &viewport,
                scissors: &[],
                blend_constant: [0; 4],
                stencil_reference: [0; 2],
                immediates: &[],
                range: DrawRange::Vertices {
                    first: 0,
                    count,
                    first_instance: 0,
                    instances,
                },
            };
            counter
                .draw(&mut k.r, &mut passes, &call, &facts(&counting, false))
                .unwrap();
            passes.before_draw(&mut k.r, &[]).unwrap();
            k.r.draw(&call);
        }
        let before = k.calls("draw");
        let n = counter.end(&mut k.r, &mut passes).unwrap();
        (n, k.calls("draw") - before)
    };
    let quad = k.quad;
    let strip = Topology::TriangleStrip;
    // One draw of 3,000 instances: counted whole (full), then as 1,500 and 1,500.
    assert_eq!(counted(&mut k, &[(&quad, strip, 4, 3000)]), (12_000, 3));
    // One list of 2,500 triangles: whole, then 1,250 and 1,250.
    let list_draw = (&list, Topology::TriangleList, 7500, 1);
    assert_eq!(counted(&mut k, &[list_draw]), (10_000, 3));
    // Three draws of 1,000 instances: whole, then the first alone and the other two together.
    let thousand = (&quad, strip, 4, 1000);
    assert_eq!(
        counted(&mut k, &[thousand, thousand, thousand]),
        (12_000, 6)
    );
    // Below the bound, counted once.
    assert_eq!(counted(&mut k, &[(&quad, strip, 4, 2000)]), (8_000, 1));
    k.healthy();
}

/// One draw's halves keep what each instance reads. Where the first instance cannot be raised (no
/// `BASE_VERTEX_INSTANCE`, or a per-instance stream stepping other than by one) the draw is split by
/// its primitives, each half drawn for every instance; one primitive's instances past the bound are
/// refused.
#[test]
fn the_occlusion_counter_splits_instances_only_where_each_reads_the_same() {
    for bases in [true, false] {
        let mut k = Kit::on(Recorder::with_caps(|c| {
            c.formats.uses[Format::R32Float as usize].remove(FormatUse::BLEND);
            if !bases {
                c.features.remove(Features::BASE_VERTEX_INSTANCE);
            }
        }));
        let mut passes = PassTracker::default();
        let mut counter = OcclusionCounter::default();
        let color = k.target(2, 2, Format::Rgba8Unorm);
        passes.set_targets(&mut k.r, colors(color)).unwrap();
        let counting = k.program("counting");
        let none = k.depth(None, false);
        let set = k.params(0, [1.0; 4], 0.5);
        let floats = |v: &[f32]| -> Vec<u8> { v.iter().flat_map(|f| f.to_le_bytes()).collect() };
        let buffer = |k: &mut Kit, bytes: &[u8]| {
            let b =
                k.r.create_buffer(&BufferDesc {
                    size: bytes.len() as u64,
                    usage: BufferUsage::VERTEX | BufferUsage::COPY_DST,
                    intent: MapIntent::Rarely,
                })
                .unwrap();
            k.r.write_buffer(&b, 0, bytes, WriteMode::Discard);
            b
        };
        // 1,000 triangles over the whole target, as a list.
        let list = buffer(
            &mut k,
            &floats(&[-1.0, -1.0, 3.0, -1.0, -1.0, 3.0].repeat(1000)),
        );
        // Per-instance shifts read with a step rate of zero: every instance reads the first, which
        // leaves the triangles in place; the others move them off the target.
        let shifts = buffer(&mut k, &floats(&[0.0, 0.0, 10.0, 10.0, 10.0, 10.0]));
        let mut desc = VertexLayoutDesc {
            streams: [None; MAX_VERTEX_STREAMS],
            attributes: [None; MAX_VERTEX_STREAMS],
        };
        for (i, step) in [StepMode::Vertex, StepMode::Instance(0)]
            .into_iter()
            .enumerate()
        {
            desc.streams[i] = Some(VertexStream { stride: 8, step });
            desc.attributes[i] = Some(VertexAttribute {
                location: i as u32,
                stream: i as u8,
                offset: 0,
                format: VertexFormat::Float32x2,
            });
        }
        let stepped = k.r.create_vertex_layout(&desc).unwrap();
        let v = shader(&mut k.r, "shifted", ShaderStage::Vertex);
        let f = shader(&mut k.r, "color", ShaderStage::Fragment);
        let shifted =
            k.r.create_program(&Program::Render {
                vertex: &v,
                fragment: Some(&f),
            })
            .unwrap();
        // Its counting variant reads the shifts as it does.
        let f = shader(&mut k.r, "counting", ShaderStage::Fragment);
        let shifted_counting =
            k.r.create_program(&Program::Render {
                vertex: &v,
                fragment: Some(&f),
            })
            .unwrap();
        let p = k.program("color");

        // One draw: (program and its counting variant, layout, its streams, topology, vertices,
        // instances, step by one).
        type Draw<'a> = ([&'a Id; 2], &'a Id, &'a [&'a Id], Topology, u32, u32, bool);
        let mut counted = |k: &mut Kit,
                           (
            [program, counting],
            layout,
            streams,
            topology,
            count,
            instances,
            by_one,
        ): Draw| {
            counter.begin(&mut k.r, &mut passes).unwrap();
            let viewport = [Viewport {
                x: 0f32.to_bits(),
                y: 0f32.to_bits(),
                width: 2f32.to_bits(),
                height: 2f32.to_bits(),
                min_depth: 0f32.to_bits(),
                max_depth: 1f32.to_bits(),
            }];
            let streams: Vec<_> = streams
                .iter()
                .map(|&buffer| Some(VertexBinding { buffer, offset: 0 }))
                .collect();
            let call = DrawCall {
                program,
                blend: &k.blend,
                depth_stencil: &none,
                raster: &k.raster,
                vertex_layout: layout,
                bindings: [Some(&set), None, None, None],
                vertex_buffers: &streams,
                index: None,
                topology,
                restart: false,
                viewports: &viewport,
                scissors: &[],
                blend_constant: [0; 4],
                stencil_reference: [0; 2],
                immediates: &[],
                range: DrawRange::Vertices {
                    first: 0,
                    count,
                    first_instance: 0,
                    instances,
                },
            };
            let facts = CountedDraw {
                instances_step_by_one: by_one,
                ..facts(counting, false)
            };
            counter.draw(&mut k.r, &mut passes, &call, &facts).unwrap();
            passes.before_draw(&mut k.r, &[]).unwrap();
            k.r.draw(&call);
            let before = k.calls("draw");
            let n = counter.end(&mut k.r, &mut passes);
            (n, k.calls("draw") - before)
        };
        let list_streams = [&list, &shifts];
        // 1,000 triangles, 3 instances each, all in place: 3,000 a pixel, counted whole, then by
        // 500 triangles for all three instances twice.
        let stepped_list = (
            [&shifted, &shifted_counting],
            &stepped,
            &list_streams[..],
            Topology::TriangleList,
            3000,
            3,
            false,
        );
        assert_eq!(
            counted(&mut k, stepped_list),
            (Ok(12_000), 3),
            "bases {bases}"
        );
        let (quad, layout) = (k.quad, k.layout);
        let plain_list = (
            [&p, &counting],
            &layout,
            &list_streams[..1],
            Topology::TriangleList,
            3000,
            3,
            true,
        );
        assert_eq!(
            counted(&mut k, plain_list),
            (Ok(12_000), 3),
            "bases {bases}"
        );
        // The quad's two triangles 3,000 times: by instances where the first can be raised; one
        // triangle's 3,000 instances cannot be split otherwise.
        let quads = (
            [&p, &counting],
            &layout,
            &[&quad][..],
            Topology::TriangleStrip,
            4,
            3000,
            true,
        );
        if bases {
            assert_eq!(counted(&mut k, quads), (Ok(12_000), 3));
        } else {
            let (n, _) = counted(&mut k, quads);
            assert_eq!(
                n,
                Err(GpuError::Invalid(
                    "a draw reaching the counting format's bound at a pixel that cannot be split: a fan, a strip with restart, an indirect draw, or one primitive's instances where the first instance cannot be raised"
                ))
            );
        }
        k.healthy();
    }
}

/// What the counter needs of a draw of these tests: its counting program, no alpha to coverage, and
/// whether it writes depth; the layouts here have no per-instance stream.
fn facts(counting: &Id, writes_depth_stencil: bool) -> CountedDraw<'_, Recorder> {
    CountedDraw {
        counting,
        alpha_to_coverage: false,
        writes_depth_stencil,
        instances_step_by_one: true,
    }
}

/// One draw of a counted query: `count` vertices of `vertices` (the quad where None) as a strip,
/// `instances` times, at the target's size, with the counting program `counting` kept first.
#[allow(clippy::too_many_arguments)]
fn counted_draw(
    k: &mut Kit,
    passes: &mut PassTracker<Recorder>,
    counter: &mut OcclusionCounter<Recorder>,
    (p, counting): (&Id, &Id),
    (ds, writes): (&Id, bool),
    set: &Id,
    size: f32,
    (vertices, count, instances): (Option<Id>, u32, u32),
) -> Result<(), GpuError> {
    let vertices = vertices.unwrap_or(k.quad);
    let viewport = [Viewport {
        x: 0f32.to_bits(),
        y: 0f32.to_bits(),
        width: size.to_bits(),
        height: size.to_bits(),
        min_depth: 0f32.to_bits(),
        max_depth: 1f32.to_bits(),
    }];
    let streams = [Some(VertexBinding {
        buffer: &vertices,
        offset: 0,
    })];
    let call = DrawCall {
        program: p,
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
            count,
            first_instance: 0,
            instances,
        },
    };
    counter.draw(&mut k.r, passes, &call, &facts(counting, writes))?;
    passes.before_draw(&mut k.r, &[])?;
    k.r.draw(&call);
    Ok(())
}

/// A colour target and a depth target of `size` squared, the depth cleared to `depth`.
fn depth_targets(k: &mut Kit, size: u32, depth: f32) -> PassTargets<Recorder> {
    depth_targets_of(k, size, depth, 1)
}

fn depth_targets_of(k: &mut Kit, size: u32, depth: f32, samples: u32) -> PassTargets<Recorder> {
    let mut targets = colors(k.target_of(size, size, Format::Rgba8Unorm, samples));
    targets.depth = Some(k.target_of(size, size, Format::Depth32Float, samples));
    let mut clear = PassTracker::default();
    clear.set_targets(&mut k.r, targets.clone()).unwrap();
    clear
        .clear_depth_stencil(&mut k.r, Some(depth.to_bits()), None)
        .unwrap();
    clear.close(&mut k.r).unwrap();
    targets
}

/// A counted query's draws see what they saw when drawn: a buffer the front writes between them,
/// and the targets it changes between them, are counted as they were, the front flushing the
/// counter first. A change of targets without a flush is refused.
#[test]
fn the_occlusion_counter_counts_across_writes_and_target_changes() {
    let mut k = Kit::new();
    let mut passes = PassTracker::default();
    let mut counter = OcclusionCounter::default();
    let p = k.program("color");
    let counting = k.program("counting");
    let programs = (&p, &counting);
    let tests = k.depth(Some(CompareFn::Less), false);
    let a = depth_targets(&mut k, 4, 0.6);
    let b = depth_targets(&mut k, 4, 0.4);
    passes.set_targets(&mut k.r, a.clone()).unwrap();
    // One binding set over one buffer, written between the draws.
    let set = k.params(0, [1.0; 4], 0.5);
    let write = |k: &mut Kit, depth: f32| {
        let bytes: Vec<u8> = [1.0f32, 1.0, 1.0, 1.0, depth]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        k.r.write_buffer(&k.uniforms, 0, &bytes, WriteMode::Ordered);
    };

    // At 0.5 the quad passes its 16 samples against 0.6; rewritten to 0.75, none.
    counter.begin(&mut k.r, &mut passes).unwrap();
    counted_draw(
        &mut k,
        &mut passes,
        &mut counter,
        programs,
        (&tests, false),
        &set,
        4.0,
        (None, 4, 1),
    )
    .unwrap();
    counter.flush(&mut k.r, &mut passes).unwrap();
    write(&mut k, 0.75);
    counted_draw(
        &mut k,
        &mut passes,
        &mut counter,
        programs,
        (&tests, false),
        &set,
        4.0,
        (None, 4, 1),
    )
    .unwrap();
    assert_eq!(counter.end(&mut k.r, &mut passes).unwrap(), 16);

    // At 0.5 against target A (0.6): 16; then against target B (0.4): none.
    write(&mut k, 0.5);
    counter.begin(&mut k.r, &mut passes).unwrap();
    counted_draw(
        &mut k,
        &mut passes,
        &mut counter,
        programs,
        (&tests, false),
        &set,
        4.0,
        (None, 4, 1),
    )
    .unwrap();
    counter.flush(&mut k.r, &mut passes).unwrap();
    passes.set_targets(&mut k.r, b.clone()).unwrap();
    counted_draw(
        &mut k,
        &mut passes,
        &mut counter,
        programs,
        (&tests, false),
        &set,
        4.0,
        (None, 4, 1),
    )
    .unwrap();
    counter.flush(&mut k.r, &mut passes).unwrap();
    passes.set_targets(&mut k.r, a.clone()).unwrap();
    counted_draw(
        &mut k,
        &mut passes,
        &mut counter,
        programs,
        (&tests, false),
        &set,
        4.0,
        (None, 4, 1),
    )
    .unwrap();
    assert_eq!(counter.end(&mut k.r, &mut passes).unwrap(), 32);
    assert_eq!(k.r.slow_paths().count(SlowPath::OcclusionCountReplay), 5);

    // The targets changed without a flush.
    counter.begin(&mut k.r, &mut passes).unwrap();
    counted_draw(
        &mut k,
        &mut passes,
        &mut counter,
        programs,
        (&tests, false),
        &set,
        4.0,
        (None, 4, 1),
    )
    .unwrap();
    passes.set_targets(&mut k.r, b).unwrap();
    let refused = Err(GpuError::Invalid(
        "the targets of a counted query changed without a flush",
    ));
    assert_eq!(
        counted_draw(
            &mut k,
            &mut passes,
            &mut counter,
            programs,
            (&tests, false),
            &set,
            4.0,
            (None, 4, 1)
        ),
        refused
    );
    assert_eq!(counter.end(&mut k.r, &mut passes).map(|_| ()), refused);
    k.healthy();
}

/// Past the counting format's bound with draws that write depth, each half is counted from the
/// depth the whole began with, the second after the first: 3,000 instances of a quad at 0.5 writing
/// depth with `LessEqual` over depth 1.0 (all pass, 12,000), then a quad at 0.3 writing it with
/// `Less` (4). Counted from the depth the whole left, the first would pass nowhere. A strip of
/// 3,000 triangles over the target is split into strips of whole pairs.
#[test]
fn the_occlusion_counter_counts_halves_from_the_depth_they_began_with() {
    let mut k = Kit::on(Recorder::with_caps(|c| {
        let at = Format::R32Float as usize;
        c.formats.uses[at].remove(FormatUse::BLEND);
    }));
    let mut passes = PassTracker::default();
    let mut counter = OcclusionCounter::default();
    let p = k.program("color");
    let counting = k.program("counting");
    let programs = (&p, &counting);
    let writes = k.depth(Some(CompareFn::LessEqual), true);
    let nearer = k.depth(Some(CompareFn::Less), true);
    let targets = depth_targets(&mut k, 2, 1.0);
    passes.set_targets(&mut k.r, targets).unwrap();
    let near = k.params(0, [1.0; 4], 0.5);
    let front = k.params(1, [1.0; 4], 0.3);
    counter.begin(&mut k.r, &mut passes).unwrap();
    counted_draw(
        &mut k,
        &mut passes,
        &mut counter,
        programs,
        (&writes, true),
        &near,
        2.0,
        (None, 4, 3000),
    )
    .unwrap();
    counted_draw(
        &mut k,
        &mut passes,
        &mut counter,
        programs,
        (&nearer, true),
        &front,
        2.0,
        (None, 4, 1),
    )
    .unwrap();
    assert_eq!(counter.end(&mut k.r, &mut passes).unwrap(), 12_004);

    // A strip of 3,002 vertices cycling through one triangle's corners over the target: 3,000
    // triangles, each covering every pixel. Counted whole (full), then as two strips of 1,500.
    let corners: Vec<u8> = [-1.0f32, -1.0, 3.0, -1.0, -1.0, 3.0]
        .repeat(1001)
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let strip =
        k.r.create_buffer(&BufferDesc {
            size: corners.len() as u64,
            usage: BufferUsage::VERTEX | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .unwrap();
    k.r.write_buffer(&strip, 0, &corners, WriteMode::Discard);
    let none = k.depth(None, false);
    counter.begin(&mut k.r, &mut passes).unwrap();
    counted_draw(
        &mut k,
        &mut passes,
        &mut counter,
        programs,
        (&none, false),
        &near,
        2.0,
        (Some(strip), 3002, 1),
    )
    .unwrap();
    let before = k.calls("draw");
    assert_eq!(counter.end(&mut k.r, &mut passes).unwrap(), 12_000);
    assert_eq!(k.calls("draw") - before, 3);
    k.healthy();
}

/// On four samples a pixel reaches the bound at 512 fragments a sample, so the depth the whole began
/// with is kept from there: 600 instances of a quad at 0.5 writing depth with `LessEqual` over 1.0
/// (all 16 samples pass, 9,600), then a quad at 0.3 writing it with `Less` (16).
#[test]
fn the_occlusion_counter_keeps_the_starting_depth_for_halves_on_four_samples() {
    let mut k = Kit::on(Recorder::with_caps(|c| {
        let at = Format::R32Float as usize;
        c.formats.uses[at].remove(FormatUse::BLEND);
    }));
    let mut passes = PassTracker::default();
    let mut counter = OcclusionCounter::default();
    let p = k.program("color");
    let counting = k.program("counting");
    let programs = (&p, &counting);
    let writes = k.depth(Some(CompareFn::LessEqual), true);
    let nearer = k.depth(Some(CompareFn::Less), true);
    let targets = depth_targets_of(&mut k, 2, 1.0, 4);
    passes.set_targets(&mut k.r, targets).unwrap();
    let near = k.params(0, [1.0; 4], 0.5);
    let front = k.params(1, [1.0; 4], 0.3);
    counter.begin(&mut k.r, &mut passes).unwrap();
    for (depth, params, instances) in [(&writes, &near, 600), (&nearer, &front, 1)] {
        counted_draw(
            &mut k,
            &mut passes,
            &mut counter,
            programs,
            (depth, true),
            params,
            2.0,
            (None, 4, instances),
        )
        .unwrap();
    }
    assert_eq!(counter.end(&mut k.r, &mut passes).unwrap(), 9_616);
    k.healthy();
}

/// On a four-sample target the counter counts samples: a rectangle whose right edge crosses pixel 1
/// at x = 1.25 covers its sample 2 alone (the standard positions), so two rows hold 4 + 1 samples
/// each.
#[test]
fn the_occlusion_counter_counts_the_samples_of_multisampled_targets() {
    let mut k = Kit::new();
    let mut passes = PassTracker::default();
    let mut counter = OcclusionCounter::default();
    let color = k.target_of(2, 2, Format::Rgba8Unorm, 4);
    passes.set_targets(&mut k.r, colors(color)).unwrap();
    let p = k.program("color");
    let counting = k.program("counting");
    let none = k.depth(None, false);
    let set = k.params(0, [1.0; 4], 0.5);
    let corners: Vec<u8> = [-1.0f32, -1.0, -1.0, 1.0, 0.25, -1.0, 0.25, 1.0]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let rect =
        k.r.create_buffer(&BufferDesc {
            size: 32,
            usage: BufferUsage::VERTEX | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .unwrap();
    k.r.write_buffer(&rect, 0, &corners, WriteMode::Discard);
    for (vertices, want) in [(rect, 10), (k.quad, 16)] {
        let viewport = [Viewport {
            x: 0f32.to_bits(),
            y: 0f32.to_bits(),
            width: 2f32.to_bits(),
            height: 2f32.to_bits(),
            min_depth: 0f32.to_bits(),
            max_depth: 1f32.to_bits(),
        }];
        let streams = [Some(VertexBinding {
            buffer: &vertices,
            offset: 0,
        })];
        let call = DrawCall {
            program: &p,
            blend: &k.blend,
            depth_stencil: &none,
            raster: &k.raster,
            vertex_layout: &k.layout,
            bindings: [Some(&set), None, None, None],
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
        counter.begin(&mut k.r, &mut passes).unwrap();
        counter
            .draw(&mut k.r, &mut passes, &call, &facts(&counting, false))
            .unwrap();
        passes.before_draw(&mut k.r, &[]).unwrap();
        k.r.draw(&call);
        assert_eq!(counter.end(&mut k.r, &mut passes).unwrap(), want);
    }
    assert_eq!(k.calls("resolve"), 2);
    k.healthy();
}
