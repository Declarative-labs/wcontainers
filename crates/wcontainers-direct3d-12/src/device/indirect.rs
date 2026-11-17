//! `ExecuteIndirect` (plan 5.2). A signature of only a draw, indexed draw or dispatch becomes one
//! indirect call per command at the signature's stride; with a count buffer, the commands are
//! copied and a compute program zeroes those past the count, then all are called
//! (`IndirectCountExpanded`, nothing waits). Signatures that also change vertex or index buffers or
//! root parameters, draws on back ends whose indirect draws ignore a first instance, and draws of
//! programs that read `SV_VertexID` or `SV_InstanceID` read the arguments back and record ordinary
//! calls (`IndirectReadBack`).

use super::query::Own;
use super::*;
use crate::list::{CommandSignature, IndexBufferView, IndirectArgument, RootArgument};

/// Zeroes the first word (the vertex, index or group count) of each command past the count.
pub(super) const EXPAND: &str = "
@group(0) @binding(32) var<storage, read> count: array<u32>;
@group(0) @binding(160) var<storage, read_write> commands: array<u32>;
struct Expand { word: u32, stride: u32, max: u32 }
@group(0) @binding(0) var<uniform> expand: Expand;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3u) {
    if id.x >= expand.max || id.x < count[expand.word] { return; }
    commands[id.x * expand.stride] = 0u;
}
";

impl<B: Backend> Device<B> {
    /// `size` bytes at `offset` of buffer `id`, as they are once the work recorded so far is done:
    /// an upload buffer's from its memory, others read back now.
    fn read_now(&mut self, id: ResourceId, offset: u64, size: u64) -> Result<Vec<u8>, Error> {
        if let Some(Resource {
            heap: HeapType::Upload,
            storage: Storage::Buffer {
                host: Some(host), ..
            },
            ..
        }) = self.resources.get_mut(id.0 as usize)
        {
            return Ok(host.bytes()[offset as usize..(offset + size) as usize].to_vec());
        }
        let buffer = self.buffer(id)?.0.clone();
        let scratch = self.scratch(size, BufferUsage::COPY_DST | BufferUsage::READ_BACK)?;
        self.backend
            .copy_buffer(&buffer, offset, &scratch, 0, size.next_multiple_of(4));
        let mut bytes = vec![0; size as usize];
        let read = self.backend.read_back_now(
            &ReadSource::Buffer {
                buffer: &scratch,
                offset: 0,
                size,
            },
            &mut bytes,
        );
        self.backend.destroy(Object::Buffer(scratch));
        read?;
        Ok(bytes)
    }
}

impl<B: Backend> Replay<B> {
    pub(super) fn execute_indirect(
        &mut self,
        d: &mut Device<B>,
        s: &CommandSignature,
        max: u32,
        args: ResourceId,
        args_offset: u64,
        count: Option<(ResourceId, u64)>,
    ) -> Result<(), Error> {
        let width = |d: &Device<B>, id: ResourceId| {
            d.resources
                .get(id.0 as usize)
                .map(|r| r.desc.width)
                .ok_or(Error::Invalid("no such resource"))
        };
        let size: u32 = s.arguments.iter().map(|a| a.size()).sum();
        let span = match max {
            0 => 0,
            n => u64::from(n - 1) * u64::from(s.stride) + u64::from(size),
        };
        if !args_offset.is_multiple_of(4) || args_offset + span > width(d, args)? {
            return Err(Error::Invalid("indirect arguments past their buffer's end"));
        }
        if let Some((c, offset)) = count
            && (!offset.is_multiple_of(4) || offset + 4 > width(d, c)?)
        {
            return Err(Error::Invalid("an indirect count past its buffer's end"));
        }
        if max == 0 || self.skipped(d)? {
            return Ok(());
        }
        let Some(pid) = self.pipeline else {
            return Ok(());
        };
        let call = *s.arguments.last().expect("a checked command signature");
        let caps = d.backend.caps();
        let reads_bases = d.pipelines[pid.0 as usize]
            .render
            .as_ref()
            .is_some_and(|r| r.reads_bases);
        let on_gpu = s.call_only()
            && caps.has(Features::INDIRECT)
            && (count.is_none() || caps.has(Features::COMPUTE))
            && match call {
                IndirectArgument::Dispatch => caps.has(Features::COMPUTE),
                _ => caps.has(Features::INDIRECT_FIRST_INSTANCE) && !reads_bases,
            };
        if !on_gpu {
            return self.read_back_indirect(d, s, max, args, args_offset, count, span);
        }
        d.upload(args);
        let source = d.buffer(args)?.0.clone();
        let (buffer, offset, own) = match count {
            None => (source, args_offset, false),
            Some((c, count_offset)) => {
                self.settle(d)?;
                d.backend.slow_paths().hit(SlowPath::IndirectCountExpanded);
                d.upload(c);
                let counts = d.buffer(c)?.0.clone();
                let counts_size = width(d, c)?.next_multiple_of(4);
                let scratch = d.scratch(
                    span,
                    BufferUsage::STORAGE | BufferUsage::INDIRECT | BufferUsage::COPY_DST,
                )?;
                d.backend
                    .copy_buffer(&source, args_offset, &scratch, 0, span);
                d.run_own(
                    Own::Expand,
                    &[(count_offset / 4) as u32, s.stride / 4, max],
                    &[
                        (32, &counts, counts_size, false),
                        (160, &scratch, span, true),
                    ],
                    max,
                )?;
                (scratch, 0, true)
            }
        };
        let result = (0..u64::from(max)).try_for_each(|i| {
            let record = (buffer.clone(), offset + i * u64::from(s.stride));
            match call {
                IndirectArgument::Dispatch => self.dispatch(d, [1; 3], Some(&record)),
                IndirectArgument::DrawIndexed => self.draw(
                    d,
                    DrawRange::Indexed {
                        first: 0,
                        count: 0,
                        base_vertex: 0,
                        first_instance: 0,
                        instances: 0,
                    },
                    Some(&record),
                ),
                _ => self.draw(
                    d,
                    DrawRange::Vertices {
                        first: 0,
                        count: 0,
                        first_instance: 0,
                        instances: 0,
                    },
                    Some(&record),
                ),
            }
        });
        if own {
            d.backend.destroy(Object::Buffer(buffer));
        }
        result
    }

