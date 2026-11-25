//! Query sets larger than WebGPU's 4096 queries a set, paged over several of its sets, and query
//! results taken from one ring frame after frame. Shared by tests/queries.rs and the browser page.

use crate::{draw, scene};
use wcontainers_gpu::{
    BufferUsage, ColorAttachment, DeviceStatus, Features, Gpu, LoadOp, PassDesc, QueryKind,
    SlowPath, StoreOp, WriteMode,
};
use wcontainers_gpu_webgpu::WebGpu;

/// Queries past WebGPU's largest set.
const QUERIES: u32 = 5000;

/// WebGPU's queries a set: the first query of the second page.
const PAGE: u32 = wgpu::QUERY_SET_MAX_QUERIES;

fn gpu<T>(r: Result<T, wcontainers_gpu::GpuError>) -> Result<T, String> {
    r.map_err(|e| format!("{e:?}"))
}

/// Finishes a read back of `count` 64-bit results.
fn words(b: &mut WebGpu, rb: <WebGpu as Gpu>::Readback, count: usize) -> Vec<u64> {
    b.wait_read(&rb);
    let mut bytes = vec![0; count * 8];
    b.finish_read(rb, &mut bytes);
    bytes
        .chunks(8)
        .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn pass_with<'a>(t: &'a draw::Targets, set: &'a <WebGpu as Gpu>::QuerySet) -> PassDesc<'a, WebGpu> {
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: Some(set),
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &t.color_view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear(scene::CLEAR.map(f32::to_bits)),
        store: StoreOp::Store,
    });
    pass
}

/// An occlusion set of 5000 queries in one pass: 4095 on the first page and 4096 on the second
/// in their own slots, 4097 there drawing nothing, 4094 begun on the second page (in a spare
/// slot), and 4999 carried across a split by an ordered write. Results resolved across the pages'
/// boundary into a buffer at an offset, and read. Where the device has timestamps, a timestamp
/// set of 5000 written on both pages, read across the boundary and ordered.
pub fn paged_sets(b: &mut WebGpu) -> Result<String, String> {
    let p =
        draw::prepare(b, scene::Scene::Triangle, draw::Twist::None).map_err(|e| e.to_string())?;
    let t = draw::targets(b, None).map_err(|e| e.to_string())?;
    let set = gpu(b.create_query_set(QueryKind::OcclusionAny, QUERIES))?;
    let results = draw::buffer(
        b,
        &[0xee; 48],
        BufferUsage::COPY_SRC | BufferUsage::READ_BACK,
    )
    .map_err(|e| e.to_string())?;
    let vertices: Vec<u8> = bytemuck::cast_slice(&scene::TRIANGLE).to_vec();
    let segmented = b.slow_paths().counts[SlowPath::QueriesSegmented as usize];
    b.begin_pass(&pass_with(&t, &set));
    for (query, drawn) in [
        (PAGE - 1, true),
        (PAGE, true),
        (PAGE + 1, false),
        (PAGE - 2, true),
    ] {
        b.begin_query(query);
        if drawn {
            draw::draw(b, &p);
        }
        b.end_query();
    }
    b.begin_query(QUERIES - 1);
    draw::draw(b, &p);
    b.write_buffer(&p.vertices, 0, &vertices, WriteMode::Ordered);
    draw::draw(b, &p);
    b.end_query();
    b.end_pass();
    gpu(b.resolve_queries(&set, PAGE - 2, 4, &results, 8))?;
    let around = gpu(b.read_back(&wcontainers_gpu::ReadSource::Buffer {
        buffer: &results,
        offset: 0,
        size: 48,
    }))?;
    let last = gpu(b.read_queries(&set, QUERIES - 1, 1))?;
    b.submit();
    let around = words(b, around, 6);
    let last = words(b, last, 1);
    if b.status() != DeviceStatus::Ok {
        return Err(format!("occlusion set of {QUERIES}: {:?}", b.status()));
    }
    let edge = 0xeeee_eeee_eeee_eeee;
    let (a, l) = (&around, last[0]);
    if a[0] != edge || a[5] != edge || a[1] == 0 || a[2] == 0 || a[3] == 0 || a[4] != 0 || l == 0 {
        return Err(format!(
            "occlusion set of {QUERIES}: 4094 to 4097 at offset 8 read {a:x?}, 4999 {l}"
        ));
    }
    // One split moved the part to the second page, one carried 4999.
    let split = b.slow_paths().counts[SlowPath::QueriesSegmented as usize] - segmented;
    if split != 2 {
        return Err(format!(
            "occlusion set of {QUERIES}: {split} segmented, not 2"
        ));
    }
    let mut line = format!(
        "occlusion set of {QUERIES}: 4094 to 4097 {:?}, 4999 {l}",
        &a[1..5]
    );
    if !b.caps().features.contains(Features::TIMESTAMPS) {
        line += "; timestamps absent";
        return Ok(line);
    }
    let stamps = gpu(b.create_query_set(QueryKind::Timestamp, QUERIES))?;
    for index in [PAGE - 2, PAGE - 1, PAGE, PAGE + 1, QUERIES - 1] {
        gpu(b.write_timestamp(&stamps, index))?;
        b.begin_pass(&pass_with(&t, &set));
        draw::draw(b, &p);
        b.end_pass();
    }
    let around = gpu(b.read_queries(&stamps, PAGE - 2, 4))?;
    let last = gpu(b.read_queries(&stamps, QUERIES - 1, 1))?;
    b.submit();
    let mut times = words(b, around, 4);
    times.extend(words(b, last, 1));
    if b.status() != DeviceStatus::Ok {
        return Err(format!("timestamp set of {QUERIES}: {:?}", b.status()));
    }
    if times[0] == 0 || times.windows(2).any(|w| w[1] < w[0]) {
        return Err(format!(
            "timestamp set of {QUERIES}: 4094 to 4097 and 4999 out of order: {times:?}"
        ));
    }
    line += &format!(
        "; timestamp set of {QUERIES}: 4094 to 4097 and 4999 in order, {} ns from first to last",
        times[4] - times[0]
    );
    Ok(line)
}

