//! Every format ability `CheckFeatureSupport` answers present is used through the device on a real
//! GPU, which must serve it: textures of each shape, mip chains, targets and depth targets cleared,
//! multisampled ones of every count answered, resolves, storage textures, copies between the
//! formats of a group, and textures of the formats answered absent are refused; and the uses that
//! need a program, from programs fxc compiled (tests/answers/compile.cmd): loads, sampling,
//! comparison, gathering, multisampled loads, typed buffer views, typed storage stores, loads and
//! atomics, vertex and index input, and drawing into targets (blended where answered) and depth
//! targets; loads of a texel of varied components, of the formats of whole-byte components alike,
//! checked against Direct3D's conversions. Needs a GPU: run it through `tools/graphics-rig/gpu-job`. `WEBGPU_API` picks wgpu's
//! API (dx12, vulkan, metal).

use std::sync::Arc;

use wcontainers_direct3d_12::features::{format_support, quality_levels};
use wcontainers_direct3d_12::list::{
    Command, CommandList, CopyLocation, Descriptor, IndexBufferView, ResourceView, RootArgument,
    SRV_DIMENSION_BUFFER, SRV_DIMENSION_TEXTURE2D, SRV_DIMENSION_TEXTURE2DMS, TargetView,
    UAV_DIMENSION_BUFFER, UAV_DIMENSION_TEXTURE2D, VertexBufferView, ViewportF,
};
use wcontainers_direct3d_12::resource::{
    DIMENSION_BUFFER, DIMENSION_TEXTURE1D, DIMENSION_TEXTURE2D, DIMENSION_TEXTURE3D,
    FLAG_DEPTH_STENCIL, FLAG_RENDER_TARGET, FLAG_UNORDERED_ACCESS, FootprintInfo, HeapType,
    LAYOUT_ROW_MAJOR, ResourceDesc,
};
use wcontainers_direct3d_12::root::{
    DescriptorRange, FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT, RANGE_SRV, RANGE_UAV, RootParameter,
    RootParameterKind, RootSignature, RootSignatureDesc, StaticSampler, VERSION_1_0, serialize,
};
use wcontainers_direct3d_12::state::{
    Blend, COMPARISON_ALWAYS, COMPARISON_LESS_EQUAL, CULL_MODE_NONE, DepthStencil, FILL_MODE_SOLID,
    InputElement, Raster, TOPOLOGY_TRIANGLELIST, TOPOLOGY_TYPE_TRIANGLE,
};
use wcontainers_direct3d_12::{Device, GraphicsPipelineDesc, PipelineId, ResourceId};
use wcontainers_directx_graphics_infrastructure::required::MOST_SAMPLES;
use wcontainers_directx_graphics_infrastructure::{
    dxgi, dxgi_format, support1, support2, typeless_group,
};
use wcontainers_gpu::{Backend, CUBE_FACES, ColorWrites, DeviceStatus, FormatKind};
use wcontainers_gpu_webgpu::WebGpu;

/// The DXGI format values up to `B4G4R4A4_UNORM`, the last Direct3D 12 defines below its video
/// and sampler feedback formats.
const FORMATS: std::ops::RangeInclusive<u32> = 1..=115;
/// The side of every texture made, two compressed blocks wide.
const TEXTURE_SIDE: u32 = 8;

fn webgpu() -> WebGpu {
    let backends = match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    };
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends;
    desc.flags |= WebGpu::INSTANCE_FLAGS;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("a GPU adapter");
    let desc = WebGpu::device_descriptor(&adapter);
    let (device, queue) = pollster::block_on(adapter.request_device(&desc)).unwrap();
    WebGpu::new(&adapter, device, queue)
}

fn texture(
    format: u32,
    dimension: u32,
    layers: u16,
    mips: u16,
    samples: u32,
    flags: u32,
) -> ResourceDesc {
    ResourceDesc {
        dimension,
        width: u64::from(TEXTURE_SIDE),
        height: if dimension == DIMENSION_TEXTURE1D {
            1
        } else {
            TEXTURE_SIDE
        },
        depth_or_array_size: layers,
        mip_levels: mips,
        format,
        sample_count: samples,
        flags,
        ..Default::default()
    }
}

/// Runs `commands` and waits for them; the device must still be whole.
fn run(device: &mut Device<WebGpu>, commands: Vec<Command>, what: &str) {
    let mut list = CommandList::new();
    for c in commands {
        list.record(c);
    }
    list.close()
        .unwrap_or_else(|e| panic!("{what}: closing: {e:?}"));
    let lists: Vec<Arc<[Command]>> = vec![list.commands().unwrap().clone()];
    let queue = device.create_queue().unwrap();
    let fence = device.create_fence(0);
    device
        .execute(queue, &lists)
        .unwrap_or_else(|e| panic!("{what}: {e:?} ({:?})", device.removed()));
    device.queue_signal(queue, fence, 1).unwrap();
    assert!(device.wait(fence, 1), "{what}: the fence");
    assert_eq!(device.backend().status(), DeviceStatus::Ok, "{what}");
    assert!(device.removed().is_none(), "{what}: {:?}", device.removed());
    device.destroy_fence(fence);
    device.destroy_queue(queue);
}

/// A resource in a default heap, which the back end must have made whole.
fn make(device: &mut Device<WebGpu>, desc: &ResourceDesc, what: &str) -> ResourceId {
    let id = device
        .create_committed(HeapType::Default, desc)
        .unwrap_or_else(|e| panic!("{what}: answered, refused: {e:?}"));
    assert_eq!(device.backend().status(), DeviceStatus::Ok, "{what}: made");
    id
}

/// A target view of the first plane or the whole of a texture of `format`: an `NV12` texture is
/// drawn into through views of its planes, the first `R8_UNORM`.
fn target(resource: ResourceId, format: u32) -> TargetView {
    TargetView {
        resource,
        format: match format {
            dxgi::NV12 => dxgi::R8_UNORM,
            f => f,
        },
        mip: 0,
        first_layer: 0,
        layers: 1,
    }
}

/// Whether Direct3D casts textures of `a` and `b` within their bit layout: the formats of a typeless
/// group, and `R10G10B10_XR_BIAS_A2_UNORM` with the `R10G10B10A2` formats.
fn cast_partners(a: u32, b: u32) -> bool {
    let group = |f| match f {
        dxgi::R10G10B10_XR_BIAS_A2_UNORM => Some(dxgi::R10G10B10A2_TYPELESS),
        f => typeless_group(f),
    };
    group(a).is_some() && group(a) == group(b)
}

