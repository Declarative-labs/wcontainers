//! Stream output (transform feedback): the vertex program's outputs written to the bound buffers by
//! the shaders crate's stage helper, as compute work before the draw (`StreamOutAsCompute`).

use std::ffi::c_uint;
use std::sync::atomic::{AtomicI32, Ordering};

use wcontainers_gpu::{
    Backend, BufferDesc, BufferRange, BufferUsage, Interpolation, MapIntent, Object, STORAGE_WORD,
    ScalarKind, StageValue, SystemValue, WriteMode,
};
use wcontainers_nir::mesa::StreamedOutput;
use wcontainers_shaders::stage::{
    STREAM_BUFFERS, STREAM_FILLED_WORDS, StreamOutput, StreamOutputEntry, StreamTargets,
};

use crate::context::Context;
use crate::gallium::*;
use crate::resource::{self, Resource};
use crate::state::refuse;

/// The semantics the driver names a vertex program's outputs by in the stage helper's terms: a
/// value at a location (its index the location), and the position.
const LOCATION_SEMANTIC: &str = "LOCATION";
const POSITION_SEMANTIC: &str = "POSITION";
/// Gallium's offset asking a target to go on from where it was filled to.
const APPEND: c_uint = c_uint::MAX;
/// Components of one location.
const LOCATION_COMPONENTS: u32 = naga::VectorSize::Quad as u32;

/// The stage helper's declaration of the outputs st/mesa streams, each buffer's elements in
/// offset order with the gaps between them named as gaps.
pub fn declaration(outputs: &[StreamedOutput], strides: [u32; STREAM_BUFFERS]) -> StreamOutput {
    let mut sorted = outputs.to_vec();
    sorted.sort_unstable_by_key(|o| (o.buffer, o.offset));
    let mut entries = Vec::new();
    let mut ends = [0u32; STREAM_BUFFERS];
    let word = STORAGE_WORD as u32;
    for o in sorted {
        let slot = o.buffer as usize;
        let mut gap = o.offset.saturating_sub(ends[slot]) / word;
        while gap > 0 {
            let count = gap.min(LOCATION_COMPONENTS);
            entries.push(StreamOutputEntry {
                stream: 0,
                semantic: None,
                semantic_index: 0,
                start_component: 0,
                component_count: count as u8,
                slot: o.buffer as u8,
            });
            gap -= count;
        }
        let (semantic, index) = if o.position {
            (POSITION_SEMANTIC, 0)
        } else {
            (LOCATION_SEMANTIC, o.location)
        };
        entries.push(StreamOutputEntry {
            stream: 0,
            semantic: Some(semantic.into()),
            semantic_index: index,
            start_component: o.first_component,
            component_count: o.components,
            slot: o.buffer as u8,
        });
        ends[slot] = o.offset + u32::from(o.components) * word;
    }
    StreamOutput { entries, strides }
}

/// A converted vertex program's outputs in the stage helper's terms (`declaration`'s names).
pub fn outputs(module: &naga::Module) -> Vec<StageValue> {
    let mut values = Vec::new();
    let Some(result) = module
        .entry_points
        .first()
        .and_then(|e| e.function.result.as_ref())
    else {
        return values;
    };
    let mut note = |binding: &Option<naga::Binding>, ty: naga::Handle<naga::Type>| {
        let (components, scalar) = match module.types[ty].inner {
            naga::TypeInner::Vector { size, scalar } => (size as u32, scalar.kind),
            naga::TypeInner::Scalar(scalar) => (1, scalar.kind),
            _ => (LOCATION_COMPONENTS, naga::ScalarKind::Float),
        };
        let scalar = match scalar {
            naga::ScalarKind::Sint => ScalarKind::Sint,
            naga::ScalarKind::Uint => ScalarKind::Uint,
            _ => ScalarKind::Float,
        };
        let (semantic, index, system) = match binding {
            Some(naga::Binding::BuiltIn(naga::BuiltIn::Position { .. })) => {
                (POSITION_SEMANTIC, 0, Some(SystemValue::Position))
            }
            Some(naga::Binding::Location { location, .. }) => (LOCATION_SEMANTIC, *location, None),
            _ => return,
        };
        values.push(StageValue {
            semantic: semantic.into(),
            semantic_index: index,
            register: index,
            mask: ((1u32 << components) - 1) as u8,
            scalar,
            interpolation: Interpolation::Perspective,
            system,
            location: index,
            component: 0,
            stream: 0,
        });
    };
    match &module.types[result.ty].inner {
        naga::TypeInner::Struct { members, .. } => {
            members.iter().for_each(|m| note(&m.binding, m.ty));
        }
        _ => note(&result.binding, result.ty),
    }
    values
}

