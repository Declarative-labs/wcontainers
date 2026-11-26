//! The recording back end's present-or-absent operations: compute dispatches, vertex capture and
//! timestamps. Each checks its rules and answers `Err(Ok(why))` for a broken one, `Err(Err(e))` for
//! an absent feature.

use crate::draw::{Bindings, arguments, assemble, entry};
use crate::draw::{
    apply_writes, binding_limits, bound_subresources, draw_call, draw_reads, draw_rules, gather,
    program_bindings, sets_of, shade_vertex, stream_usages, vertex_list,
};
use crate::program::{self, Error, Value};
use crate::recorder::{Bound, Entries, Id, Obj, Recorder, varyings};
use crate::recorder::{buffer_for, buffer_obj, object_of, outside_pass, touch_in, touch_use};
use naga::{Binding, BuiltIn, ScalarKind};
use std::cell::RefCell;
use wcontainers_gpu::{caps::*, draw::*, error::*, resource::*, shader::*, *};

type Refusal = Result<String, GpuError>;

/// Checks a compute program's workgroup against the limits.
pub(crate) fn workgroup_limits(rec: &Recorder, shader: &ShaderModule) -> Result<(), &'static str> {
    let l = rec.caps().limits;
    let ep = entry(shader).map_err(|_| "a compute shader without its entry point")?;
    let size = shader.module.entry_points[ep].workgroup_size;
    if size.iter().zip(l.compute_size).any(|(&n, m)| n > m)
        || size.iter().map(|&n| u64::from(n)).product::<u64>() > u64::from(l.compute_invocations)
    {
        return Err("a workgroup past the compute limits");
    }
    let info = shader.info.get_entry_point(ep);
    let m = &shader.module;
    let storage: u64 = m
        .global_variables
        .iter()
        .filter(|(h, g)| g.space == naga::AddressSpace::WorkGroup && !info[*h].is_empty())
        .map(|(_, g)| u64::from(m.types[g.ty].inner.size(m.to_ctx())))
        .sum();
    if storage > u64::from(l.compute_workgroup_storage) {
        return Err("workgroup variables past the limit");
    }
    Ok(())
}

pub(crate) fn dispatch_call(
    rec: &mut Recorder,
    call: &DispatchCall<'_, Recorder>,
) -> Result<(), Refusal> {
    needs_feature(rec, Features::COMPUTE)?;
    outside_pass(rec, "dispatch").map_err(Ok)?;
    let groups = match call.groups {
        DispatchGroups::Direct(g) => g,
        DispatchGroups::Indirect { .. } => {
            return Err(Err(GpuError::Absent(Features::INDIRECT)));
        }
    };
    let shader = match object_of(rec, *call.program).map_err(Ok)? {
        Obj::ComputeProgram(s) => s.clone(),
        _ => {
            return Err(Ok(
                "a dispatch of a program that is not a compute program".into()
            ));
        }
    };
    if groups.iter().any(|&n| n > rec.caps().limits.max_dispatch) {
        return Err(Ok("a dispatch of more groups than max_dispatch".into()));
    }
    binding_limits(rec, &call.bindings, call.immediates).map_err(Ok)?;
    let sets = sets_of(rec, &call.bindings).map_err(Ok)?;
    program_bindings(rec, &[&shader], &sets).map_err(Ok)?;
    // One usage scope: a buffer bound writable is bound no other way.
    let buffers: Vec<(Id, bool)> = sets
        .iter()
        .flatten()
        .flatten()
        .filter_map(|(_, _, b)| match *b {
            Bound::Buffer {
                buffer, writable, ..
            } => Some((buffer, writable)),
            _ => None,
        })
        .collect();
    if buffers
        .iter()
        .any(|&(b, w)| w && buffers.iter().filter(|(o, _)| *o == b).count() > 1)
    {
        return Err(Ok(
            "a dispatch binding a buffer as writable storage and another way".into(),
        ));
    }
    touch_bound(rec, &sets).map_err(Ok)?;
    let m = &shader.module;
    let ep = entry(&shader).map_err(Ok)?;
    let fun = &m.entry_points[ep].function;
    let size = m.entry_points[ep].workgroup_size;
    let writes = RefCell::default();
    let res = Bindings {
        rec,
        sets: &sets,
        writes: &writes,
    };
    let vec3 = |v: [u32; 3]| Value::Composite(v.map(Value::U32).to_vec());
    for g in cube(groups) {
        for l in cube(size) {
            let global = [0, 1, 2].map(|i| g[i] * size[i] + l[i]);
            let index = (l[2] * size[1] + l[1]) * size[0] + l[0];
            let args = arguments(m, fun, |binding, _| match *binding {
                Binding::BuiltIn(BuiltIn::GlobalInvocationId) => Ok(vec3(global)),
                Binding::BuiltIn(BuiltIn::LocalInvocationId) => Ok(vec3(l)),
                Binding::BuiltIn(BuiltIn::LocalInvocationIndex) => Ok(Value::U32(index)),
                Binding::BuiltIn(BuiltIn::WorkGroupId) => Ok(vec3(g)),
                Binding::BuiltIn(BuiltIn::NumWorkGroups) => Ok(vec3(groups)),
                ref b => Err(format!("compute input {b:?}")),
            })
            .map_err(Ok)?;
            program::run(m, &shader.info, ep, args, &res, &mut writes.borrow_mut()).map_err(Ok)?;
        }
    }
    apply_writes(rec, &sets, writes.into_inner());
    Ok(())
}