/// Each format on a device of its own, so that one that loses its device leaves the others'
/// checks whole; every failure is printed, then the test fails if there was one.
#[test]
fn every_answered_format_ability_is_served() {
    // Uses of resources, then of programs.
    let mut used = [0; 2];
    let mut failures = Vec::new();
    for format in FORMATS {
        for (i, uses) in [uses_of, program_uses_of].into_iter().enumerate() {
            match std::panic::catch_unwind(|| uses(format)) {
                Ok(n) => used[i] += n,
                Err(e) => failures.push(
                    e.downcast_ref::<String>()
                        .cloned()
                        .unwrap_or_else(|| format!("format {format}")),
                ),
            }
        }
    }
    eprintln!(
        "answered abilities used: {} by resources, {} by programs",
        used[0], used[1]
    );
    for f in &failures {
        eprintln!("not served: {f}");
    }
    assert!(failures.is_empty(), "not served as answered: {failures:?}");
}

/// Uses every ability answered for DXGI format `format`; how many uses were made.
fn uses_of(format: u32) -> usize {
    let mut device = Device::new(webgpu());
    let caps = device.backend().caps().clone();
    let mut used = 0;
    {
        let s = format_support(&caps, format);
        let has = |bit: u32| s.support1 & bit != 0;
        if s.is_empty() {
            // Answered absent: no texture of it is made.
            let desc = texture(format, DIMENSION_TEXTURE2D, 1, 1, 1, 0);
            assert!(
                device.create_committed(HeapType::Default, &desc).is_err(),
                "format {format}: answered absent, made"
            );
            return 0;
        }
        let shapes = [
            (support1::TEXTURE1D, DIMENSION_TEXTURE1D, 1),
            (support1::TEXTURE2D, DIMENSION_TEXTURE2D, 1),
            (support1::TEXTURE3D, DIMENSION_TEXTURE3D, 2),
            (
                support1::TEXTURECUBE,
                DIMENSION_TEXTURE2D,
                CUBE_FACES as u16,
            ),
        ];
        for (bit, dimension, layers) in shapes {
            if has(bit) {
                let mips = if has(support1::MIP) { 3 } else { 1 };
                let what = format!("format {format}: shape {bit:#x}, {mips} levels");
                let id = make(
                    &mut device,
                    &texture(format, dimension, layers, mips, 1, 0),
                    &what,
                );
                device.destroy_resource(id);
                used += 1;
            }
        }
        let depth = has(support1::DEPTH_STENCIL);
        let colour = has(support1::RENDER_TARGET);
        let flag = if depth {
            FLAG_DEPTH_STENCIL
        } else {
            FLAG_RENDER_TARGET
        };
        let clear = |view: TargetView| match depth {
            true => Command::ClearDepth {
                view: Descriptor::DepthStencil { view, read_only: 0 },
                depth: Some(1.0),
                stencil: Some(0),
                rects: vec![],
            },
            false => Command::ClearTarget {
                view: Descriptor::RenderTarget(view),
                color: [0.25, 0.5, 0.75, 1.0],
                rects: vec![],
            },
        };
        if depth || colour {
            let what = format!("format {format}: a cleared target");
            let id = make(
                &mut device,
                &texture(format, DIMENSION_TEXTURE2D, 1, 1, 1, flag),
                &what,
            );
            run(&mut device, vec![clear(target(id, format))], &what);
            for samples in 2..=MOST_SAMPLES {
                if quality_levels(&caps, format, samples).unwrap_or(0) == 0 {
                    continue;
                }
                assert!(
                    has(support1::MULTISAMPLE_RENDERTARGET),
                    "format {format}: {samples} samples, no multisampled target"
                );
                let what = format!("format {format}: a cleared target of {samples} samples");
                let ms = make(
                    &mut device,
                    &texture(format, DIMENSION_TEXTURE2D, 1, 1, samples, flag),
                    &what,
                );
                let mut commands = vec![clear(target(ms, format))];
                if has(support1::MULTISAMPLE_RESOLVE) {
                    commands.push(Command::Resolve {
                        dst: id,
                        dst_index: 0,
                        src: ms,
                        src_index: 0,
                        format,
                    });
                }
                run(&mut device, commands, &what);
                device.destroy_resource(ms);
                used += 1;
            }
            device.destroy_resource(id);
            used += 1;
        }
        if has(support1::TYPED_UNORDERED_ACCESS_VIEW) {
            let what = format!("format {format}: a storage texture");
            let id = make(
                &mut device,
                &texture(format, DIMENSION_TEXTURE2D, 1, 1, 1, FLAG_UNORDERED_ACCESS),
                &what,
            );
            device.destroy_resource(id);
            used += 1;
        }
        // A copy between this format's texture and each other format of its group.
        if has(support1::CAST_WITHIN_BIT_LAYOUT) {
            for other in FORMATS.filter(|&o| o != format && cast_partners(format, o)) {
                if format_support(&caps, other).support1 & support1::TEXTURE2D == 0 {
                    continue;
                }
                let what = format!("format {format}: copied into {other}");
                let a = make(
                    &mut device,
                    &texture(format, DIMENSION_TEXTURE2D, 1, 1, 1, 0),
                    &what,
                );
                let b = make(
                    &mut device,
                    &texture(other, DIMENSION_TEXTURE2D, 1, 1, 1, 0),
                    &what,
                );
                let at = |resource| CopyLocation::Subresource { resource, index: 0 };
                run(
                    &mut device,
                    vec![Command::CopyTexture {
                        dst: at(b),
                        at: [0; 3],
                        src: at(a),
                        area: None,
                    }],
                    &what,
                );
                device.destroy_resource(a);
                device.destroy_resource(b);
                used += 1;
            }
        }
    }
    used
}