/// One stream output target: Gallium's struct first, so that a pointer to it is one to this.
#[repr(C)]
pub struct Target<B: Backend> {
    base: pipe_stream_output_target,
    /// The bytes written from the bound range's start while the target is not bound, one word.
    filled: B::Buffer,
    /// Bytes from the bound range's start (the target's offset down to the storage alignment) to
    /// the target's offset.
    lead: u64,
}

/// The targets bound, and the stage helper's block of filled sizes and statistics, which holds
/// theirs while they are bound.
pub struct Bound<B: Backend> {
    targets: [*mut pipe_stream_output_target; STREAM_BUFFERS],
    pub filled: Option<B::Buffer>,
}

impl<B: Backend> Default for Bound<B> {
    fn default() -> Self {
        Bound {
            targets: [std::ptr::null_mut(); STREAM_BUFFERS],
            filled: None,
        }
    }
}

impl<B: Backend> Bound<B> {
    pub fn active(&self) -> bool {
        self.targets.iter().any(|t| !t.is_null())
    }

    /// Where the stage helper writes one draw's outputs.
    pub fn targets(&self) -> Result<StreamTargets<'_, B>, String> {
        let filled = self
            .filled
            .as_ref()
            .ok_or("stream output without its block")?;
        let mut buffers = [const { None }; STREAM_BUFFERS];
        for (slot, &t) in buffers.iter_mut().zip(&self.targets) {
            if t.is_null() {
                continue;
            }
            // SAFETY: a bound target holds a reference on itself and on its buffer.
            let target = unsafe { &*t.cast::<Target<B>>() };
            // SAFETY: as above.
            let resource = unsafe { Resource::<B>::of(target.base.buffer) };
            *slot = Some(BufferRange {
                buffer: resource.buffer().ok_or("stream output into a texture")?,
                offset: u64::from(target.base.buffer_offset) - target.lead,
                size: target.lead + u64::from(target.base.buffer_size),
            });
        }
        Ok(StreamTargets {
            buffers,
            filled: BufferRange {
                buffer: filled,
                offset: 0,
                size: STREAM_FILLED_WORDS as u64 * STORAGE_WORD,
            },
        })
    }

    /// Lets every target go, with the context.
    pub fn release(&mut self, screen: &crate::screen::Screen<B>) {
        for t in std::mem::replace(&mut self.targets, [std::ptr::null_mut(); STREAM_BUFFERS]) {
            // SAFETY: a bound target holds a reference.
            unsafe { unreference::<B>(t, screen) };
        }
        if let Some(filled) = self.filled.take() {
            screen.retire(Object::Buffer(filled));
        }
    }
}

const COUNTER_USAGE: BufferUsage = BufferUsage::STORAGE
    .union(BufferUsage::COPY_SRC)
    .union(BufferUsage::COPY_DST)
    .union(BufferUsage::READ_BACK);

/// `pipe_context::create_stream_output_target`.
pub unsafe extern "C" fn create_stream_output_target<B: Backend>(
    pipe: *mut pipe_context,
    buffer: *mut pipe_resource,
    offset: c_uint,
    size: c_uint,
) -> *mut pipe_stream_output_target {
    // SAFETY: Mesa passes this driver's context.
    let context = unsafe { Context::<B>::of(pipe) };
    let screen = context.screen();
    let mut backend = screen.backend();
    let align = u64::from(backend.caps().limits.min_storage_offset_alignment);
    let lead = u64::from(offset) % align;
    let filled = backend
        .create_buffer(&BufferDesc {
            size: STORAGE_WORD,
            usage: COUNTER_USAGE,
            intent: MapIntent::Rarely,
        })
        .unwrap_or_else(|e| refuse(format!("a stream output target: {e:?}")));
    backend.write_buffer(&filled, 0, &(lead as u32).to_le_bytes(), WriteMode::Ordered);
    let mut target = Box::new(Target::<B> {
        base: pipe_stream_output_target::default(),
        filled,
        lead,
    });
    target.base.reference.count = 1;
    target.base.context = pipe;
    target.base.buffer_offset = offset;
    target.base.buffer_size = size;
    // SAFETY: the buffer is a live resource Mesa hands over; the target takes a reference.
    unsafe { resource::reference(&mut target.base.buffer, buffer) };
    Box::into_raw(target).cast()
}

