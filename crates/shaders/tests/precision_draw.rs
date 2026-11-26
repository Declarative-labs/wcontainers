//! Values Direct3D 12 defines bit for bit, run by real Direct3D 12 on the GPU and on WARP and by
//! the WebGPU back end on wgpu's Direct3D 12 and Vulkan paths, fxc's build and dxc's: minimum-precision
//! constants (`precision.hlsl`'s `cs_min16`, the constant buffers seeded words) and `frexp` of any
//! bits (`frexp.hlsl`'s `cs_frexp`). The words written are compared (a NaN as any NaN). Seeds:
//! `WCONTAINERS_PRECISION_SEED` (default 1) and `WCONTAINERS_PRECISION_CASES` seeds from it (default
//! 4).

#![cfg(windows)]

mod common;

use common::*;
use wcontainers_gpu::{
    Gpu, draw::DispatchCall, draw::DispatchGroups, reflect::SlotKind, reflect::VariantKey,
    resource::BindingEntry, resource::BindingResource, resource::BufferRange,
    resource::BufferUsage, resource::ReadSource, shader::Program, shader::StageSet,
};
use wcontainers_graphics_test::{Driver, d3d12};
use wcontainers_shaders::{Source, convert};
use windows::Win32::Graphics::Direct3D12::*;

/// Each constant buffer's bytes (b0 and b1), one placement step of Direct3D 12's constant buffers,
/// at least what any program declares.
const CONSTANT_BYTES: usize = D3D12_CONSTANT_BUFFER_DATA_PLACEMENT_ALIGNMENT as usize;

/// The two constant buffers' bytes.
type Constants = [Vec<u8>; 2];

/// One program: its name, the words it writes, its workgroups, its constants at a seed, and which
/// words hold a float's bits.
struct Shader {
    name: &'static str,
    words: usize,
    groups: u32,
    constants: fn(u64) -> Constants,
    float: fn(usize) -> bool,
    /// The known difference a word of a build may show, by name, if any.
    known: fn(&Constants, &str, usize) -> Option<&'static str>,
}

/// Direct3D flushes 32-bit subnormals in programs, WGSL leaves it to the implementation (plan
/// 4.12, a listed loss).
const SUBNORMAL: &str = "subnormal operand (plan 4.12)";
/// fxc's `min16i as def32` conversion: the GPU and WARP keep the 16 bits below and sign-extend
/// them, the DXBC route keeps 32 (gap A34).
const MIN16_SIGNED_CONVERSION: &str = "min16i as def32 (A34)";
/// fxc picks a vector's component at a run-time index by a dot product with a row of an identity
/// matrix; on both devices a NaN or infinity elsewhere in the vector makes the result a NaN, and a
/// NaN picked comes back as the device's own NaN, where dxbc-spirv folds the pick into the
/// component alone (gap A35).
const PICKED_BY_DOT: &str =
    "a NaN, or a component beside a NaN or infinity, picked by a dot product (A35)";

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks(size_of::<u32>())
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
        .collect()
}

