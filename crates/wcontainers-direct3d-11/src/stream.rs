//! Stream output declarations (`CreateGeometryShaderWithStreamOutput`), checked against the
//! shader's output signature as Direct3D checks them.

use wcontainers_shaders::stage::{StreamOutput, StreamOutputEntry};

use crate::desc::level;
use crate::error::{Error, Result};
use crate::shader::SignatureElement;

/// One `D3D11_SO_DECLARATION_ENTRY`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclEntry {
    pub stream: u32,
    /// `None` for a gap.
    pub semantic: Option<String>,
    pub semantic_index: u32,
    pub start_component: u8,
    pub component_count: u8,
    pub slot: u8,
}

/// A declaration as `CreateGeometryShaderWithStreamOutput` gives it: the entries and the strides,
/// each `None` where the program gave no array, and the rasterized stream.
pub type Declaration<'a> = (Option<&'a [DeclEntry]>, Option<&'a [u32]>, u32);

/// `D3D11_SO_NO_RASTERIZED_STREAM`.
pub const NO_RASTERIZED_STREAM: u32 = u32::MAX;
/// The largest stride of a stream output buffer.
const MAX_STRIDE: u32 = 2048;

const INVALID: Error = Error::InvalidArg("a stream output declaration Direct3D refuses");

/// The declaration a shader made with `entries` (none: the program gave no array) and `strides`
/// (none: no array) streams, and whether stream 0 is drawn, at feature `level`. `outputs` is the
/// shader's output signature; `vertex` says the bytecode is a vertex shader's, which needs a
/// declaration.
pub fn declaration(
    entries: Option<&[DeclEntry]>,
    strides: Option<&[u32]>,
    rasterized: u32,
    device_level: u32,
    outputs: &[SignatureElement],
    vertex: bool,
) -> Result<(Option<StreamOutput>, bool)> {
    let eleven = device_level >= level::L11_0;
    let drawn = match rasterized {
        0 => true,
        1..=3 | NO_RASTERIZED_STREAM if eleven => false,
        _ => return Err(INVALID),
    };
    let strides = strides.filter(|s| !s.is_empty());
    if strides.is_some_and(|s| s.len() > 4 || (!eleven && s.len() > 1)) {
        return Err(INVALID);
    }
    let Some(entries) = entries else {
        // A geometry shader without stream output; a vertex shader needs a declaration.
        if vertex || (!eleven && strides.is_some()) {
            return Err(INVALID);
        }
        return Ok((None, drawn));
    };
    if entries.is_empty() {
        return Err(INVALID);
    }
    // Components each buffer takes, and the entries of each.
    let mut components = [0u32; 4];
    let mut named = [0u32; 4];
    let mut counts = [0u32; 4];
    let mut last_slot = None;
    let mut closed = [false; 4];
    // The output components written, per stream, register and component.
    let mut written: Vec<(u32, u32, u8)> = Vec::new();
    let mut out = Vec::with_capacity(entries.len());
    for e in entries {
        let slot = usize::from(e.slot);
        if e.stream > 3 || (!eleven && e.stream != 0) || slot > 3 || e.component_count == 0 {
            return Err(INVALID);
        }
        // Each buffer's entries come together.
        if last_slot != Some(slot) {
            if closed[slot] {
                return Err(INVALID);
            }
            if let Some(l) = last_slot {
                closed[l] = true;
            }
            last_slot = Some(slot);
        }
        match &e.semantic {
            None => {
                if e.semantic_index != 0 || e.start_component != 0 {
                    return Err(INVALID);
                }
            }
            Some(name) => {
                let o = outputs
                    .iter()
                    .find(|o| {
                        o.stream == e.stream
                            && o.index == e.semantic_index
                            && o.semantic.eq_ignore_ascii_case(name)
                    })
                    .ok_or(INVALID)?;
                let width = o.mask.count_ones();
                let (start, count) = (u32::from(e.start_component), u32::from(e.component_count));
                if start + count > width {
                    return Err(INVALID);
                }
                let first = o.mask.trailing_zeros() + start;
                for c in first..first + count {
                    let at = (e.stream, o.register, c as u8);
                    if written.contains(&at) {
                        return Err(INVALID);
                    }
                    written.push(at);
                }
                named[slot] += 1;
            }
        }
        components[slot] += u32::from(e.component_count);
        counts[slot] += 1;
        out.push(StreamOutputEntry {
            stream: e.stream as u8,
            semantic: e.semantic.clone(),
            semantic_index: e.semantic_index,
            start_component: e.start_component,
            component_count: e.component_count,
            slot: e.slot,
        });
    }
    let used: Vec<usize> = (0..4).filter(|&b| counts[b] > 0).collect();
    // A buffer holding gaps alone is refused; at 10_x several buffers hold one value each.
    if used.iter().any(|&b| named[b] == 0)
        || (!eleven && used.len() > 1 && used.iter().any(|&b| counts[b] != 1))
    {
        return Err(INVALID);
    }
    let mut chosen = [0u32; 4];
    if let Some(s) = strides {
        for &b in &used {
            let stride = *s.get(b).ok_or(INVALID)?;
            if stride == 0
                || !stride.is_multiple_of(4)
                || stride > MAX_STRIDE
                || stride < 4 * components[b]
            {
                return Err(INVALID);
            }
            chosen[b] = stride;
        }
    } else if used.iter().any(|&b| 4 * components[b] > MAX_STRIDE) {
        return Err(INVALID);
    }
    Ok((
        Some(StreamOutput {
            entries: out,
            strides: chosen,
        }),
        drawn,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The output signature of Wine's test_stream_output geometry shader: SV_Position, ATTRIB1
    /// (four), attrib2 (three), ATTriB3 (two, xy of register 3), ATTRIB4 (one, z of register 3).
    fn outputs() -> Vec<SignatureElement> {
        let e = |semantic: &str, index, register, mask| SignatureElement {
            semantic: semantic.into(),
            index,
            register,
            system: 0,
            mask,
            stream: 0,
        };
        vec![
            e("SV_Position", 0, 0, 0xf),
            e("ATTRIB", 1, 1, 0xf),
            e("attrib", 2, 2, 0x7),
            e("ATTriB", 3, 3, 0x3),
            e("ATTRIB", 4, 3, 0x4),
        ]
    }

    fn entry(semantic: Option<&str>, index: u32, start: u8, count: u8, slot: u8) -> DeclEntry {
        DeclEntry {
            stream: 0,
            semantic: semantic.map(Into::into),
            semantic_index: index,
            start_component: start,
            component_count: count,
            slot,
        }
    }

    fn check(entries: &[DeclEntry], strides: Option<&[u32]>, at: u32) -> bool {
        declaration(Some(entries), strides, 0, at, &outputs(), false).is_ok()
    }

    #[test]
    fn declarations_are_checked_against_the_signature() {
        let at = level::L11_0;
        let s = [64u32; 4];
        assert!(check(&[entry(Some("sv_position"), 0, 0, 4, 0)], None, at));
        assert!(check(
            &[entry(Some("ATTRIB"), 4, 0, 1, 0)],
            Some(&s[..1]),
            at
        ));
        assert!(check(&[entry(Some("ATTRIB"), 3, 1, 1, 0)], None, at));
        assert!(!check(&[entry(Some("ATTRIB"), 3, 2, 1, 0)], None, at));
        assert!(!check(&[entry(Some("ATTRIB"), 2, 0, 4, 0)], None, at));
        assert!(!check(&[entry(Some("ATTRIB_"), 1, 0, 4, 0)], None, at));
        // Gaps: no start or index, a count, and never a buffer of gaps alone.
        let p = entry(Some("SV_POSITION"), 0, 0, 4, 0);
        let a = entry(Some("ATTRIB"), 1, 0, 4, 0);
        assert!(check(
            &[p.clone(), entry(None, 0, 0, 8, 0), a.clone()],
            Some(&s),
            at
        ));
        assert!(!check(
            &[p.clone(), entry(None, 0, 1, 8, 0), a.clone()],
            None,
            at
        ));
        assert!(!check(&[p.clone(), entry(None, 0, 0, 0, 0)], None, at));
        assert!(!check(&[a.clone(), entry(None, 0, 0, 4, 1)], None, at));
        // The stride holds the buffer's components.
        let gap = entry(None, 0, 0, 8, 0);
        assert!(!check(
            &[p.clone(), gap.clone(), gap, a.clone()],
            Some(&s),
            at
        ));
        assert!(!check(std::slice::from_ref(&p), Some(&[63]), at));
        assert!(!check(std::slice::from_ref(&p), Some(&[0]), at));
        assert!(check(std::slice::from_ref(&p), Some(&[64, 1]), at));
        // Each component once; each buffer's entries together.
        let half = entry(Some("ATTRIB"), 1, 2, 2, 1);
        assert!(check(&[entry(Some("ATTRIB"), 1, 0, 2, 0), half], None, at));
        assert!(!check(
            &[a.clone(), entry(Some("ATTRIB"), 1, 0, 3, 0)],
            None,
            at
        ));
        let b = entry(Some("ATTRIB"), 2, 0, 3, 1);
        let c = entry(Some("ATTRIB"), 3, 0, 2, 0);
        assert!(!check(&[a.clone(), b.clone(), c], None, at));
        // At 10_0 several buffers hold one value each.
        let ten = level::L10_0;
        assert!(check(&[a.clone(), b.clone()], None, ten));
        assert!(!check(
            &[a.clone(), entry(Some("ATTRIB"), 4, 0, 1, 0), b],
            None,
            ten
        ));
        assert!(!check(std::slice::from_ref(&p), Some(&s[..2]), ten));
        // Streams and the rasterized stream.
        let streamed = DeclEntry { stream: 1, ..a };
        assert!(!check(&[streamed], None, at));
        let o = outputs();
        let one = [p];
        assert!(declaration(Some(&one), None, 3, at, &o, false).is_ok_and(|d| !d.1));
        assert!(declaration(Some(&one), None, 4, at, &o, false).is_err());
        assert!(declaration(Some(&one), None, NO_RASTERIZED_STREAM, ten, &o, false).is_err());
        assert!(declaration(None, None, 0, at, &o, true).is_err());
        assert!(declaration(None, Some(&s[..1]), 0, at, &o, false).is_ok());
        assert!(declaration(Some(&[]), None, 0, at, &o, false).is_err());
    }
}