/// The programs of tests/answers/compile.cmd; those of several component kinds are indexed by
/// [`Kind`].
mod programs {
    macro_rules! one {
        ($name:literal) => {
            include_bytes!(concat!("answers/", $name, ".dxbc")) as &[u8]
        };
    }
    macro_rules! kinds {
        ($name:literal) => {
            [
                include_bytes!(concat!("answers/", $name, "_f.dxbc")) as &[u8],
                include_bytes!(concat!("answers/", $name, "_u.dxbc")),
                include_bytes!(concat!("answers/", $name, "_i.dxbc")),
            ]
        };
    }
    pub const LOAD: [&[u8]; 3] = kinds!("load");
    pub const MULTISAMPLE_LOAD: [&[u8]; 3] = kinds!("multisample_load");
    pub const BUFFER_LOAD: [&[u8]; 3] = kinds!("buffer_load");
    pub const STORE: [&[u8]; 3] = kinds!("store");
    pub const BUFFER_STORE: [&[u8]; 3] = kinds!("buffer_store");
    pub const TYPED_LOAD: [&[u8]; 3] = kinds!("typed_load");
    pub const VERTEX: [&[u8]; 3] = kinds!("vertex");
    pub const PIXEL: [&[u8]; 3] = kinds!("pixel");
    pub const SAMPLE: &[u8] = one!("sample_f");
    pub const COMPARE: &[u8] = one!("compare_f");
    pub const GATHER: &[u8] = one!("gather_f");
    pub const GATHER_COMPARE: &[u8] = one!("gather_compare_f");
    /// A program of each kind, or none.
    pub type ByKind = [Option<&'static [u8]>; 3];
    /// Each atomic program by the kind of the view it is used on (none where Direct3D has no
    /// such atomic; minimum and maximum programs declare the operation's own sign, as a program
    /// using them on a view of the other sign does) and the four words it writes on a texel of
    /// zero: the values its operations return, then the texel's last value.
    pub const ATOMICS: [(u32, ByKind, [u32; 4]); 6] = {
        use super::support2::*;
        [
            (
                UAV_ATOMIC_ADD,
                [None, Some(one!("atomic_add_u")), Some(one!("atomic_add_i"))],
                [0, 0, 0, 5],
            ),
            (
                UAV_ATOMIC_BITWISE_OPS,
                [
                    None,
                    Some(one!("atomic_bitwise_u")),
                    Some(one!("atomic_bitwise_i")),
                ],
                [0, 6, 2, 3],
            ),
            (
                UAV_ATOMIC_COMPARE_STORE_OR_COMPARE_EXCHANGE,
                [
                    None,
                    Some(one!("atomic_compare_u")),
                    Some(one!("atomic_compare_i")),
                ],
                [7, 0, 0, 9],
            ),
            (
                UAV_ATOMIC_EXCHANGE,
                [
                    Some(one!("atomic_exchange_f")),
                    Some(one!("atomic_exchange_u")),
                    Some(one!("atomic_exchange_i")),
                ],
                [0, 0, 0, 4],
            ),
            (
                UAV_ATOMIC_SIGNED_MIN_OR_MAX,
                [
                    None,
                    Some(one!("atomic_signed_min_or_max_i")),
                    Some(one!("atomic_signed_min_or_max_i")),
                ],
                [0, 3, 0, -2i32 as u32],
            ),
            (
                UAV_ATOMIC_UNSIGNED_MIN_OR_MAX,
                [
                    None,
                    Some(one!("atomic_unsigned_min_or_max_u")),
                    Some(one!("atomic_unsigned_min_or_max_u")),
                ],
                [0, 8, 0, 2],
            ),
        ]
    };
}

/// The component kind a program declares for views of a format: float (also normalised, depth),
/// unsigned (also stencil) or signed; indexes the programs of several kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Float,
    Uint,
    Sint,
}

fn kind_of(format: u32) -> Kind {
    match format {
        dxgi::X24_TYPELESS_G8_UINT | dxgi::X32_TYPELESS_G8X24_UINT => Kind::Uint,
        _ => match dxgi_format(format).map(|(f, _)| f.kind()) {
            Some(FormatKind::Uint | FormatKind::Stencil) => Kind::Uint,
            Some(FormatKind::Sint) => Kind::Sint,
            _ => Kind::Float,
        },
    }
}

/// The format a view of a texture of `format` takes: a plane's for `NV12` (the first), the pixel
/// pairs' for `YUY2`, else the format itself.
fn view_format(format: u32) -> u32 {
    match format {
        dxgi::NV12 => dxgi::R8_UNORM,
        dxgi::YUY2 => dxgi::R8G8B8A8_UNORM,
        f => f,
    }
}

/// `D3D12_FILTER_MIN_MAG_MIP_LINEAR` and `D3D12_FILTER_COMPARISON_MIN_MAG_MIP_LINEAR`.
const FILTER_LINEAR: u32 = 0x15;
const FILTER_COMPARISON_LINEAR: u32 = 0x95;
/// `D3D12_TEXTURE_ADDRESS_MODE_CLAMP`.
const ADDRESS_CLAMP: u32 = 3;
/// `D3D12_ROOT_PARAMETER_TYPE_UAV`.
const ROOT_UAV: u32 = 4;
/// `D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING`.
const DEFAULT_MAPPING: u32 = 0x1688;
/// `D3D12_BLEND_ONE` and `D3D12_BLEND_OP_ADD`.
const BLEND_ONE: u32 = 2;
const BLEND_OP_ADD: u32 = 1;
/// The bytes of what a program writes to its result buffer: four 32-bit words.
const RESULT_BYTES: u64 = 16;
/// The bytes of a vertex or of one typed buffer's elements: four 32-bit components at most.
const ELEMENT_BYTES: u32 = 16;

fn buffer(size: u64, flags: u32) -> ResourceDesc {
    ResourceDesc {
        dimension: DIMENSION_BUFFER,
        width: size,
        height: 1,
        depth_or_array_size: 1,
        mip_levels: 1,
        sample_count: 1,
        layout: LAYOUT_ROW_MAJOR,
        flags,
        ..Default::default()
    }
}

/// A buffer in an upload heap holding `bytes`.
fn uploaded(device: &mut Device<WebGpu>, bytes: &[u8]) -> ResourceId {
    let id = device
        .create_committed(HeapType::Upload, &buffer(bytes.len() as u64, 0))
        .unwrap();
    let mapped = device.map(id).unwrap();
    mapped.iter().zip(bytes).for_each(|(c, b)| c.set(*b));
    id
}

/// What the program sees of a compute use: the result buffer at `u0` as a root view, then a table
/// of one view at `t0` and one at `u1`; a linear sampler at `s0` and a comparison one at `s1`.
struct Programs {
    compute: RootSignature,
    graphics: RootSignature,
    heap: wcontainers_direct3d_12::HeapId,
    table: u64,
}