    /// `ExecuteIndirect` by reading the commands back and recording each as ordinary calls.
    #[allow(clippy::too_many_arguments)]
    fn read_back_indirect(
        &mut self,
        d: &mut Device<B>,
        s: &CommandSignature,
        max: u32,
        args: ResourceId,
        args_offset: u64,
        count: Option<(ResourceId, u64)>,
        span: u64,
    ) -> Result<(), Error> {
        self.settle(d)?;
        d.backend.slow_paths().hit(SlowPath::IndirectReadBack);
        let bytes = d.read_now(args, args_offset, span)?;
        let n = match count {
            Some((c, offset)) => {
                let v = d.read_now(c, offset, 4)?;
                u32::from_le_bytes([v[0], v[1], v[2], v[3]]).min(max)
            }
            None => max,
        };
        let compute = s.dispatches();
        for i in 0..n as usize {
            let mut at = i * s.stride as usize;
            for &a in &s.arguments {
                let w = |k: usize| {
                    let p = at + 4 * k;
                    u32::from_le_bytes([bytes[p], bytes[p + 1], bytes[p + 2], bytes[p + 3]])
                };
                let address = || u64::from(w(0)) | u64::from(w(1)) << 32;
                match a {
                    IndirectArgument::Draw => self.draw(
                        d,
                        DrawRange::Vertices {
                            count: w(0),
                            instances: w(1),
                            first: w(2),
                            first_instance: w(3),
                        },
                        None,
                    )?,
                    IndirectArgument::DrawIndexed => self.draw(
                        d,
                        DrawRange::Indexed {
                            count: w(0),
                            instances: w(1),
                            first: w(2),
                            base_vertex: w(3) as i32,
                            first_instance: w(4),
                        },
                        None,
                    )?,
                    IndirectArgument::Dispatch => self.dispatch(d, [w(0), w(1), w(2)], None)?,
                    IndirectArgument::VertexBuffer { slot } => self.run(
                        d,
                        &Command::VertexBuffers {
                            start: slot,
                            views: vec![VertexBufferView {
                                address: address(),
                                size: w(2),
                                stride: w(3),
                            }],
                        },
                    )?,
                    IndirectArgument::IndexBuffer => self.run(
                        d,
                        &Command::IndexBuffer(Some(IndexBufferView {
                            address: address(),
                            size: w(2),
                            format: w(3),
                        })),
                    )?,
                    IndirectArgument::Constants {
                        root,
                        offset,
                        count,
                    } => self.run(
                        d,
                        &Command::RootConstants {
                            compute,
                            index: root,
                            offset,
                            values: (0..count as usize).map(w).collect(),
                        },
                    )?,
                    IndirectArgument::RootView { root } => self.run(
                        d,
                        &Command::RootArgument {
                            compute,
                            index: root,
                            argument: RootArgument::Descriptor(address()),
                        },
                    )?,
                }
                at += a.size() as usize;
            }
        }
        Ok(())
    }
}
