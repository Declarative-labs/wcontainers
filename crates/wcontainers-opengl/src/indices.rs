//! A draw's indices rewritten on the host where the host draws its primitives otherwise than
//! OpenGL: flat values from the other provoking vertex (`ProvokingRewrite`), restart in lists and
//! in strips drawn without it (`RestartRewrite`), and 8-bit indices a back end lacks. Mesa's own
//! index translator and generator (`indices/u_indices`, MIT) and restart splitter
//! (`util/u_prim_restart`, MIT) do the work.

use std::ffi::{c_uint, c_void};

use wcontainers_gpu::{IndexFormat, ProvokingVertex, Topology};
use wcontainers_shaders::stage::TRIANGLE_VERTICES;

use crate::gallium::*;

/// A draw's indices as the host draws them.
pub struct Rewritten {
    /// Mesa's primitive of the indices: a list or strip, or a list with adjacency.
    pub primitive: mesa_prim,
    pub format: IndexFormat,
    /// The indices, little-endian, as words so that either width is aligned.
    words: Vec<u32>,
    bytes: usize,
}

impl Rewritten {
    pub fn indices(&self) -> &[u8] {
        // SAFETY: the words' bytes, of which the first `bytes` were written.
        unsafe { std::slice::from_raw_parts(self.words.as_ptr().cast::<u8>(), self.bytes) }
    }

    pub fn count(&self) -> u32 {
        (self.bytes / self.format.size() as usize) as u32
    }

    /// The host's topology of the indices, which have no adjacency.
    pub fn topology(&self) -> Result<Topology, String> {
        topology(self.primitive)
    }
}

/// The primitives the host draws as they come, and those that never restart there.
const STRIPS_AND_LISTS: &[mesa_prim] = &[
    MESA_PRIM_POINTS,
    MESA_PRIM_LINES,
    MESA_PRIM_LINE_STRIP,
    MESA_PRIM_TRIANGLES,
    MESA_PRIM_TRIANGLE_STRIP,
];
const LISTS: &[mesa_prim] = &[
    MESA_PRIM_POINTS,
    MESA_PRIM_LINES,
    MESA_PRIM_TRIANGLES,
    MESA_PRIM_LINES_ADJACENCY,
    MESA_PRIM_TRIANGLES_ADJACENCY,
];

/// Whether the host's lists and strips would draw `mode` otherwise than OpenGL with restart as
/// given: lists never restart there.
pub fn restarts_otherwise(mode: mesa_prim, restart: bool) -> bool {
    restart && LISTS.contains(&mode)
}

fn mask(primitives: &[mesa_prim]) -> c_uint {
    primitives.iter().fold(0, |mask, &p| mask | 1 << p)
}

fn convention(provoking: ProvokingVertex) -> c_uint {
    match provoking {
        ProvokingVertex::First => PV_FIRST,
        ProvokingVertex::Last => PV_LAST,
    }
}

/// `count` less the vertices of an unfinished last primitive, or zero where not one is whole.
pub fn trimmed(mode: mesa_prim, count: u32) -> u32 {
    let line = 2;
    let [line_adjacency, triangle_adjacency] =
        [line, TRIANGLE_VERTICES].map(|n| n * WITH_ADJACENCY);
    match mode {
        MESA_PRIM_LINES => count - count % line,
        MESA_PRIM_TRIANGLES => count - count % TRIANGLE_VERTICES,
        MESA_PRIM_LINES_ADJACENCY => count - count % line_adjacency,
        MESA_PRIM_TRIANGLES_ADJACENCY => count - count % triangle_adjacency,
        MESA_PRIM_LINE_STRIP | MESA_PRIM_LINE_LOOP if count < line => 0,
        MESA_PRIM_TRIANGLE_STRIP | MESA_PRIM_TRIANGLE_FAN if count < TRIANGLE_VERTICES => 0,
        MESA_PRIM_LINE_STRIP_ADJACENCY if count < line_adjacency => 0,
        MESA_PRIM_TRIANGLE_STRIP_ADJACENCY if count < triangle_adjacency => 0,
        _ => count,
    }
}

/// The vertices of the first primitive with adjacency for each of its own: one beside each.
const WITH_ADJACENCY: u32 = 2;

