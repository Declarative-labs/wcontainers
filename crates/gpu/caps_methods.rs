//! Methods of the types [`crate::caps`] declares.

use crate::caps::*;
use crate::format::Format;

impl FormatCaps {
    /// Whether a texture of `format` may have `count` samples.
    pub fn takes_samples(&self, format: Format, count: u32) -> bool {
        count.is_power_of_two()
            && count <= 128
            && self.samples[format as usize] & (count as u8) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_counts_are_per_format() {
        let mut caps = FormatCaps {
            uses: [FormatUse::empty(); crate::format::FORMAT_COUNT],
            samples: [0b1; crate::format::FORMAT_COUNT],
        };
        caps.samples[Format::Rgba8Unorm as usize] = 0b1101;
        assert!(caps.takes_samples(Format::Rgba8Unorm, 1));
        assert!(caps.takes_samples(Format::Rgba8Unorm, 4));
        assert!(caps.takes_samples(Format::Rgba8Unorm, 8));
        assert!(!caps.takes_samples(Format::Rgba8Unorm, 2));
        assert!(!caps.takes_samples(Format::Rgba8Unorm, 3));
        assert!(!caps.takes_samples(Format::Rgba8Unorm, 256));
        assert!(!caps.takes_samples(Format::R32Float, 4));
    }
}
