//! Methods of the types [`crate::format`] declares.

use crate::format::*;

impl Format {
    /// The bits of `Depth24Plus`'s depth as a back end stores it: a 24-bit unorm word.
    pub const UNORM24_BITS: u32 = 24;

    /// The largest 24-bit unorm depth word, the one that stands for 1.0.
    pub const UNORM24_MAX: u32 = (1 << Self::UNORM24_BITS) - 1;

    /// The bytes one sample of colour targets in these formats holds, in attachment order, as
    /// [`crate::caps::Limits::color_bytes_per_sample`] bounds it: each target starts at a multiple of its
    /// alignment and adds its cost ([`FormatFacts::target_bytes`]). None where one cannot be a colour
    /// target.
    pub fn color_target_bytes(formats: impl IntoIterator<Item = Format>) -> Option<u32> {
        formats.into_iter().try_fold(0, |at: u32, f| {
            let (cost, align) = FORMAT_FACTS[f as usize].target_bytes?;
            Some(at.next_multiple_of(align) + cost)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, vec::Vec};
    use serde::Deserialize;

    /// Every format, by its number.
    fn all() -> Vec<Format> {
        (0..)
            .map_while(|n| {
                Format::deserialize(
                    serde::de::value::U32Deserializer::<serde::de::value::Error>::new(n),
                )
                .ok()
            })
            .collect()
    }

    /// A format's name lists the channels its texels hold, as DXGI's and WebGPU's names do
    /// (`x` padding, `e` a shared exponent; ASTC blocks hold all four); `components` names the same.
    #[test]
    fn components_are_the_channels_a_format_names() {
        let formats = all();
        assert_eq!(formats.last(), Some(&Format::Astc4x4UnormSrgb));
        for f in formats {
            let name = format!("{f:?}");
            let expected = if FORMAT_FACTS[f as usize].depth_stencil {
                0b0001
            } else if name.starts_with("Astc") {
                0b1111
            } else {
                let rest = ["Bc1", "Bc2", "Bc3", "Bc4", "Bc5", "Bc6h", "Bc7", "Etc2"]
                    .iter()
                    .find_map(|p| name.strip_prefix(p))
                    .unwrap_or(&name);
                let end = rest[1..]
                    .find(|c: char| c.is_ascii_uppercase())
                    .map_or(rest.len(), |i| i + 1);
                rest[..end]
                    .to_ascii_lowercase()
                    .chars()
                    .map(|c| match c {
                        'r' => 0b0001,
                        'g' => 0b0010,
                        'b' => 0b0100,
                        'a' => 0b1000,
                        _ => 0,
                    })
                    .fold(0, |m, b| m | b)
            };
            assert_eq!(FORMAT_FACTS[f as usize].components, expected, "{f:?}");
        }
    }
}