/// The places of a primitive's own vertices among those of its primitive with adjacency, which
/// OpenGL draws without the adjacent ones where no geometry program runs.
const LINE_IN_ADJACENCY: [usize; 2] = [1, 2];
const TRIANGLE_IN_ADJACENCY: [usize; 3] = [0, 2, 4];

/// A list with adjacency as the list of its primitives without their adjacent vertices
/// (`AdjacencyDropped`); any other as it is.
pub fn without_adjacency(rewritten: Rewritten) -> Rewritten {
    let (primitive, own): (mesa_prim, &[usize]) = match rewritten.primitive {
        MESA_PRIM_LINES_ADJACENCY => (MESA_PRIM_LINES, &LINE_IN_ADJACENCY),
        MESA_PRIM_TRIANGLES_ADJACENCY => (MESA_PRIM_TRIANGLES, &TRIANGLE_IN_ADJACENCY),
        _ => return rewritten,
    };
    let size = rewritten.format.size() as usize;
    let each = own.len() * WITH_ADJACENCY as usize * size;
    let mut kept = Vec::with_capacity(rewritten.bytes / WITH_ADJACENCY as usize);
    for vertices in rewritten.indices().chunks_exact(each) {
        for &at in own {
            kept.extend_from_slice(&vertices[at * size..(at + 1) * size]);
        }
    }
    let mut words = words_for(kept.len());
    for (word, bytes) in words.iter_mut().zip(kept.chunks(size_of::<u32>())) {
        let mut whole = [0; size_of::<u32>()];
        whole[..bytes.len()].copy_from_slice(bytes);
        *word = u32::from_ne_bytes(whole);
    }
    Rewritten {
        primitive,
        format: rewritten.format,
        bytes: kept.len(),
        words,
    }
}

fn topology(primitive: mesa_prim) -> Result<Topology, String> {
    Ok(match primitive {
        MESA_PRIM_POINTS => Topology::PointList,
        MESA_PRIM_LINES => Topology::LineList,
        MESA_PRIM_LINE_STRIP => Topology::LineStrip,
        MESA_PRIM_TRIANGLES => Topology::TriangleList,
        MESA_PRIM_TRIANGLE_STRIP => Topology::TriangleStrip,
        other => return Err(format!("Mesa's index translator gave primitive {other}")),
    })
}

fn format(bytes: c_uint) -> IndexFormat {
    if bytes == IndexFormat::U32.size() {
        IndexFormat::U32
    } else {
        IndexFormat::U16
    }
}

/// Words enough for `bytes`.
fn words_for(bytes: usize) -> Vec<u32> {
    vec![0; bytes.div_ceil(size_of::<u32>())]
}

/// The vertices `first..first + count` drawn as `mode` without indices, as indices that draw them
/// (as lists only where `lists_only`) with the host's provoking vertex `host` where OpenGL's is
/// `api`.
pub fn generated(
    mode: mesa_prim,
    first: u32,
    count: u32,
    lists_only: bool,
    api: ProvokingVertex,
    host: ProvokingVertex,
) -> Result<Option<Rewritten>, String> {
    let count = trimmed(mode, count);
    if count == 0 {
        return Ok(None);
    }
    let (mut primitive, mut size, mut out) = (MESA_PRIM_POINTS, 0, 0);
    let mut generate = None;
    // SAFETY: Mesa's generator fills the four values it is given.
    let mode_of = unsafe {
        u_index_generator(
            mask(if lists_only { LISTS } else { STRIPS_AND_LISTS }),
            mode,
            first,
            count,
            convention(api),
            convention(host),
            &mut primitive,
            &mut size,
            &mut out,
            &mut generate,
        )
    };
    let generate = generate
        .filter(|_| mode_of != U_TRANSLATE_ERROR)
        .ok_or("Mesa's index generator has none for the draw")?;
    let bytes = out as usize * size as usize;
    let mut words = words_for(bytes);
    // SAFETY: the generator writes `out` indices of `size` bytes, which the words hold.
    unsafe { generate(first, out, words.as_mut_ptr().cast::<c_void>()) };
    Ok(Some(Rewritten {
        primitive,
        format: format(size),
        words,
        bytes,
    }))
}