impl Programs {
    fn new(device: &mut Device<WebGpu>) -> Programs {
        let sampler = |filter: u32, register: u32| {
            StaticSampler([
                filter,
                ADDRESS_CLAMP,
                ADDRESS_CLAMP,
                ADDRESS_CLAMP,
                0,
                1,
                COMPARISON_LESS_EQUAL,
                0,
                0,
                f32::MAX.to_bits(),
                register,
                0,
                0,
            ])
        };
        let range = |kind: u32, register: u32, offset: u32| DescriptorRange {
            kind,
            count: 1,
            register,
            space: 0,
            flags: 0,
            offset,
        };
        let compute = device
            .create_root_signature(&serialize(&RootSignatureDesc {
                version: VERSION_1_0,
                parameters: vec![
                    RootParameter {
                        kind: RootParameterKind::Descriptor {
                            kind: ROOT_UAV,
                            register: 0,
                            space: 0,
                            flags: 0,
                        },
                        visibility: 0,
                    },
                    RootParameter {
                        kind: RootParameterKind::Table(vec![
                            range(RANGE_SRV, 0, 0),
                            range(RANGE_UAV, 1, 1),
                        ]),
                        visibility: 0,
                    },
                ],
                samplers: vec![
                    sampler(FILTER_LINEAR, 0),
                    sampler(FILTER_COMPARISON_LINEAR, 1),
                ],
                flags: 0,
            }))
            .unwrap();
        let graphics = device
            .create_root_signature(&serialize(&RootSignatureDesc {
                version: VERSION_1_0,
                parameters: vec![],
                samplers: vec![],
                flags: FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
            }))
            .unwrap();
        let (heap, _, table) = device.create_descriptor_heap(2);
        Programs {
            compute,
            graphics,
            heap,
            table,
        }
    }

    /// Runs compute program `cs` once with views `t0` and `u1` after `before`; the four words it
    /// wrote.
    fn dispatch(
        &self,
        device: &mut Device<WebGpu>,
        cs: &[u8],
        views: [Descriptor; 2],
        before: Vec<Command>,
        what: &str,
    ) -> [u32; 4] {
        let pso = device
            .create_compute_pipeline(Some(&self.compute), cs)
            .unwrap_or_else(|e| panic!("{what}: the program refused: {e:?}"));
        let slots = device.descriptor_slots(self.heap).unwrap();
        slots.iter().zip(views).for_each(|(s, v)| s.set(v));
        let result = make(device, &buffer(RESULT_BYTES, FLAG_UNORDERED_ACCESS), what);
        let readback = device
            .create_committed(HeapType::Readback, &buffer(RESULT_BYTES, 0))
            .unwrap();
        let mut commands = before;
        commands.extend([
            Command::RootSignature {
                compute: true,
                root: Some(self.compute.clone()),
            },
            Command::Pipeline(pso),
            Command::RootArgument {
                compute: true,
                index: 0,
                argument: RootArgument::Descriptor(device.address(result)),
            },
            Command::RootArgument {
                compute: true,
                index: 1,
                argument: RootArgument::Table(self.table),
            },
            Command::Dispatch([1, 1, 1]),
            Command::CopyBuffer {
                dst: readback,
                dst_offset: 0,
                src: result,
                src_offset: 0,
                size: RESULT_BYTES,
            },
        ]);
        run(device, commands, what);
        let words = read_words(device, readback);
        device.destroy_resource(result);
        device.destroy_resource(readback);
        device.destroy_pipeline(pso);
        words
    }

    /// A pipeline drawing with vertex program `vs` fetching `input` and pixel program `ps` into
    /// `outputs`.
    fn pipeline(
        &self,
        device: &mut Device<WebGpu>,
        (vs, input): (&[u8], u32),
        ps: &[u8],
        outputs: Outputs,
        what: &str,
    ) -> PipelineId {
        let Outputs {
            targets,
            blended,
            depth,
            samples,
        } = outputs;
        let mut blend = Blend::default();
        blend.targets[0].write_mask = ColorWrites::all().bits();
        if blended {
            let t = &mut blend.targets[0];
            t.blend_enable = true;
            (t.src, t.dst, t.op) = (BLEND_ONE, BLEND_ONE, BLEND_OP_ADD);
            (t.src_alpha, t.dst_alpha, t.op_alpha) = (BLEND_ONE, BLEND_ONE, BLEND_OP_ADD);
        }
        let depth_stencil = DepthStencil {
            depth_enable: depth != dxgi::UNKNOWN,
            depth_write_all: true,
            depth_func: COMPARISON_ALWAYS,
            ..Default::default()
        };
        device
            .create_graphics_pipeline(&GraphicsPipelineDesc {
                root: Some(self.graphics.clone()),
                vs,
                ps,
                blend,
                sample_mask: u32::MAX,
                raster: Raster {
                    fill: FILL_MODE_SOLID,
                    cull: CULL_MODE_NONE,
                    depth_clip: true,
                    ..Default::default()
                },
                depth_stencil,
                inputs: vec![InputElement {
                    semantic: "VALUE".into(),
                    semantic_index: 0,
                    format: input,
                    slot: 0,
                    offset: 0,
                    per_instance: false,
                    step_rate: 0,
                }],
                topology_type: TOPOLOGY_TYPE_TRIANGLE,
                dsv_format: depth,
                rtv_formats: targets,
                samples,
                ..Default::default()
            })
            .unwrap_or_else(|e| panic!("{what}: the pipeline refused: {e:?}"))
    }
}

/// What a pipeline draws into: targets of `targets`, blended (adding) where `blended`, or a depth
/// target of `depth`, of `samples` samples.
struct Outputs {
    targets: Vec<u32>,
    blended: bool,
    depth: u32,
    samples: u32,
}

impl Outputs {
    fn colour(targets: Vec<u32>, blended: bool, samples: u32) -> Outputs {
        Outputs {
            targets,
            blended,
            depth: dxgi::UNKNOWN,
            samples,
        }
    }
}

