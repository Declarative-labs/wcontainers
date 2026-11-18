//! The tessellator's run time: three dispatches of `tessellator.wgsl` over a draw's patches, which
//! leave the domain points, the indices joining them and an indexed indirect draw of them in buffers
//! the tessellator owns.

use super::helper::{Owned, StageError, grow};
use super::tess;
use wcontainers_gpu::{
    Backend, BindingEntry, BindingResource, BufferRange, BufferUsage, DispatchCall, DispatchGroups,
    MAX_BINDING_SETS, Program, RingKind, StageSet,
};

/// The fixed-function tessellator's state, as a hull shader declares it (`StageExtras::Hull`'s
/// numbering).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tessellation {
    /// Triangle (0), quad (1) or isoline (2).
    pub domain: u8,
    /// Integer (0), powers of two (1), fractional odd (2), fractional even (3).
    pub partitioning: u8,
    /// Points (0), lines (1), clockwise (2) or counter-clockwise (3) triangles.
    pub output: u8,
    /// The largest factor the hull shader declares; every factor it writes is clamped to it.
    pub max_factor: f32,
}

/// The tessellator program and its output buffers, grown as draws need.
pub struct Tessellator<B: Backend> {
    program: B::Program,
    counts: Option<Owned<B>>,
    points: Option<Owned<B>>,
    indices: Option<Owned<B>>,
}

/// Invocations per workgroup of the tessellator program.
const WORKGROUP: u32 = 64;

impl<B: Backend> Tessellator<B> {
    pub fn new(backend: &mut B) -> Result<Self, StageError> {
        let shader = backend.create_shader(&super::tessellator_program()?)?;
        Ok(Tessellator {
            program: backend.create_program(&Program::Compute(&shader))?,
            counts: None,
            points: None,
            indices: None,
        })
    }

    /// Records the passes for `patches` patches (`primitives` per API instance) whose records are in
    /// `hull` (header first). Afterwards [`Self::counts`] holds the indexed indirect draw of
    /// [`Self::indices`] (u32 point numbers) at offset 0, and [`Self::points`] the points.
    pub fn run(
        &mut self,
        backend: &mut B,
        t: &Tessellation,
        hull: BufferRange<&B::Buffer>,
        patches: u32,
        primitives: u32,
    ) -> Result<(), StageError> {
        if t.domain > 2 || t.partitioning > 3 || t.output > 3 || (t.domain == 2 && t.output > 1) {
            return Err(StageError::Declaration);
        }
        let (per_points, per_indices) = tess::patch_bounds(t.max_factor);
        let patches64 = u64::from(patches.max(1));
        let usage = BufferUsage::STORAGE | BufferUsage::COPY_SRC;
        let counts_bytes = (u64::from(tess::COUNTS_BASE) + 2 * patches64) * 4;
        grow(
            backend,
            &mut self.counts,
            counts_bytes,
            usage | BufferUsage::INDIRECT,
        )?;
        let points_bytes = patches64 * u64::from(per_points) * 16;
        grow(backend, &mut self.points, points_bytes, usage)?;
        let indices_bytes =
            (patches64 * u64::from(per_indices) + u64::from(tess::INDEX_HEADER)) * 4;
        grow(
            backend,
            &mut self.indices,
            indices_bytes,
            usage | BufferUsage::INDEX,
        )?;

        let limits = backend.caps().limits;
        let values = |mode: u32| -> [u8; 32] {
            let words = [
                patches,
                mode,
                t.domain.into(),
                t.partitioning.into(),
                t.output.into(),
                primitives.max(1),
                0,
                0,
            ];
            let mut bytes = [0; 32];
            for (b, w) in bytes.chunks_mut(4).zip(words) {
                b.copy_from_slice(&w.to_le_bytes());
            }
            bytes
        };
        let parts = [values(0), values(1), values(2)];
        let mut offsets = [0u64; 3];
        let ring = backend.write_buffer_ring(
            RingKind::Data,
            &[&parts[0], &parts[1], &parts[2]],
            limits.min_uniform_offset_alignment,
            &mut offsets,
        )?;
        let groups = patches.div_ceil(WORKGROUP);
        for (mode, offset) in offsets.into_iter().enumerate() {
            let entries = [
                BindingEntry {
                    binding: 0,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &ring,
                        offset,
                        size: 32,
                    }),
                },
                BindingEntry {
                    binding: 1,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::StorageBuffer {
                        range: hull,
                        writable: false,
                    },
                },
                storage(2, &self.counts.as_ref().unwrap().buffer, counts_bytes),
                storage(3, &self.points.as_ref().unwrap().buffer, points_bytes),
                storage(4, &self.indices.as_ref().unwrap().buffer, indices_bytes),
            ];
            let set = backend.create_binding_set(&entries)?;
            let mut sets = [None; MAX_BINDING_SETS];
            sets[super::STAGE_SET as usize] = Some(&set);
            backend.dispatch(&DispatchCall {
                program: &self.program,
                bindings: sets,
                immediates: &[],
                groups: DispatchGroups::Direct([if mode == 1 { 1 } else { groups }, 1, 1]),
            })?;
        }
        Ok(())
    }

    /// The counts, points and indices buffers with their sizes, after a run.
    pub(super) fn owned(&self) -> (&Owned<B>, &Owned<B>, &Owned<B>) {
        let ran = "the tessellator ran";
        (
            self.counts.as_ref().expect(ran),
            self.points.as_ref().expect(ran),
            self.indices.as_ref().expect(ran),
        )
    }

    /// Releases its program and buffers.
    pub fn destroy(self, backend: &mut B) {
        use wcontainers_gpu::Object;
        backend.destroy(Object::Program(self.program));
        for owned in [self.counts, self.points, self.indices]
            .into_iter()
            .flatten()
        {
            backend.destroy(Object::Buffer(owned.buffer));
        }
    }

    /// The indexed indirect draw's arguments (five words at offset 0), the draw's point total (word
    /// 5), and from word [`tess::COUNTS_BASE`] each patch's first point and first index.
    pub fn counts(&self) -> Option<&B::Buffer> {
        self.counts.as_ref().map(|o| &o.buffer)
    }

    /// The points: four u32 each, u and v (f32 bits), the patch's number in the draw and its
    /// primitive number in its API instance.
    pub fn points(&self) -> Option<&B::Buffer> {
        self.points.as_ref().map(|o| &o.buffer)
    }

    /// The indices: the draw's index total, then from word [`tess::INDEX_HEADER`] u32 point numbers,
    /// three per triangle, two per line, one per point.
    pub fn indices(&self) -> Option<&B::Buffer> {
        self.indices.as_ref().map(|o| &o.buffer)
    }
}

fn storage<B: Backend>(binding: u32, buffer: &B::Buffer, size: u64) -> BindingEntry<'_, B> {
    BindingEntry {
        binding,
        stages: StageSet::COMPUTE,
        resource: BindingResource::StorageBuffer {
            range: BufferRange {
                buffer,
                offset: 0,
                size,
            },
            writable: true,
        },
    }
}