/// The draw's `indices` (its whole range, of `info`'s index size) as indices that draw them as
/// OpenGL does on the host: cut at each restart value where `split_restart`, as lists only where
/// `lists_only` (the host's strips restart at the largest value), and with the host's provoking
/// vertex `host` where OpenGL's is `api`.
pub fn translated(
    info: &pipe_draw_info,
    indices: &[u8],
    split_restart: bool,
    lists_only: bool,
    api: ProvokingVertex,
    host: ProvokingVertex,
) -> Result<Option<Rewritten>, String> {
    let index_size = c_uint::from(info.index_size);
    let count = (indices.len() / index_size as usize) as u32;
    let mode = mesa_prim::from(info.mode);
    // Mesa's functions read the indices as their own width.
    let aligned;
    let indices = if indices.as_ptr().cast::<u32>().is_aligned() {
        indices
    } else {
        let mut words = words_for(indices.len());
        // SAFETY: the words hold at least the indices' bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(
                indices.as_ptr(),
                words.as_mut_ptr().cast::<u8>(),
                indices.len(),
            );
        }
        aligned = words;
        // SAFETY: as above.
        unsafe { std::slice::from_raw_parts(aligned.as_ptr().cast::<u8>(), indices.len()) }
    };
    let pieces = if split_restart {
        restart_pieces(info, indices, count)?
    } else {
        vec![(0, trimmed(mode, count))]
    };
    let primitives = mask(if lists_only { LISTS } else { STRIPS_AND_LISTS });
    let mut converted = Vec::with_capacity(pieces.len());
    let mut bytes = 0;
    let (mut primitive, mut size) = (MESA_PRIM_POINTS, 0);
    for (start, count) in pieces.into_iter().filter(|&(_, count)| count > 0) {
        let mut out = 0;
        let mut translate = None;
        // SAFETY: Mesa's translator fills the four values it is given.
        let mode_of = unsafe {
            u_index_translator(
                primitives,
                mode,
                index_size,
                count,
                convention(api),
                convention(host),
                PR_DISABLE,
                &mut primitive,
                &mut size,
                &mut out,
                &mut translate,
            )
        };
        let translate = translate
            .filter(|_| mode_of != U_TRANSLATE_ERROR)
            .ok_or("Mesa's index translator has none for the draw")?;
        converted.push((start, count, out, translate));
        bytes += out as usize * size as usize;
    }
    if bytes == 0 {
        return Ok(None);
    }
    let mut words = words_for(bytes);
    let mut at = 0;
    for (start, count, out, translate) in converted {
        // SAFETY: the translator reads `count` indices from `start` of the indices and writes `out`
        // indices of `size` bytes at `at`, which the words hold.
        unsafe {
            translate(
                indices.as_ptr().cast::<c_void>(),
                start,
                count,
                out,
                info.restart_index,
                words.as_mut_ptr().cast::<u8>().add(at).cast::<c_void>(),
            );
        }
        at += out as usize * size as usize;
    }
    Ok(Some(Rewritten {
        primitive,
        format: format(size),
        words,
        bytes,
    }))
}

unsafe extern "C" {
    /// The C runtime's, which Mesa's restart splitter allocates its ranges from.
    fn free(memory: *mut c_void);
}

/// The ranges of the `count` indices between restart values, each less an unfinished last
/// primitive, as (first index, count).
fn restart_pieces(
    info: &pipe_draw_info,
    indices: &[u8],
    count: u32,
) -> Result<Vec<(u32, u32)>, String> {
    let whole = pipe_draw_start_count_bias {
        start: 0,
        count,
        index_bias: 0,
    };
    let (mut pieces, mut lowest, mut highest, mut total) = (0, 0, 0, 0);
    // SAFETY: the splitter reads `count` indices of the draw's size and returns its own array of
    // `pieces` ranges, or none.
    let ranges = unsafe {
        util_prim_restart_convert_to_direct(
            indices.as_ptr().cast::<c_void>(),
            info,
            &whole,
            &mut pieces,
            &mut lowest,
            &mut highest,
            &mut total,
        )
    };
    if ranges.is_null() {
        // Nothing whole between the restart values.
        return Ok(Vec::new());
    }
    // SAFETY: the splitter's array of `pieces` ranges, freed after it is read.
    let found = unsafe { std::slice::from_raw_parts(ranges, pieces as usize) }
        .iter()
        .map(|r| (r.start, r.count))
        .collect();
    // SAFETY: as above.
    unsafe { free(ranges.cast::<c_void>()) };
    Ok(found)
}