/// The commands drawing the three vertices of `vertices` (zeros where none) covering a target of
/// `TEXTURE_SIDE`, indexed through `indices` of that index format where given.
fn draw(
    pso: PipelineId,
    root: &RootSignature,
    colors: Vec<Descriptor>,
    depth: Option<Descriptor>,
    vertices: u64,
    indices: Option<(u64, u32)>,
) -> Vec<Command> {
    let side = TEXTURE_SIDE as f32;
    let mut commands = vec![
        Command::RootSignature {
            compute: false,
            root: Some(root.clone()),
        },
        Command::Pipeline(pso),
        Command::Targets { colors, depth },
        Command::Viewports(vec![ViewportF {
            width: side,
            height: side,
            max_depth: 1.0,
            ..Default::default()
        }]),
        Command::Scissors(vec![[0, 0, TEXTURE_SIDE as i32, TEXTURE_SIDE as i32]]),
        Command::Topology(TOPOLOGY_TRIANGLELIST),
        Command::VertexBuffers {
            start: 0,
            views: vec![VertexBufferView {
                address: vertices,
                size: 3 * ELEMENT_BYTES,
                stride: ELEMENT_BYTES,
            }],
        },
    ];
    match indices {
        Some((address, format)) => commands.extend([
            Command::IndexBuffer(Some(IndexBufferView {
                address,
                size: 3 * size_of::<u32>() as u32,
                format,
            })),
            Command::DrawIndexed {
                indices: 3,
                instances: 1,
                first_index: 0,
                base_vertex: 0,
                first_instance: 0,
            },
        ]),
        None => commands.push(Command::Draw {
            vertices: 3,
            instances: 1,
            first_vertex: 0,
            first_instance: 0,
        }),
    }
    commands
}

fn read_words(device: &mut Device<WebGpu>, readback: ResourceId) -> [u32; 4] {
    let bytes: Vec<u8> = device.map(readback).unwrap()[..RESULT_BYTES as usize]
        .iter()
        .map(std::cell::Cell::get)
        .collect();
    std::array::from_fn(|i| u32::from_le_bytes(bytes[i * 4..][..4].try_into().unwrap()))
}

/// How a plain format's stored components read.
#[derive(Clone, Copy)]
enum Read {
    Unorm,
    Snorm,
    Uint,
    Sint,
    Float,
}

/// A first texel of varied components for DXGI format `format`, the four words a load of it
/// gives by Direct3D's conversions (components the format lacks read 0, alpha 1), and how many
/// bits each word may be off (normalised conversions round); for the formats of whole-byte
/// components alike, none otherwise.
fn varied_texel(format: u32) -> Option<(Vec<u8>, [u32; 4], u32)> {
    use Read::*;
    use dxgi::*;
    // Each stored component's read component, in the order they are stored; none: not read.
    const RGBA: &[Option<usize>] = &[Some(0), Some(1), Some(2), Some(3)];
    const RGB: &[Option<usize>] = &[Some(0), Some(1), Some(2)];
    const RG: &[Option<usize>] = &[Some(0), Some(1)];
    const R: &[Option<usize>] = &[Some(0)];
    const A: &[Option<usize>] = &[Some(3)];
    const BGRA: &[Option<usize>] = &[Some(2), Some(1), Some(0), Some(3)];
    const BGRX: &[Option<usize>] = &[Some(2), Some(1), Some(0), None];
    let (bytes, read, order) = match format {
        R32G32B32A32_FLOAT => (4, Float, RGBA),
        R32G32B32A32_UINT => (4, Uint, RGBA),
        R32G32B32A32_SINT => (4, Sint, RGBA),
        R32G32B32_FLOAT => (4, Float, RGB),
        R32G32B32_UINT => (4, Uint, RGB),
        R32G32B32_SINT => (4, Sint, RGB),
        R16G16B16A16_FLOAT => (2, Float, RGBA),
        R16G16B16A16_UNORM => (2, Unorm, RGBA),
        R16G16B16A16_UINT => (2, Uint, RGBA),
        R16G16B16A16_SNORM => (2, Snorm, RGBA),
        R16G16B16A16_SINT => (2, Sint, RGBA),
        R32G32_FLOAT => (4, Float, RG),
        R32G32_UINT => (4, Uint, RG),
        R32G32_SINT => (4, Sint, RG),
        R8G8B8A8_UNORM => (1, Unorm, RGBA),
        R8G8B8A8_UINT => (1, Uint, RGBA),
        R8G8B8A8_SNORM => (1, Snorm, RGBA),
        R8G8B8A8_SINT => (1, Sint, RGBA),
        R16G16_FLOAT => (2, Float, RG),
        R16G16_UNORM => (2, Unorm, RG),
        R16G16_UINT => (2, Uint, RG),
        R16G16_SNORM => (2, Snorm, RG),
        R16G16_SINT => (2, Sint, RG),
        R32_FLOAT => (4, Float, R),
        R32_UINT => (4, Uint, R),
        R32_SINT => (4, Sint, R),
        R8G8_UNORM => (1, Unorm, RG),
        R8G8_UINT => (1, Uint, RG),
        R8G8_SNORM => (1, Snorm, RG),
        R8G8_SINT => (1, Sint, RG),
        R16_FLOAT => (2, Float, R),
        R16_UNORM => (2, Unorm, R),
        R16_UINT => (2, Uint, R),
        R16_SNORM => (2, Snorm, R),
        R16_SINT => (2, Sint, R),
        R8_UNORM => (1, Unorm, R),
        R8_UINT => (1, Uint, R),
        R8_SNORM => (1, Snorm, R),
        R8_SINT => (1, Sint, R),
        A8_UNORM => (1, Unorm, A),
        B8G8R8A8_UNORM => (1, Unorm, BGRA),
        B8G8R8X8_UNORM => (1, Unorm, BGRX),
        _ => return None,
    };
    // Stored values of either sign, the floats' as their bits.
    let raw = |i: usize| -> u32 {
        match (bytes, read) {
            (1, _) => [0x40, 0xc1, 0x62, 0x93][i],
            (2, _) => [0x3c00, 0xc123, 0x4246, 0x3369][i],
            (_, Float) => [1.5f32, -2.25, 3.0, 0.125][i].to_bits(),
            _ => [0x1234_5678, 0xf000_0001, 7, 0x7fff_ffff][i],
        }
    };
    let bits = 8 * bytes;
    let word = |v: u32| -> u32 {
        let high = (1u64 << bits) - 1;
        let signed = || (i64::from(v) << (64 - bits)) >> (64 - bits);
        match read {
            Unorm => (f64::from(v) / high as f64) as f32,
            Snorm => (signed() as f64 / (high >> 1) as f64).max(-1.0) as f32,
            Uint => return v,
            Sint => return signed() as i32 as u32,
            Float if bytes == 2 => half::f16::from_bits(v as u16).to_f32(),
            Float => return v,
        }
        .to_bits()
    };
    let one = match read {
        Uint | Sint => 1,
        _ => 1.0f32.to_bits(),
    };
    let mut expected = [0, 0, 0, one];
    let mut stored = Vec::new();
    for (i, to) in order.iter().enumerate() {
        let v = raw(i);
        stored.extend_from_slice(&v.to_le_bytes()[..bytes as usize]);
        if let Some(to) = to {
            expected[*to] = word(v);
        }
    }
    let tolerance = match read {
        Unorm | Snorm => 1,
        _ => 0,
    };
    Some((stored, expected, tolerance))
}

