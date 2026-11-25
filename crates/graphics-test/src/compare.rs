//! How far two pictures of the same size and depth differ, at that depth.

use image::{ColorType, DynamicImage};

/// The channel depth a picture is compared at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    Eight,
    Sixteen,
    Float,
}

impl Depth {
    pub fn of(image: &DynamicImage) -> anyhow::Result<Depth> {
        Ok(match image.color() {
            ColorType::L8 | ColorType::La8 | ColorType::Rgb8 | ColorType::Rgba8 => Depth::Eight,
            ColorType::L16 | ColorType::La16 | ColorType::Rgb16 | ColorType::Rgba16 => {
                Depth::Sixteen
            }
            ColorType::Rgb32F | ColorType::Rgba32F => Depth::Float,
            other => anyhow::bail!("pictures of {other:?} are not compared"),
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Depth::Eight => "8-bit",
            Depth::Sixteen => "16-bit",
            Depth::Float => "float",
        }
    }
}

/// The side of the square tiles a picture's differences are counted in.
pub const TILE: u32 = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct Difference {
    pub depth: Depth,
    pub pixels: u64,
    /// Pixels where some channel differs by more than the tolerance.
    pub differing: u64,
    /// The largest difference of any channel of any pixel, alpha included, in the pictures' own
    /// units: steps of 1/255 or 1/65535, or the float value. Infinite where a NaN meets a number.
    pub largest: f64,
    /// The first differing pixel, x and y from the top left.
    pub first: Option<(u32, u32)>,
    /// Differing pixels in each `TILE` by `TILE` tile, row by row from the top left; tiles at the
    /// right and bottom edges may be smaller.
    pub tiles: Vec<u32>,
}

impl Difference {
    /// How many tiles make one row of the picture.
    pub fn tile_columns(width: u32) -> u32 {
        width.div_ceil(TILE)
    }
}

/// Compares two pictures channel by channel at their own depth; a pixel differs when any of its
/// channels differs by more than `tolerance`, in the same units as `Difference::largest`.
pub fn compare(a: &DynamicImage, b: &DynamicImage, tolerance: f64) -> anyhow::Result<Difference> {
    anyhow::ensure!(
        (a.width(), a.height()) == (b.width(), b.height()),
        "the pictures differ in size: {}x{} and {}x{}",
        a.width(),
        a.height(),
        b.width(),
        b.height()
    );
    let depth = Depth::of(a)?;
    let other = Depth::of(b)?;
    anyhow::ensure!(
        depth == other,
        "the pictures differ in depth: {} and {}",
        depth.name(),
        other.name()
    );
    let (ca, cb): (Vec<f64>, Vec<f64>) = match depth {
        Depth::Eight => (
            a.to_rgba8().into_raw().into_iter().map(f64::from).collect(),
            b.to_rgba8().into_raw().into_iter().map(f64::from).collect(),
        ),
        Depth::Sixteen => (
            a.to_rgba16()
                .into_raw()
                .into_iter()
                .map(f64::from)
                .collect(),
            b.to_rgba16()
                .into_raw()
                .into_iter()
                .map(f64::from)
                .collect(),
        ),
        Depth::Float => (
            a.to_rgba32f()
                .into_raw()
                .into_iter()
                .map(f64::from)
                .collect(),
            b.to_rgba32f()
                .into_raw()
                .into_iter()
                .map(f64::from)
                .collect(),
        ),
    };
    let columns = Difference::tile_columns(a.width());
    let mut difference = Difference {
        depth,
        pixels: 0,
        differing: 0,
        largest: 0.0,
        first: None,
        tiles: vec![0; (columns * a.height().div_ceil(TILE)) as usize],
    };
    for (i, (pa, pb)) in ca
        .as_chunks::<4>()
        .0
        .iter()
        .zip(cb.as_chunks::<4>().0)
        .enumerate()
    {
        let largest = pa
            .iter()
            .zip(pb)
            .map(|(&x, &y)| channel_difference(x, y))
            .fold(0.0, f64::max);
        difference.pixels += 1;
        difference.largest = difference.largest.max(largest);
        if largest > tolerance {
            difference.differing += 1;
            let i = i as u32;
            let (x, y) = (i % a.width(), i / a.width());
            difference.first.get_or_insert((x, y));
            difference.tiles[(y / TILE * columns + x / TILE) as usize] += 1;
        }
    }
    Ok(difference)
}

/// Equal values (two NaNs, or infinities of one sign) differ by 0; a NaN against anything else
/// differs infinitely.
fn channel_difference(x: f64, y: f64) -> f64 {
    if x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()) {
        0.0
    } else if x.is_nan() || y.is_nan() {
        f64::INFINITY
    } else {
        (x - y).abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba, Rgba32FImage, RgbaImage};