/// `frames` frames each resolving and reading a query that draws on even frames and not on odd
/// ones: every result as drawn, all taken from one query ring.
pub fn results_ring(b: &mut WebGpu, frames: u32) -> Result<String, String> {
    let p =
        draw::prepare(b, scene::Scene::Triangle, draw::Twist::None).map_err(|e| e.to_string())?;
    let t = draw::targets(b, None).map_err(|e| e.to_string())?;
    let set = gpu(b.create_query_set(QueryKind::OcclusionAny, 2))?;
    let results = draw::buffer(b, &[0; 16], BufferUsage::COPY_SRC | BufferUsage::READ_BACK)
        .map_err(|e| e.to_string())?;
    let rings = b.counters().query_rings;
    for frame in 0..frames {
        b.begin_pass(&pass_with(&t, &set));
        b.begin_query(0);
        if frame % 2 == 0 {
            draw::draw(b, &p);
        }
        b.end_query();
        b.begin_query(1);
        draw::draw(b, &p);
        b.end_query();
        b.end_pass();
        gpu(b.resolve_queries(&set, 0, 2, &results, 0))?;
        let resolved = gpu(b.read_back(&wcontainers_gpu::ReadSource::Buffer {
            buffer: &results,
            offset: 0,
            size: 16,
        }))?;
        let read = gpu(b.read_queries(&set, 0, 2))?;
        b.submit();
        let (resolved, read) = (words(b, resolved, 2), words(b, read, 2));
        if resolved != read || (read[0] != 0) != (frame % 2 == 0) || read[1] == 0 {
            return Err(format!(
                "frame {frame}: resolved {resolved:?}, read {read:?}"
            ));
        }
    }
    let (made, all) = (b.counters().query_rings - rings, b.counters().query_rings);
    if b.status() != DeviceStatus::Ok || made > 1 || all != 1 {
        return Err(format!(
            "{frames} frames of results made {made} query rings, {all} on the device; {:?}",
            b.status()
        ));
    }
    Ok(format!(
        "query results: {frames} frames resolved and read as drawn; {made} query rings made in \
         them, 1 on the device"
    ))
}