/// Real Direct3D 12 running `code` with the constant buffers at root descriptors and the output
/// at a root unordered access view; the words written.
fn direct3d12(gpu: &d3d12::Gpu, shader: &Shader, code: &[u8], constants: &Constants) -> Vec<u32> {
    let bytes = (shader.words * size_of::<u32>()) as u64;
    let descriptor = |kind| D3D12_ROOT_PARAMETER {
        ParameterType: kind,
        Anonymous: D3D12_ROOT_PARAMETER_0 {
            Descriptor: D3D12_ROOT_DESCRIPTOR::default(),
        },
        ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
    };
    let mut parameters = [
        descriptor(D3D12_ROOT_PARAMETER_TYPE_CBV),
        descriptor(D3D12_ROOT_PARAMETER_TYPE_CBV),
        descriptor(D3D12_ROOT_PARAMETER_TYPE_UAV),
    ];
    parameters[1].Anonymous.Descriptor.ShaderRegister = 1;
    let root = gpu
        .root_signature(&D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
            Version: D3D_ROOT_SIGNATURE_VERSION_1_0,
            Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
                Desc_1_0: D3D12_ROOT_SIGNATURE_DESC {
                    NumParameters: parameters.len() as u32,
                    pParameters: parameters.as_ptr(),
                    ..Default::default()
                },
            },
        })
        .unwrap();
    let desc = D3D12_COMPUTE_PIPELINE_STATE_DESC {
        pRootSignature: std::mem::ManuallyDrop::new(Some(root.clone())),
        CS: d3d12::bytecode(code),
        ..Default::default()
    };
    let pipeline: ID3D12PipelineState =
        unsafe { gpu.device.CreateComputePipelineState(&desc) }.unwrap();
    drop(std::mem::ManuallyDrop::into_inner(desc.pRootSignature));
    let buffers = constants.each_ref().map(|c| gpu.upload(c).unwrap());
    let output = gpu
        .committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &D3D12_RESOURCE_DESC {
                Flags: D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
                ..d3d12::buffer_desc(bytes)
            },
            D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
            None,
        )
        .unwrap();
    let list = &gpu.list;
    unsafe {
        list.SetComputeRootSignature(&root);
        list.SetPipelineState(&pipeline);
        for (i, b) in buffers.iter().enumerate() {
            list.SetComputeRootConstantBufferView(i as u32, b.GetGPUVirtualAddress());
        }
        list.SetComputeRootUnorderedAccessView(2, output.GetGPUVirtualAddress());
        list.Dispatch(shader.groups, 1, 1);
    }
    for b in &buffers {
        gpu.keep(b);
    }
    let read = gpu
        .read_buffer(&output, bytes, D3D12_RESOURCE_STATE_UNORDERED_ACCESS)
        .unwrap();
    words(&read)
}

/// The same through `b`, the build converted with the default key.
fn ours<B: Gpu>(b: &mut B, shader: &Shader, source: Source, constants: &Constants) -> Vec<u32> {
    let bytes = (shader.words * size_of::<u32>()) as u64;
    let c = convert(source, &VariantKey::default(), None).unwrap();
    assert!(
        c.reflection.helpers.is_empty(),
        "{:?}",
        c.reflection.helpers
    );
    let uniforms = constants
        .each_ref()
        .map(|c| buffer(b, c, BufferUsage::UNIFORM));
    let output = buffer(
        b,
        &vec![0; bytes as usize],
        BufferUsage::STORAGE | BufferUsage::COPY_SRC,
    );
    let entries: Vec<_> = c
        .reflection
        .bindings
        .iter()
        .map(|r| BindingEntry {
            binding: r.binding,
            stages: StageSet::COMPUTE,
            resource: match r.kind {
                SlotKind::Uniform => BindingResource::UniformBuffer(BufferRange {
                    buffer: &uniforms[r.register as usize],
                    offset: 0,
                    size: CONSTANT_BYTES as u64,
                }),
                SlotKind::Storage => BindingResource::StorageBuffer {
                    range: BufferRange {
                        buffer: &output,
                        offset: 0,
                        size: bytes,
                    },
                    writable: true,
                },
                other => panic!("{other:?}"),
            },
        })
        .collect();
    let set = b.create_binding_set(&entries).unwrap();
    let module = b.create_shader(&c.modules[0]).unwrap();
    let program = b.create_program(&Program::Compute(&module)).unwrap();
    b.dispatch(&DispatchCall {
        program: &program,
        bindings: [Some(&set), None, None, None],
        immediates: &[],
        groups: DispatchGroups::Direct([shader.groups, 1, 1]),
    })
    .unwrap();
    let mut out = vec![0; bytes as usize];
    b.read_back_now(
        &ReadSource::Buffer {
            buffer: &output,
            offset: 0,
            size: bytes,
        },
        &mut out,
    )
    .unwrap();
    assert_eq!(b.status(), wcontainers_gpu::error::DeviceStatus::Ok);
    words(&out)
}