/// Whether the first three components a program read of `kind` are zero.
fn reads_zero(kind: Kind, words: [u32; 4]) -> bool {
    words[..3].iter().all(|&w| match kind {
        Kind::Float => f32::from_bits(w) == 0.0,
        Kind::Uint | Kind::Sint => w == 0,
    })
}

/// A single-sampled 2D texture of `format` with `flags`, and the commands copying zeros into its
/// first subresource from `zeros`, an upload buffer of zeros as large as any footprint needs.
fn zeroed(
    device: &mut Device<WebGpu>,
    format: u32,
    flags: u32,
    zeros: ResourceId,
    what: &str,
) -> (ResourceId, Command, FootprintInfo) {
    let desc = texture(format, DIMENSION_TEXTURE2D, 1, 1, 1, flags);
    let id = make(device, &desc, what);
    let (f, _) = desc.footprints(0, 1, 0).unwrap();
    let copy = Command::CopyTexture {
        dst: CopyLocation::Subresource {
            resource: id,
            index: 0,
        },
        at: [0; 3],
        src: CopyLocation::Footprint {
            resource: zeros,
            footprint: f[0].footprint,
        },
        area: None,
    };
    (id, copy, f[0])
}

/// The bytes of the first texel of subresource 0 of `id`, laid out as `f`.
fn first_texel(
    device: &mut Device<WebGpu>,
    id: ResourceId,
    f: FootprintInfo,
    what: &str,
) -> Vec<u8> {
    let total = u64::from(f.footprint.row_pitch) * u64::from(f.rows);
    let readback = device
        .create_committed(HeapType::Readback, &buffer(total, 0))
        .unwrap();
    run(
        device,
        vec![Command::CopyTexture {
            dst: CopyLocation::Footprint {
                resource: readback,
                footprint: f.footprint,
            },
            at: [0; 3],
            src: CopyLocation::Subresource {
                resource: id,
                index: 0,
            },
            area: None,
        }],
        what,
    );
    let texel = (f.row_size / u64::from(f.footprint.width)) as usize;
    let bytes = device.map(readback).unwrap()[..texel]
        .iter()
        .map(std::cell::Cell::get)
        .collect();
    device.destroy_resource(readback);
    bytes
}