/// `pipe_context::stream_output_target_destroy`, when its last reference goes.
pub unsafe extern "C" fn stream_output_target_destroy<B: Backend>(
    pipe: *mut pipe_context,
    target: *mut pipe_stream_output_target,
) {
    // SAFETY: Mesa passes this driver's context and a target it made, destroyed once.
    let (context, target) = unsafe {
        (
            Context::<B>::of(pipe),
            Box::from_raw(target.cast::<Target<B>>()),
        )
    };
    context.screen().retire(Object::Buffer(target.filled));
    // SAFETY: the target held a reference on its buffer.
    unsafe { resource::release(target.base.buffer) };
}

/// Drops one reference on a target (null for none), destroying it with the last.
///
/// # Safety
/// `target` is null or a live target of this driver holding the reference dropped.
unsafe fn unreference<B: Backend>(
    target: *mut pipe_stream_output_target,
    screen: &crate::screen::Screen<B>,
) {
    if target.is_null() {
        return;
    }
    // SAFETY: the caller's.
    let last = unsafe { AtomicI32::from_ptr(&raw mut (*target).reference.count) }
        .fetch_sub(1, Ordering::AcqRel)
        == 1;
    if last {
        // SAFETY: the last reference: nothing uses the target after.
        let target = unsafe { Box::from_raw(target.cast::<Target<B>>()) };
        screen.retire(Object::Buffer(target.filled));
        // SAFETY: the target held a reference on its buffer.
        unsafe { resource::release(target.base.buffer) };
    }
}

/// `pipe_context::set_stream_output_targets`: each target unbound keeps its filled size in its
/// own word; each bound goes on from it (`APPEND`) or starts at the offset given.
pub unsafe extern "C" fn set_stream_output_targets<B: Backend>(
    pipe: *mut pipe_context,
    count: c_uint,
    targets: *mut *mut pipe_stream_output_target,
    offsets: *const c_uint,
    _: mesa_prim,
) {
    // SAFETY: Mesa passes this driver's context and `count` targets and offsets.
    let (context, new, offsets) = unsafe {
        (
            Context::<B>::of(pipe),
            std::slice::from_raw_parts(targets, count as usize),
            std::slice::from_raw_parts(offsets, count as usize),
        )
    };
    let screen = context.screen();
    let mut backend = screen.backend();
    let backend = &mut *backend;
    if let Err(error) = context.tracker.close(backend) {
        refuse(format!("stream output targets: {error:?}"));
    }
    let streams = &mut context.streams;
    if streams.filled.is_none() && !new.iter().all(|t| t.is_null()) {
        let block = backend
            .create_buffer(&BufferDesc {
                size: STREAM_FILLED_WORDS as u64 * STORAGE_WORD,
                usage: COUNTER_USAGE,
                intent: MapIntent::Rarely,
            })
            .unwrap_or_else(|e| refuse(format!("stream output's block: {e:?}")));
        streams.filled = Some(block);
    }
    for (slot, old) in streams.targets.iter_mut().enumerate() {
        let word = slot as u64 * STORAGE_WORD;
        if let (Some(filled), false) = (&streams.filled, old.is_null()) {
            // SAFETY: a bound target holds a reference on itself.
            let target = unsafe { &*old.cast::<Target<B>>() };
            backend.copy_buffer(filled, word, &target.filled, 0, STORAGE_WORD);
        }
        // SAFETY: a bound target holds a reference.
        unsafe { unreference::<B>(*old, screen) };
        *old = new.get(slot).copied().unwrap_or(std::ptr::null_mut());
        if old.is_null() {
            continue;
        }
        // SAFETY: Mesa hands over live targets of this driver; the slot takes a reference.
        let target = unsafe { &*old.cast::<Target<B>>() };
        // SAFETY: as above.
        unsafe { AtomicI32::from_ptr(&raw mut (**old).reference.count) }
            .fetch_add(1, Ordering::Relaxed);
        let filled = streams.filled.as_ref().expect("made above");
        match offsets[slot] {
            APPEND => backend.copy_buffer(&target.filled, 0, filled, word, STORAGE_WORD),
            start => {
                let bytes = (target.lead + u64::from(start)) as u32;
                backend.write_buffer(filled, word, &bytes.to_le_bytes(), WriteMode::Ordered);
            }
        }
    }
}
