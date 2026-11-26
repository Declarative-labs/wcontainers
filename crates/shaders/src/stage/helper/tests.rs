//! The domain stage's compute form on a WebGPU device (lavapipe on the build machines, decision D6 of
//! the graphics brief): its dispatch covers the most points the patches can make, and the invocations
//! past the tessellator's point total write a record outside every clip volume instead of running
//! the domain program on points the tessellator never made.

use super::*;
use crate::{Source, convert};
use wcontainers_gpu::error::DeviceStatus;
use wcontainers_gpu::reflect::{SlotKind, VariantKey};
use wcontainers_gpu::resource::{BufferDesc, MapIntent, ReadSource, WriteMode};
use wcontainers_gpu::{Object, StandaloneGpu};
use wcontainers_gpu_webgpu::{WebGpu, WebGpuConfig};

/// Patches in the draw; the hull shaders' constant buffer holds 32.
const PATCHES: u32 = 8;
/// Control points per patch, as `vs_patch` makes them and the triangle hull shaders read them.
const PATCH_POINTS: u32 = 3;
/// Every factor of every patch: a few points each, far below what the hull's largest factor allows.
const FACTOR: f32 = 3.0;
/// Bytes of the hull shaders' constant buffer (`float4 factors[64]`).
const FACTOR_BYTES: usize = 64 * 16;

fn program(name: &str) -> Converted {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/own")
        .join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    convert(Source::Dxbc(&bytes), &VariantKey::default(), None)
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn words<B: Gpu>(b: &mut B, buffer: &B::Buffer, size: u64) -> Vec<u32> {
    let mut out = alloc::vec![0u8; usize::try_from(size).unwrap()];
    b.read_back_now(
        &ReadSource::Buffer {
            buffer,
            offset: 0,
            size,
        },
        &mut out,
    )
    .unwrap();
    out.chunks(4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
        .collect()
}

#[test]
fn domain_invocations_past_the_point_total_write_no_point() {
    let mut b = <WebGpu as StandaloneGpu>::new(WebGpuConfig {
        backends: wgpu::Backends::PRIMARY,
        lavapipe: true,
        software: false,
        required: wgpu::Features::empty(),
        without: wgpu::Features::empty(),
    })
    .expect("a WebGPU device (lavapipe where VK_DRIVER_FILES names it)");
    let vs = program("vs_patch.dxbc");
    let hull = program("hs_tri_integer_ccw.dxbc");
    let domain = program("ds_tri.dxbc");
    let entry = |semantic: &str, count| StreamOutputEntry {
        stream: 0,
        semantic: Some(semantic.into()),
        semantic_index: 0,
        start_component: 0,
        component_count: count,
        slot: 0,
    };
    let so = StreamOutput {
        entries: alloc::vec![entry("DOMAIN", 3), entry("PRIMITIVE", 1)],
        strides: [16, 0, 0, 0],
    };
    let mut stages =
        Stages::tessellated_with(&mut b, &vs, &hull, &domain, None, Some(&so)).unwrap();

    let mut constants = alloc::vec![0u8; FACTOR_BYTES];
    for word in constants.chunks_mut(4) {
        word.copy_from_slice(&FACTOR.to_le_bytes());
    }
    let constant_buffer = b
        .create_buffer(&BufferDesc {
            size: constants.len() as u64,
            usage: BufferUsage::UNIFORM | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .unwrap();
    b.write_buffer(&constant_buffer, 0, &constants, WriteMode::Ordered);
    let mut by_set: [Vec<BindingEntry<'_, WebGpu>>; MAX_BINDING_SETS] = Default::default();
    for r in &hull.reflection.bindings {
        assert_eq!((r.kind, r.register), (SlotKind::Uniform, 0), "{r:?}");
        by_set[r.set as usize].push(BindingEntry {
            binding: r.binding,
            stages: StageSet::COMPUTE,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &constant_buffer,
                offset: 0,
                size: constants.len() as u64,
            }),
        });
    }
    let hull_sets = by_set.map(|e| (!e.is_empty()).then(|| b.create_binding_set(&e).unwrap()));

    let streamed_bytes = 4096;
    let target = b
        .create_buffer(&BufferDesc {
            size: streamed_bytes,
            usage: BufferUsage::STORAGE | BufferUsage::COPY_SRC,
            intent: MapIntent::Rarely,
        })
        .unwrap();
    let filled = b
        .create_buffer(&BufferDesc {
            size: 32,
            usage: BufferUsage::STORAGE | BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .unwrap();
    b.write_buffer(&filled, 0, &[0; 32], WriteMode::Ordered);
    let none = [None; MAX_BINDING_SETS];
    stages
        .run_after_domain(
            &mut b,
            &StageDraw {
                vertex_sets: none,
                geometry_sets: hull_sets.each_ref().map(Option::as_ref),
                index: None,
                topology: InputTopology::PatchList(PATCH_POINTS as u8),
                range: DrawRange::Vertices {
                    first: 0,
                    count: PATCHES * PATCH_POINTS,
                    first_instance: 0,
                    instances: 1,
                },
                stream_output: Some(StreamTargets {
                    buffers: [
                        Some(BufferRange {
                            buffer: &target,
                            offset: 0,
                            size: streamed_bytes,
                        }),
                        None,
                        None,
                        None,
                    ],
                    filled: BufferRange {
                        buffer: &filled,
                        offset: 0,
                        size: 32,
                    },
                }),
            },
            &DomainSets {
                domain_sets: none,
                geometry_sets: none,
            },
        )
        .unwrap();
    assert_eq!(b.status(), DeviceStatus::Ok);

    let h = stages.hull.as_ref().unwrap();
    let (counts, _, _) = h.tessellator.owned();
    let (counts_buffer, counts_size) = (counts.buffer.clone(), counts.size);
    let (per_points, _) = tess::patch_bounds(h.tessellation.max_factor);
    let d = stages.domain.as_ref().unwrap();
    let (record_words, records) = (d.words, d.records.as_ref().unwrap());
    let (records_buffer, records_size) = (records.buffer.clone(), records.size);
    let total = words(&mut b, &counts_buffer, counts_size)[tess::POINT_TOTAL as usize];
    let bound = PATCHES * per_points;
    assert!(
        0 < total && total < bound,
        "the patches make {total} points, fewer than the {bound} the dispatch covers"
    );
    let records = words(&mut b, &records_buffer, records_size);
    let w = |point: u32| records[(point * record_words) as usize + 3];
    let outside = super::super::OUTSIDE_W.to_bits();
    for point in 0..total {
        assert_ne!(w(point), outside, "point {point} of {total} was drawn");
    }
    for point in total..bound {
        let at = (point * record_words) as usize;
        assert_eq!(
            records[at..at + 4],
            [0, 0, 0, outside],
            "invocation {point}, past the {total} points, wrote no point"
        );
    }
    stages.destroy(&mut b);
    for set in hull_sets.into_iter().flatten() {
        b.destroy(Object::BindingSet(set));
    }
    for buffer in [constant_buffer, target, filled] {
        b.destroy(Object::Buffer(buffer));
    }
}