/// The words of `a` and `b` that differ, a NaN equal to any NaN in a float's word.
fn apart(shader: &Shader, a: &[u32], b: &[u32]) -> Vec<usize> {
    let nan = |w: u32| f32::from_bits(w).is_nan();
    (0..shader.words)
        .filter(|&i| a[i] != b[i] && !((shader.float)(i) && nan(a[i]) && nan(b[i])))
        .collect()
}

/// Every word each build writes equals the GPU's under real Direct3D 12 but where a known
/// difference names it (counted by name and printed); where WARP differs from the GPU it is printed.
fn alike(shader: &Shader) {
    let seed: u64 = std::env::var("WCONTAINERS_PRECISION_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_PRECISION_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let mut paths = [
        ("wgpu on direct3d 12", webgpu()),
        ("wgpu on vulkan", webgpu_on(wgpu::Backends::VULKAN)),
    ];
    let gpu = d3d12::create(Driver::Hardware, false).unwrap();
    let warp = d3d12::create(Driver::Warp, false).unwrap();
    let mut known = std::collections::BTreeMap::<(&str, &str), usize>::new();
    let mut failures = Vec::new();
    let mut compared = 0;
    for seed in seed..seed + seeds {
        let constants = (shader.constants)(seed);
        for build in ["dxbc", "dxil"] {
            let code = bytecode(&format!("{}.{build}", shader.name));
            let reference = direct3d12(&gpu, shader, &code, &constants);
            let on_warp = direct3d12(&warp, shader, &code, &constants);
            eprintln!(
                "seed {seed} {build}: gpu {:08x?}, warp apart at {:?}",
                &reference[..4],
                apart(shader, &reference, &on_warp)
            );
            for (path, b) in &mut paths {
                let source = match build {
                    "dxil" => Source::Dxil(&code),
                    _ => Source::Dxbc(&code),
                };
                let words = ours(b, shader, source, &constants);
                let mut differ = apart(shader, &reference, &words);
                differ.retain(|&i| match (shader.known)(&constants, build, i) {
                    Some(name) => {
                        *known.entry((build, name)).or_default() += 1;
                        false
                    }
                    None => true,
                });
                compared += 1;
                eprintln!("seed {seed} {build} {path}: apart at {differ:?}");
                if !differ.is_empty() {
                    failures.push(format!(
                        "seed {seed} {build} {path}: {:x?}",
                        differ
                            .iter()
                            .map(|&i| (i, reference[i], words[i]))
                            .collect::<Vec<_>>()
                    ));
                }
            }
        }
    }
    eprintln!(
        "{}: {compared} comparisons, {} failed; known words apart: {known:?}",
        shader.name,
        failures.len()
    );
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Seeded bytes for both constant buffers.
fn seeded_constants(seed: u64) -> Constants {
    let mut rng = fastrand::Rng::with_seed(seed);
    [(); 2].map(|_| (0..CONSTANT_BYTES).map(|_| rng.u8(..)).collect())
}

/// `cs_min16`'s words holding a float's bits; the others hold integers.
const MIN16_FLOATS: [usize; 15] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 22, 24, 26, 28, 30];
/// `cs_min16`'s words of a `min16int` made an `int`.
const MIN16_SIGNED: [usize; 10] = [14, 15, 16, 18, 19, 23, 25, 27, 29, 31];

const MIN16: Shader = Shader {
    name: "cs_min16",
    words: 32,
    groups: 1,
    constants: seeded_constants,
    float: |i| MIN16_FLOATS.contains(&i),
    known: |_, build, i| {
        (build == "dxbc" && MIN16_SIGNED.contains(&i)).then_some(MIN16_SIGNED_CONVERSION)
    },
};

/// Minimum-precision constants: 32-bit in fxc's build, 16-bit in dxc's.
#[test]
fn minimum_precision_constants_read_as_direct3d_12_reads_them() {
    alike(&MIN16);
}

