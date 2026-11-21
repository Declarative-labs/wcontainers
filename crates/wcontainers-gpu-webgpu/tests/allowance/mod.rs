//! The rig's allowance for pictures drawn on any GPU against Direct3D 11's references, shared by
//! the tests that draw the rig's scenes.

/// How far a picture may differ from Direct3D 11's reference on any GPU: the largest 8-bit channel
/// difference and how many pixels may exceed it, the allowance WARP earns
/// (references/d3d11/tolerances.txt). On the GPU the references were drawn on, the back end's
/// pictures are equal to them in every pixel (web/check compares them at 0).
pub fn allowance(picture: &str) -> (u8, usize) {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../wcontainers-graphics-test/references/d3d11/tolerances.txt"
    ))
    .expect("the rig's tolerances");
    text.lines()
        .find_map(|line| {
            let mut words = line.split_whitespace();
            if words.next()? != picture {
                return None;
            }
            let tolerance: f64 = words.next()?.parse().ok()?;
            Some((tolerance as u8, words.next()?.parse().ok()?))
        })
        .expect("an allowance for the picture")
}

/// Pixels of `a` differing from `b` by more than `tolerance` in some 8-bit channel, and the largest
/// difference.
pub fn beyond(a: &[u8], b: &[u8], tolerance: u8) -> (usize, u8) {
    let mut worst = 0;
    let mut beyond = 0;
    for (p, q) in a.chunks(4).zip(b.chunks(4)) {
        let d = p
            .iter()
            .zip(q)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0);
        worst = worst.max(d);
        beyond += usize::from(d > tolerance);
    }
    (beyond, worst)
}

/// Whether `drawn` is within the rig's allowance of the reference `picture` (`reference`), printing
/// how far it is.
pub fn within(drawn: &[u8], reference: &[u8], picture: &str) -> bool {
    let (tolerance, pixels) = allowance(picture);
    let (over, worst) = beyond(drawn, reference, tolerance);
    let differ = beyond(drawn, reference, 0).0;
    eprintln!(
        "{picture}: {differ} pixels differ, {over} by more than {tolerance} (allowed {pixels}), largest difference {worst}"
    );
    over <= pixels
}
