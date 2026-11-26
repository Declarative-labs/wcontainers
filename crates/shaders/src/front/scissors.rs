//! Scissors cut to the area every attachment of a fitted pass covers.

use alloc::vec::Vec;
use wcontainers_gpu::draw::ScissorRect;

/// Cuts `scissors` to `area` (from [`wcontainers_gpu::passes::PassTracker::before_draw`]), or gives `area`
/// itself for a draw without scissors.
pub fn cut_scissors(scissors: &[ScissorRect], area: ScissorRect, out: &mut Vec<ScissorRect>) {
    out.clear();
    if scissors.is_empty() {
        out.push(area);
        return;
    }
    for s in scissors {
        let x = s.x.max(area.x);
        let y = s.y.max(area.y);
        let right = s.x.saturating_add(s.width).min(area.x + area.width).max(x);
        let bottom =
            s.y.saturating_add(s.height)
                .min(area.y + area.height)
                .max(y);
        out.push(ScissorRect {
            x,
            y,
            width: right - x,
            height: bottom - y,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scissors_reaching_past_the_largest_offset_are_cut_to_the_area() {
        let area = ScissorRect {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        };
        let wide = ScissorRect {
            x: 1,
            y: 1,
            width: u32::MAX,
            height: u32::MAX,
        };
        let mut out = Vec::new();
        cut_scissors(&[wide], area, &mut out);
        assert_eq!(
            out,
            [ScissorRect {
                x: 1,
                y: 1,
                width: 3,
                height: 3
            }]
        );
    }
}