/// Uses every ability answered for DXGI format `format` that needs a program; how many uses were
/// made. Each program reads zeros, so what it read is checked to be zero; stores and draws are
/// checked to have written the first texel, atomics to have returned and left what Direct3D's
/// operations give.
fn program_uses_of(format: u32) -> usize {
    let mut device = Device::new(webgpu());
    let caps = device.backend().caps().clone();
    let s = format_support(&caps, format);
    let has = |bit: u32| s.support1 & bit != 0;
    let has2 = |bit: u32| s.support2 & bit != 0;
    let (kind, view) = (kind_of(format), view_format(format));
    let k = kind as usize;
    let programs = Programs::new(&mut device);
    // Zeros enough for every footprint and buffer made here.
    let zero_bytes = [format, dxgi::R8G8B8A8_UNORM]
        .map(|f| {
            let desc = texture(f, DIMENSION_TEXTURE2D, 1, 1, 1, 0);
            desc.footprints(0, 1, 0).map_or(0, |(_, total)| total)
        })
        .into_iter()
        .chain([u64::from(3 * ELEMENT_BYTES)])
        .max()
        .unwrap_or_default();
    let zeros = uploaded(&mut device, &vec![0; zero_bytes as usize]);
    let mut used = 0;
    let srv = |resource, dimension| {
        Descriptor::ShaderResource(ResourceView {
            resource,
            format: view,
            dimension,
            mapping: DEFAULT_MAPPING,
            first_element: 0,
            count: 1,
            stride: 0,
            mip: 0,
            first_layer: 0,
            layers: 1,
            flags: 0,
            min_lod: 0,
        })
    };
    let uav = |resource, dimension| {
        Descriptor::UnorderedAccess(ResourceView {
            mapping: 0,
            ..match srv(resource, dimension) {
                Descriptor::ShaderResource(v) => v,
                _ => unreachable!(),
            }
        })
    };

    // Reads of a texture of zeros.
    let reads: [(u32, &[u8], &str); 5] = [
        (support1::SHADER_LOAD, programs::LOAD[k], "a load"),
        (support1::SHADER_SAMPLE, programs::SAMPLE, "a sample"),
        (
            support1::SHADER_SAMPLE_COMPARISON,
            programs::COMPARE,
            "a comparison sample",
        ),
        (support1::SHADER_GATHER, programs::GATHER, "a gather"),
        (
            support1::SHADER_GATHER_COMPARISON,
            programs::GATHER_COMPARE,
            "a comparison gather",
        ),
    ];
    if reads.iter().any(|(bit, ..)| has(*bit)) {
        let what = format!("format {format}: a texture read by programs");
        let (t, zero, _) = zeroed(&mut device, format, 0, zeros, &what);
        for (bit, cs, name) in reads {
            if !has(bit) {
                continue;
            }
            let what = format!("format {format}: {name}");
            let views = [srv(t, SRV_DIMENSION_TEXTURE2D), Descriptor::Empty];
            let words = programs.dispatch(&mut device, cs, views, vec![zero.clone()], &what);
            let compared = bit == support1::SHADER_SAMPLE_COMPARISON
                || bit == support1::SHADER_GATHER_COMPARISON;
            // A comparison of a texel of zero with 0.5 by `LESS_EQUAL` fails: zero.
            let read_as = if compared { Kind::Float } else { kind };
            let zero_words = match bit {
                support1::SHADER_SAMPLE_COMPARISON => words[0] == 0,
                support1::SHADER_GATHER | support1::SHADER_GATHER_COMPARISON => {
                    words.iter().all(|&w| f32::from_bits(w) == 0.0)
                }
                _ => reads_zero(read_as, words),
            };
            assert!(zero_words, "{what}: read {words:x?} of zeros");
            used += 1;
        }
        device.destroy_resource(t);
    }

    // A load of a texel of varied components, of the formats `varied_texel` decodes.
    if has(support1::SHADER_LOAD)
        && format == view
        && let Some((bytes, expected, tolerance)) = varied_texel(format)
    {
        let what = format!("format {format}: a load of a varied texel");
        let mut fill = vec![0; zero_bytes as usize];
        fill[..bytes.len()].copy_from_slice(&bytes);
        let varied = uploaded(&mut device, &fill);
        let (t, copy, _) = zeroed(&mut device, format, 0, varied, &what);
        let views = [srv(t, SRV_DIMENSION_TEXTURE2D), Descriptor::Empty];
        let words = programs.dispatch(&mut device, programs::LOAD[k], views, vec![copy], &what);
        let near = words
            .iter()
            .zip(expected)
            .all(|(&w, e)| w.abs_diff(e) <= tolerance);
        assert!(
            near,
            "{what}: read {words:x?}, Direct3D gives {expected:x?}"
        );
        device.destroy_resource(t);
        device.destroy_resource(varied);
        used += 1;
    }

    // A load of each sample of a multisampled texture cleared to zeros.
    if has(support1::MULTISAMPLE_LOAD) {
        let depth = has(support1::DEPTH_STENCIL);
        for samples in 2..=MOST_SAMPLES {
            if quality_levels(&caps, format, samples).unwrap_or(0) == 0 {
                continue;
            }
            let what = format!("format {format}: a multisampled load of {samples} samples");
            let flag = if depth {
                FLAG_DEPTH_STENCIL
            } else {
                FLAG_RENDER_TARGET
            };
            let ms = make(
                &mut device,
                &texture(format, DIMENSION_TEXTURE2D, 1, 1, samples, flag),
                &what,
            );
            let clear = match depth {
                true => Command::ClearDepth {
                    view: Descriptor::DepthStencil {
                        view: target(ms, format),
                        read_only: 0,
                    },
                    depth: Some(0.0),
                    stencil: Some(0),
                    rects: vec![],
                },
                false => Command::ClearTarget {
                    view: Descriptor::RenderTarget(target(ms, format)),
                    color: [0.0; 4],
                    rects: vec![],
                },
            };
            let views = [srv(ms, SRV_DIMENSION_TEXTURE2DMS), Descriptor::Empty];
            let words = programs.dispatch(
                &mut device,
                programs::MULTISAMPLE_LOAD[k],
                views,
                vec![clear],
                &what,
            );
            assert!(reads_zero(kind, words), "{what}: read {words:x?} of zeros");
            device.destroy_resource(ms);
            used += 1;
        }
    }

    // Typed buffer views: a load of zeros, and a store where storage views are answered.
    if has(support1::BUFFER) {
        let what = format!("format {format}: a typed buffer view");
        let b = make(
            &mut device,
            &buffer(u64::from(ELEMENT_BYTES), FLAG_UNORDERED_ACCESS),
            &what,
        );
        let view = |d: Descriptor| match d {
            Descriptor::ShaderResource(v) => Descriptor::ShaderResource(ResourceView {
                dimension: SRV_DIMENSION_BUFFER,
                ..v
            }),
            Descriptor::UnorderedAccess(v) => Descriptor::UnorderedAccess(ResourceView {
                dimension: UAV_DIMENSION_BUFFER,
                ..v
            }),
            d => d,
        };
        let zero = Command::CopyBuffer {
            dst: b,
            dst_offset: 0,
            src: zeros,
            src_offset: 0,
            size: u64::from(ELEMENT_BYTES),
        };
        let views = [view(srv(b, SRV_DIMENSION_BUFFER)), Descriptor::Empty];
        let words = programs.dispatch(
            &mut device,
            programs::BUFFER_LOAD[k],
            views,
            vec![zero.clone()],
            &what,
        );
        assert!(reads_zero(kind, words), "{what}: read {words:x?} of zeros");
        used += 1;
        if has(support1::TYPED_UNORDERED_ACCESS_VIEW) && has2(support2::UAV_TYPED_STORE) {
            let what = format!("format {format}: a typed buffer store");
            let views = [Descriptor::Empty, view(uav(b, UAV_DIMENSION_BUFFER))];
            programs.dispatch(
                &mut device,
                programs::BUFFER_STORE[k],
                views,
                vec![zero],
                &what,
            );
            let readback = device
                .create_committed(HeapType::Readback, &buffer(u64::from(ELEMENT_BYTES), 0))
                .unwrap();
            run(
                &mut device,
                vec![Command::CopyBuffer {
                    dst: readback,
                    dst_offset: 0,
                    src: b,
                    src_offset: 0,
                    size: u64::from(ELEMENT_BYTES),
                }],
                &what,
            );
            let words = read_words(&mut device, readback);
            assert_ne!(words, [0; 4], "{what}: nothing written");
            device.destroy_resource(readback);
            used += 1;
        }
        device.destroy_resource(b);
    }

    // Storage textures: a store, a typed load of zeros, and every answered atomic.
    if has(support1::TYPED_UNORDERED_ACCESS_VIEW) {
        let what = format!("format {format}: a storage texture used by programs");
        let (t, zero, f) = zeroed(&mut device, format, FLAG_UNORDERED_ACCESS, zeros, &what);
        let views = [Descriptor::Empty, uav(t, UAV_DIMENSION_TEXTURE2D)];
        if has2(support2::UAV_TYPED_STORE) {
            let what = format!("format {format}: a typed store");
            programs.dispatch(
                &mut device,
                programs::STORE[k],
                views,
                vec![zero.clone()],
                &what,
            );
            let texel = first_texel(&mut device, t, f, &what);
            assert!(texel.iter().any(|&b| b != 0), "{what}: nothing written");
            used += 1;
        }
        if has2(support2::UAV_TYPED_LOAD) {
            let what = format!("format {format}: a typed storage load");
            let words = programs.dispatch(
                &mut device,
                programs::TYPED_LOAD[k],
                views,
                vec![zero.clone()],
                &what,
            );
            assert!(
                reads_zero(kind, [words[0], 0, 0, 0]),
                "{what}: read {words:x?} of zeros"
            );
            used += 1;
        }
        for (bit, by_kind, expected) in programs::ATOMICS {
            if !has2(bit) {
                continue;
            }
            let what = format!("format {format}: atomics {bit:#x}");
            let cs = by_kind[k].unwrap_or_else(|| panic!("{what}: no such atomic of its kind"));
            let words = programs.dispatch(&mut device, cs, views, vec![zero.clone()], &what);
            // The float exchange swaps in the float 4.
            let expected = match kind {
                Kind::Float => expected.map(|w| match w {
                    0 => 0,
                    w => (w as f32).to_bits(),
                }),
                _ => expected,
            };
            assert_eq!(words, expected, "{what}");
            used += 1;
        }
        device.destroy_resource(t);
    }

    // Vertex input of this format into a target of `R8G8B8A8_UNORM`, and index input.
    let rgba8 = dxgi::R8G8B8A8_UNORM;
    let mut drawn_into = |device: &mut Device<WebGpu>,
                          pso: PipelineId,
                          input: u32,
                          indices: Option<u32>,
                          what: &str| {
        let (t, _, f) = zeroed(device, rgba8, FLAG_RENDER_TARGET, zeros, what);
        let vertices = device.address(zeros);
        let index_address = indices.map(|format| {
            let index_bytes: Vec<u8> = match format {
                dxgi::R16_UINT => [0u16, 1, 2].iter().flat_map(|i| i.to_le_bytes()).collect(),
                _ => [0u32, 1, 2].iter().flat_map(|i| i.to_le_bytes()).collect(),
            };
            let b = uploaded(device, &index_bytes);
            (b, (device.address(b), format))
        });
        let mut commands = vec![Command::ClearTarget {
            view: Descriptor::RenderTarget(target(t, rgba8)),
            color: [0.0; 4],
            rects: vec![],
        }];
        commands.extend(draw(
            pso,
            &programs.graphics,
            vec![Descriptor::RenderTarget(target(t, rgba8))],
            None,
            vertices,
            index_address.map(|(_, i)| i),
        ));
        run(device, commands, what);
        let texel = first_texel(device, t, f, what);
        assert_eq!(texel, [u8::MAX; 4], "{what}: not drawn, {input}");
        if let Some((b, _)) = index_address {
            device.destroy_resource(b);
        }
        device.destroy_resource(t);
        used += 1;
    };
    if has(support1::IA_VERTEX_BUFFER) {
        let what = format!("format {format}: vertex input");
        let pso = programs.pipeline(
            &mut device,
            (programs::VERTEX[k], format),
            programs::PIXEL[Kind::Float as usize],
            Outputs::colour(vec![rgba8], false, 1),
            &what,
        );
        drawn_into(&mut device, pso, format, None, &what);
        device.destroy_pipeline(pso);
    }
    let float4 = dxgi::R32G32B32A32_FLOAT;
    if has(support1::IA_INDEX_BUFFER) {
        let what = format!("format {format}: index input");
        let pso = programs.pipeline(
            &mut device,
            (programs::VERTEX[Kind::Float as usize], float4),
            programs::PIXEL[Kind::Float as usize],
            Outputs::colour(vec![rgba8], false, 1),
            &what,
        );
        drawn_into(&mut device, pso, float4, Some(format), &what);
        device.destroy_pipeline(pso);
    }

    // Draws into a target of this format, blended where answered, single-sampled and of every
    // answered sample count; and into a depth target.
    let vs = (programs::VERTEX[Kind::Float as usize], float4);
    let vertices = device.address(zeros);
    if has(support1::RENDER_TARGET) {
        let blended = has(support1::BLENDABLE);
        for samples in 1..=MOST_SAMPLES {
            if samples > 1 && quality_levels(&caps, format, samples).unwrap_or(0) == 0 {
                continue;
            }
            let what =
                format!("format {format}: drawn into with {samples} samples, blended {blended}");
            let (t, zero, f) = match samples {
                1 => {
                    let (t, zero, f) =
                        zeroed(&mut device, format, FLAG_RENDER_TARGET, zeros, &what);
                    (t, Some(zero), Some(f))
                }
                _ => {
                    let desc = texture(
                        format,
                        DIMENSION_TEXTURE2D,
                        1,
                        1,
                        samples,
                        FLAG_RENDER_TARGET,
                    );
                    (make(&mut device, &desc, &what), None, None)
                }
            };
            let pso = programs.pipeline(
                &mut device,
                vs,
                programs::PIXEL[k],
                Outputs::colour(vec![view], blended, samples),
                &what,
            );
            let rtv = Descriptor::RenderTarget(target(t, format));
            let mut commands: Vec<Command> = zero.into_iter().collect();
            commands.push(Command::ClearTarget {
                view: rtv,
                color: [0.0; 4],
                rects: vec![],
            });
            commands.extend(draw(
                pso,
                &programs.graphics,
                vec![rtv],
                None,
                vertices,
                None,
            ));
            run(&mut device, commands, &what);
            if let Some(f) = f {
                let texel = first_texel(&mut device, t, f, &what);
                assert!(texel.iter().any(|&b| b != 0), "{what}: nothing drawn");
            }
            device.destroy_pipeline(pso);
            device.destroy_resource(t);
            used += 1;
        }
    }
    if has(support1::DEPTH_STENCIL) {
        let what = format!("format {format}: a depth target drawn into");
        let desc = texture(format, DIMENSION_TEXTURE2D, 1, 1, 1, FLAG_DEPTH_STENCIL);
        let t = make(&mut device, &desc, &what);
        let outputs = Outputs {
            targets: vec![],
            blended: false,
            depth: format,
            samples: 1,
        };
        let pso = programs.pipeline(&mut device, vs, &[], outputs, &what);
        let dsv = Descriptor::DepthStencil {
            view: target(t, format),
            read_only: 0,
        };
        let mut commands = vec![Command::ClearDepth {
            view: dsv,
            depth: Some(1.0),
            stencil: Some(0),
            rects: vec![],
        }];
        commands.extend(draw(
            pso,
            &programs.graphics,
            vec![],
            Some(dsv),
            vertices,
            None,
        ));
        run(&mut device, commands, &what);
        device.destroy_pipeline(pso);
        device.destroy_resource(t);
        used += 1;
    }
    device.destroy_resource(zeros);
    device.destroy_descriptor_heap(programs.heap);
    used
}