pub(crate) fn capture_call(
    rec: &mut Recorder,
    call: &DrawCall<'_, Recorder>,
    targets: CaptureTargets<'_, Recorder>,
    rasterize: bool,
) -> Result<(), Refusal> {
    needs_feature(rec, Features::TRANSFORM_FEEDBACK)?;
    match (rasterize, rec.pass.is_some()) {
        (true, false) => return Err(Ok("a rasterised capture outside a pass".into())),
        (false, true) => return Err(Ok("a capture without rasterising inside a pass".into())),
        _ => {}
    }
    if targets.is_empty() || targets.len() > rec.caps().limits.capture_buffers as usize {
        return Err(Ok(
            "capture targets of none or more than capture_buffers".into()
        ));
    }
    let state = gather(rec, call).map_err(Ok)?;
    draw_rules(rec, call).map_err(Ok)?;
    let shaders: Vec<&ShaderModule> = std::iter::once(&*state.vertex).collect();
    program_bindings(rec, &shaders, &state.sets).map_err(Ok)?;
    stream_usages(rec, call).map_err(Ok)?;
    let mut used: Vec<Id> = call
        .vertex_buffers
        .iter()
        .flatten()
        .map(|v| *v.buffer)
        .chain(call.index.iter().map(|i| *i.buffer))
        .collect();
    used.extend(
        state
            .sets
            .iter()
            .flatten()
            .flatten()
            .filter_map(|(_, _, b)| match *b {
                Bound::Buffer { buffer, .. } => Some(buffer),
                _ => None,
            }),
    );
    for t in targets {
        buffer_for(rec, *t.buffer, BufferUsage::CAPTURE, "a capture target").map_err(Ok)?;
        let end = t.offset.checked_add(t.size);
        if end.is_none_or(|e| e > buffer_obj(rec, *t.buffer).map_or(0, |b| b.bytes.len() as u64)) {
            return Err(Ok("a capture target past its buffer's end".into()));
        }
        if used.contains(t.buffer) {
            return Err(Ok("a capture target the call also uses another way".into()));
        }
    }
    if !rasterize {
        for (buffer, start, end) in draw_reads(rec, call, &state).map_err(Ok)? {
            touch_use(rec, buffer, start, end);
        }
        // The bound buffers are among the reads; the bound textures are not.
        for (texture, sub) in bound_subresources(rec, &state.sets).map_err(Ok)? {
            touch_in(rec, texture, sub, 0, u64::MAX);
        }
    }
    let outputs = varyings(&state.vertex);
    if outputs.is_empty() {
        return Err(Ok("a capture of a program with no location outputs".into()));
    }
    if targets.len() > 1 && targets.len() != outputs.len() {
        return Err(Ok(
            "several capture targets, not one for each output the program writes".into(),
        ));
    }
    if targets.len() == 1 && outputs.iter().sum::<u32>() > rec.caps().limits.capture_components {
        return Err(Ok(
            "a capture into one target of more components than capture_components".into(),
        ));
    }
    let (list, first_instance, instances) = vertex_list(rec, call).map_err(Ok)?;
    let order = assemble(&list, call.topology, state.raster.provoking).map_err(Ok)?;
    // Each vertex's outputs in location order, as 32-bit components.
    let mut vertices: Vec<Vec<Vec<u8>>> = Vec::new();
    for instance in first_instance..first_instance + instances {
        for &v in &order {
            let (_, values) =
                shade_vertex(rec, &state, call, v, instance, first_instance).map_err(Ok)?;
            vertices.push(
                values
                    .iter()
                    .map(|o| o.numbers.iter().flat_map(|&n| bits(o.kind, n)).collect())
                    .collect(),
            );
        }
    }
    let mut bytes: Vec<Vec<u8>> = vec![Vec::new(); targets.len()];
    for vertex in &vertices {
        for (n, output) in vertex.iter().enumerate() {
            bytes[if targets.len() == 1 { 0 } else { n }].extend(output);
        }
    }
    if targets
        .iter()
        .zip(&bytes)
        .any(|(t, b)| b.len() as u64 > t.size)
    {
        return Err(Ok(
            "capture targets that do not hold every vertex the draw makes".into(),
        ));
    }
    for (t, b) in targets.iter().zip(bytes) {
        let end = t.offset + b.len() as u64;
        if let Some(Obj::Buffer(o)) = rec.objects[t.buffer.0 as usize].as_mut() {
            o.bytes[t.offset as usize..end as usize].copy_from_slice(&b);
        }
        touch_use(rec, *t.buffer, t.offset, end);
    }
    if rasterize {
        draw_call(rec, call).map_err(Ok)?;
    }
    Ok(())
}

