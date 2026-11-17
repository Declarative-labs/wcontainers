//! How far two pictures of the same size differ.

use image::RgbaImage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Difference {
    pub pixels: u64,
    /// Pixels where some channel differs by more than the tolerance.
    pub differing: u64,
    /// The largest difference of any channel of any pixel, alpha included.
    pub largest: u8,
    /// The first differing pixel, x and y from the top left.
    pub first: Option<(u32, u32)>,
}

/// Compares two pictures channel by channel; a pixel differs when any of its channels differs by
/// more than `tolerance`.
pub fn compare(a: &RgbaImage, b: &RgbaImage, tolerance: u8) -> anyhow::Result<Difference> {
    anyhow::ensure!(
        a.dimensions() == b.dimensions(),
        "the pictures differ in size: {:?} and {:?}",
        a.dimensions(),
        b.dimensions()
    );
    let mut difference = Difference {
        pixels: 0,
        differing: 0,
        largest: 0,
        first: None,
    };
    for ((x, y, pa), pb) in a.enumerate_pixels().zip(b.pixels()) {
        let largest =
            pa.0.iter()
                .zip(pb.0)
                .map(|(&ca, cb)| ca.abs_diff(cb))
                .max()
                .unwrap_or(0);
        difference.pixels += 1;
        difference.largest = difference.largest.max(largest);
        if largest > tolerance {
            difference.differing += 1;
            difference.first.get_or_insert((x, y));
        }
    }
    Ok(difference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn counts_pixels_beyond_tolerance() {
        let a = RgbaImage::from_pixel(4, 3, Rgba([10, 20, 30, 255]));
        let mut b = a.clone();
        b.put_pixel(1, 2, Rgba([10, 23, 30, 255]));
        b.put_pixel(3, 0, Rgba([10, 20, 30, 254]));
        let d = compare(&a, &b, 0).unwrap();
        assert_eq!(
            (d.pixels, d.differing, d.largest, d.first),
            (12, 2, 3, Some((3, 0)))
        );
        let d = compare(&a, &b, 1).unwrap();
        assert_eq!((d.differing, d.largest, d.first), (1, 3, Some((1, 2))));
        let d = compare(&a, &b, 3).unwrap();
        assert_eq!((d.differing, d.first), (0, None));
    }

    #[test]
    fn refuses_different_sizes() {
        let a = RgbaImage::new(2, 2);
        let b = RgbaImage::new(2, 3);
        assert!(compare(&a, &b, 255).is_err());
    }
}
