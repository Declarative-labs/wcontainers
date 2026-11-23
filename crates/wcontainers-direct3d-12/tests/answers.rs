//! Every format ability `CheckFeatureSupport` answers present is used through the device on a real
//! GPU, which must serve it: textures of each shape, mip chains, targets and depth targets cleared,
//! multisampled ones of every count answered, resolves, storage textures, copies between the
//! formats of a group, and textures of the formats answered absent are refused. Needs a GPU: run
//! it through `tools/graphics-rig/gpu-job`. `WEBGPU_API` picks wgpu's API (dx12, vulkan, metal).

use std::sync::Arc;

use wcontainers_direct3d_12::features::{format_support, quality_levels};
use wcontainers_direct3d_12::list::{Command, CommandList, CopyLocation, Descriptor, TargetView};
use wcontainers_direct3d_12::resource::{
    DIMENSION_TEXTURE1D, DIMENSION_TEXTURE2D, DIMENSION_TEXTURE3D, FLAG_DEPTH_STENCIL,
    FLAG_RENDER_TARGET, FLAG_UNORDERED_ACCESS, HeapType, ResourceDesc,
};
use wcontainers_direct3d_12::{Device, ResourceId};
use wcontainers_directx_graphics_infrastructure::required::MOST_SAMPLES;
use wcontainers_directx_graphics_infrastructure::{dxgi, support1, typeless_group};
use wcontainers_gpu::{Backend, CUBE_FACES, DeviceStatus};
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

fn make(device: &mut Device<WebGpu>, desc: &ResourceDesc, what: &str) -> ResourceId {
    device
        .create_committed(HeapType::Default, desc)
        .unwrap_or_else(|e| panic!("{what}: answered, refused: {e:?}"))
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
    let mut used = 0;
    let mut failures = Vec::new();
    for format in FORMATS {
        match std::panic::catch_unwind(|| uses_of(format)) {
            Ok(n) => used += n,
            Err(e) => failures.push(
                e.downcast_ref::<String>()
                    .cloned()
                    .unwrap_or_else(|| format!("format {format}")),
            ),
        }
    }
    eprintln!("{used} answered abilities used");
    for f in &failures {
        eprintln!("not served: {f}");
    }
    let unknown: Vec<_> = failures
        .iter()
        .filter(|f| !KNOWN_FAILURES.iter().any(|(k, _)| f.contains(k)))
        .collect();
    assert!(unknown.is_empty(), "not served as answered: {unknown:?}");
    for (known, reason) in KNOWN_FAILURES {
        assert!(
            failures.iter().any(|f| f.contains(known)),
            "served now, so no longer a known failure: {known} ({reason})"
        );
    }
}

/// Uses that fail, each with its reason (the status file's gap list names them).
const KNOWN_FAILURES: [(&str, &str); 1] = [(
    "format 43: copied into 40",
    "a copy of an R32_SINT texture into a D32_FLOAT one loses the device in the back end's \
     depth copy by draw (`DepthReadByDraw`); not traced further",
)];

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