pub(crate) fn timestamp_call(rec: &mut Recorder, set: Id, index: u32) -> Result<(), Refusal> {
    needs_feature(rec, Features::TIMESTAMPS)?;
    outside_pass(rec, "write_timestamp").map_err(Ok)?;
    let now = rec.clock;
    match rec.objects.get_mut(set.0 as usize) {
        Some(Some(Obj::QuerySet {
            kind: QueryKind::Timestamp,
            results,
        })) => match results.get_mut(index as usize) {
            Some(r) => *r = now,
            None => return Err(Ok("a timestamp index outside its set".into())),
        },
        _ => {
            return Err(Ok(
                "a timestamp written into what is not a live timestamp set".into(),
            ));
        }
    }
    Ok(())
}

/// Answers an absent feature as such.
fn needs_feature(rec: &Recorder, feature: Features) -> Result<(), Refusal> {
    match rec.caps().features.contains(feature) {
        true => Ok(()),
        false => Err(Err(GpuError::Absent(feature))),
    }
}

/// Marks every buffer range and texture subresource the binding sets bind as used by a command.
fn touch_bound(
    rec: &mut Recorder,
    sets: &[Option<Entries>; MAX_BINDING_SETS],
) -> Result<(), Error> {
    for (_, _, bound) in sets.iter().flatten().flatten() {
        if let Bound::Buffer {
            buffer,
            offset,
            size,
            ..
        } = *bound
        {
            touch_use(rec, buffer, offset, offset + size);
        }
    }
    for (texture, sub) in bound_subresources(rec, sets)? {
        touch_in(rec, texture, sub, 0, u64::MAX);
    }
    Ok(())
}

/// Every point of a box of `size`, x fastest.
fn cube(size: [u32; 3]) -> impl Iterator<Item = [u32; 3]> {
    (0..size[2])
        .flat_map(move |z| (0..size[1]).flat_map(move |y| (0..size[0]).map(move |x| [x, y, z])))
}

/// A captured component's 32 bits, little-endian.
fn bits(kind: ScalarKind, n: f64) -> [u8; 4] {
    match kind {
        ScalarKind::Sint => (n as i32).to_le_bytes(),
        ScalarKind::Uint | ScalarKind::Bool => (n as u32).to_le_bytes(),
        _ => (n as f32).to_le_bytes(),
    }
}