/// `cs_frexp`'s invocations, an operand each.
const FREXP_OPERANDS: usize = 64;
/// The operands of one constant buffer row (`float4 operands[16]`).
const ROW_OPERANDS: usize = size_of::<[f32; 4]>() / size_of::<f32>();

/// The operands: every special value first, each NaN and infinity in a row of finite values (so that
/// fxc's build, which picks an operand from its row by a dot product, gives the infinities alone and
/// the finite values beside them are compared; A35), then seeded special values and floats of any
/// bits.
fn frexp_constants(seed: u64) -> Constants {
    const SPECIAL: [u32; 17] = [
        f32::NAN.to_bits(),
        0x7fc9_0fdb, // NaN with a payload
        0xffc0_0001, // NaN, sign set
        0x7f80_0001, // signalling NaN
        f32::INFINITY.to_bits(),
        f32::NEG_INFINITY.to_bits(),
        0f32.to_bits(),
        (-0f32).to_bits(),
        1f32.to_bits(),
        (-1f32).to_bits(),
        0.5f32.to_bits(),
        1,           // the least subnormal
        0x0007_ffff, // a subnormal
        0x807f_ffff, // the largest subnormal, negative
        f32::MIN_POSITIVE.to_bits(),
        f32::MAX.to_bits(),
        7604f32.to_bits(),
    ];
    let mut rng = fastrand::Rng::with_seed(seed);
    let finite = |rng: &mut fastrand::Rng| loop {
        let w = rng.u32(..);
        if f32::from_bits(w).is_finite() {
            break w;
        }
    };
    let (odd, plain): (Vec<u32>, Vec<u32>) = SPECIAL
        .iter()
        .partition(|&&w| !f32::from_bits(w).is_finite());
    let mut plain = plain.into_iter();
    let mut operands = Vec::new();
    for w in odd {
        operands.push(w);
        for _ in 1..ROW_OPERANDS {
            operands.push(plain.next().unwrap_or_else(|| finite(&mut rng)));
        }
    }
    operands.extend(plain);
    while operands.len() < FREXP_OPERANDS {
        operands.push(match rng.u32(..3) {
            0 => SPECIAL[rng.usize(..SPECIAL.len())],
            _ => rng.u32(..),
        });
    }
    let mut bytes: Vec<u8> = operands.iter().flat_map(|w| w.to_le_bytes()).collect();
    bytes.resize(CONSTANT_BYTES, 0);
    [bytes, vec![0; CONSTANT_BYTES]]
}

const FREXP: Shader = Shader {
    name: "cs_frexp",
    words: 2 * FREXP_OPERANDS,
    groups: 1,
    constants: frexp_constants,
    float: |_| true,
    known: |constants, build, i| {
        let operand = |n: usize| {
            let at = n * size_of::<u32>();
            f32::from_bits(u32::from_le_bytes(
                constants[0][at..at + size_of::<u32>()].try_into().unwrap(),
            ))
        };
        let n = i / 2;
        let row = n / ROW_OPERANDS * ROW_OPERANDS..(n / ROW_OPERANDS + 1) * ROW_OPERANDS;
        // The dot product's other terms are a NaN where another operand of the row is not finite,
        // and the devices' dot product of a NaN gives their own NaN, not the operand's payload.
        let beside = row.filter(|&k| k != n).any(|k| !operand(k).is_finite());
        if build == "dxbc" && (beside || operand(n).is_nan()) {
            Some(PICKED_BY_DOT)
        } else {
            operand(n).is_subnormal().then_some(SUBNORMAL)
        }
    },
};

/// `frexp` of NaNs, infinities, zeros and any bits: both builds give Direct3D 12's parts (the
/// DXIL build's NaNs with a payload, the runner's frexp line 52, included), but where fxc's build
/// picks a NaN, or an operand beside a NaN or infinity of its row (A35).
#[test]
fn frexp_gives_direct3d_12s_parts() {
    alike(&FREXP);
}