    #[test]
    fn counts_pixels_beyond_tolerance() {
        let a = RgbaImage::from_pixel(4, 3, Rgba([10, 20, 30, 255]));
        let mut b = a.clone();
        b.put_pixel(1, 2, Rgba([10, 23, 30, 255]));
        b.put_pixel(3, 0, Rgba([10, 20, 30, 254]));
        let (a, b) = (DynamicImage::from(a), DynamicImage::from(b));
        let d = compare(&a, &b, 0.0).unwrap();
        assert_eq!(
            (d.pixels, d.differing, d.largest, d.first),
            (12, 2, 3.0, Some((3, 0)))
        );
        let d = compare(&a, &b, 1.0).unwrap();
        assert_eq!((d.differing, d.largest, d.first), (1, 3.0, Some((1, 2))));
        let d = compare(&a, &b, 3.0).unwrap();
        assert_eq!((d.differing, d.first), (0, None));
    }

    #[test]
    fn counts_each_tile() {
        let a = RgbaImage::from_pixel(40, 20, Rgba([0, 0, 0, 255]));
        let mut b = a.clone();
        for (x, y) in [(0, 0), (15, 15), (16, 0), (39, 19), (39, 18)] {
            b.put_pixel(x, y, Rgba([9, 0, 0, 255]));
        }
        let d = compare(&a.into(), &b.into(), 0.0).unwrap();
        assert_eq!(d.tiles, vec![2, 1, 0, 0, 0, 2]);
    }

    #[test]
    fn refuses_different_sizes() {
        let a = DynamicImage::from(RgbaImage::new(2, 2));
        let b = DynamicImage::from(RgbaImage::new(2, 3));
        assert!(compare(&a, &b, 255.0).is_err());
    }

    #[test]
    fn refuses_different_depths() {
        let a = DynamicImage::from(RgbaImage::new(2, 2));
        let b = DynamicImage::ImageRgba16(ImageBuffer::new(2, 2));
        assert!(compare(&a, &b, 0.0).is_err());
    }

    #[test]
    fn sixteen_bit_low_bits_count() {
        let a: ImageBuffer<Rgba<u16>, _> = ImageBuffer::from_pixel(2, 2, Rgba([1000, 0, 0, 65535]));
        let mut b = a.clone();
        b.put_pixel(1, 1, Rgba([1100, 0, 0, 65535]));
        let d = compare(&a.into(), &b.into(), 0.0).unwrap();
        assert_eq!((d.differing, d.largest, d.first), (1, 100.0, Some((1, 1))));
    }

    #[test]
    fn float_differences_and_nan() {
        let a = Rgba32FImage::from_pixel(2, 1, Rgba([0.5, f32::NAN, 1.0, 1.0]));
        let mut b = a.clone();
        b.put_pixel(0, 0, Rgba([0.5 + 1e-7, f32::NAN, 1.0, 1.0]));
        let (fa, fb) = (DynamicImage::from(a.clone()), DynamicImage::from(b.clone()));
        let d = compare(&fa, &fb, 0.0).unwrap();
        assert_eq!((d.differing, d.first), (1, Some((0, 0))));
        assert!(d.largest > 0.0 && d.largest < 1e-6);
        b.put_pixel(1, 0, Rgba([0.5, 2.0, 1.0, 1.0]));
        let d = compare(&fa, &DynamicImage::from(b), 1.0).unwrap();
        assert_eq!((d.differing, d.largest), (1, f64::INFINITY));
    }
}
