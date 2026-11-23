//! The built-in clear program (any machine): every storage format and dimension gives a module that
//! validates and whose WGSL reads back, and the values it stores.

use wcontainers_gpu::{Format, ViewDimension};
use wcontainers_shaders::ConvertError;
use wcontainers_shaders::builtin::{
    clear_groups, clear_params, storage_clear, storage_clear_value,
};

/// Every format, by its number.
fn formats() -> Vec<Format> {
    use serde::Deserialize;
    (0..)
        .map_while(|n| {
            Format::deserialize(
                serde::de::value::U32Deserializer::<serde::de::value::Error>::new(n),
            )
            .ok()
        })
        .collect()
}

#[test]
fn every_storage_format_and_dimension_gives_a_clear_program() {
    let mut made = 0;
    for format in formats() {
        for dimension in [ViewDimension::D2, ViewDimension::D2Array, ViewDimension::D3] {
            match storage_clear(format, dimension) {
                Ok(m) => {
                    let wgsl = naga::back::wgsl::write_string(
                        &m.module,
                        &m.info,
                        naga::back::wgsl::WriterFlags::empty(),
                    )
                    .unwrap();
                    naga::front::wgsl::parse_str(&wgsl).unwrap();
                    made += 1;
                }
                Err(e) => assert_eq!(
                    e,
                    ConvertError::Unsupported(wcontainers_shaders::refusal::STORAGE_FORMAT_ABSENT)
                ),
            }
        }
        assert_eq!(
            storage_clear(format, ViewDimension::Cube).err(),
            Some(ConvertError::Unsupported(
                if storage_clear(format, ViewDimension::D2).is_ok() {
                    wcontainers_shaders::refusal::CLEAR_DIMENSION
                } else {
                    wcontainers_shaders::refusal::STORAGE_FORMAT_ABSENT
                }
            )),
        );
    }
    // The 40 formats WGSL has a storage form of and `A8Unorm`, three dimensions each.
    assert_eq!(made, 123);
}

#[test]
fn integer_clears_store_each_components_own_bits() {
    let v = |f, values, float| storage_clear_value(f, values, float);
    // Normalised codes as the floats a store scales back to every code.
    for bits in [8u32, 16] {
        let f = if bits == 8 {
            Format::R8Unorm
        } else {
            Format::R16Unorm
        };
        let max = (1u32 << bits) - 1;
        for c in 0..=max {
            let x = f32::from_bits(v(f, [c | 0xabc0_0000, 0, 0, 0], false).unwrap()[0]);
            assert_eq!((x * max as f32).round() as u32, c, "{f:?} {c}");
        }
        let f = if bits == 8 {
            Format::R8Snorm
        } else {
            Format::R16Snorm
        };
        let half = 1u32 << (bits - 1);
        for c in 0..=max {
            let signed = ((c << (32 - bits)) as i32) >> (32 - bits);
            let r = v(f, [c, 0, 0, 0], false);
            if c == half {
                assert_eq!(
                    r,
                    Err(ConvertError::Unsupported(
                        wcontainers_shaders::refusal::SNORM_MOST_NEGATIVE
                    ))
                );
                continue;
            }
            let x = f32::from_bits(r.unwrap()[0]);
            assert_eq!((x * (half - 1) as f32).round() as i32, signed, "{f:?} {c}");
        }
    }
    // 10 and 2 bits.
    let w = v(Format::Rgb10a2Unorm, [1023, 512, 0x1_0001, 2], false).unwrap();
    assert_eq!(
        w.map(f32::from_bits),
        [1.0, 512.0 / 1023.0, 1.0 / 1023.0, 2.0 / 3.0]
    );
    // Integers: low bits, sign-extended for signed formats.
    assert_eq!(
        v(Format::Rgba8Uint, [0x1ff, 7, 0x100, 255], false),
        Ok([0xff, 7, 0, 255])
    );
    assert_eq!(
        v(Format::Rgba16Sint, [0xffff, 0x8000, 0x1_7fff, 1], false),
        Ok([u32::MAX, (-32768i32) as u32, 0x7fff, 1])
    );
    assert_eq!(
        v(Format::Rg32Sint, [5, u32::MAX, 0, 0], false),
        Ok([5, u32::MAX, 0, 0])
    );
    // Floats from their own bits: halves, 11- and 10-bit floats (blue, a 10-bit denormal), and 32-bit
    // ones unchanged.
    let h = v(Format::Rgba16Float, [0x3c00, 0xfc00, 0x0001, 0x8000], false).unwrap();
    assert_eq!(
        h,
        [1.0f32, f32::NEG_INFINITY, 2f32.powi(-24), -0.0].map(f32::to_bits)
    );
    let s = v(Format::Rg11b10Float, [0x3c0, 0x7c0, 0x1f, 0x123], false).unwrap();
    assert_eq!(
        &s[..3],
        &[1.0f32, f32::INFINITY, 31.0 * 2f32.powi(-19)].map(f32::to_bits)
    );
    assert_eq!(
        v(Format::R32Float, [0x7fc0_1234, 0, 0, 0], false),
        Ok([0x7fc0_1234, 0, 0, 0])
    );
    // `A8Unorm`: its one code from the alpha value, zeros in the channels it lacks.
    assert_eq!(
        v(Format::A8Unorm, [7, 9, 11, 0x1_0033], false),
        Ok([0, 0, 0, ((51.0f64 / 255.0) as f32).to_bits()])
    );
}

#[test]
fn float_clears_pass_the_floats_and_refuse_integer_formats() {
    let values = [1.5f32, -2.0, 0.25, f32::NAN].map(f32::to_bits);
    for f in [
        Format::Rgba8Unorm,
        Format::Rgba16Float,
        Format::Rgba8Snorm,
        Format::Rg11b10Float,
    ] {
        assert_eq!(storage_clear_value(f, values, true), Ok(values));
    }
    assert_eq!(
        storage_clear_value(Format::A8Unorm, values, true),
        Ok([0, 0, 0, values[3]])
    );
    // Denormals become zeros of their sign.
    let denormal = [1e-40f32, -1e-40, 2e-38, 0.0].map(f32::to_bits);
    assert_eq!(
        storage_clear_value(Format::R32Float, denormal, true),
        Ok([0, 0x8000_0000, 2e-38f32.to_bits(), 0])
    );
    for f in [Format::Rgba8Uint, Format::R32Sint] {
        assert_eq!(
            storage_clear_value(f, values, true),
            Err(ConvertError::Unsupported(
                wcontainers_shaders::refusal::FLOAT_CLEAR_OF_INTEGER
            ))
        );
    }
    assert_eq!(
        storage_clear_value(Format::Rgba8UnormSrgb, values, true),
        Err(ConvertError::Unsupported(
            wcontainers_shaders::refusal::STORAGE_FORMAT_ABSENT
        ))
    );
}

#[test]
fn params_and_groups() {
    let p = clear_params([1, 2, 3, 4], [5, 6, 7], [8, 9, 10]);
    let words: Vec<u32> = p
        .chunks(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    assert_eq!(words, [1, 2, 3, 4, 5, 6, 7, 0, 8, 9, 10, 0]);
    assert_eq!(clear_groups([0, 0, 0], [17, 8, 3]), [3, 1, 3]);
    assert_eq!(clear_groups([4, 4, 2], [5, 12, 3]), [1, 1, 1]);
}

/// The pixel program of a pipeline without a pixel shader: clamped depth from the linked value and
/// the depth range, clip distances discarding, the counting output; nothing at all without a variant.
#[test]
fn a_pixel_program_is_made_for_a_variant_alone() {
    use wcontainers_gpu::{HelperValue, LinkedValues, StageExtras, VariantFlags, VariantKey};
    use wcontainers_shaders::builtin::pixel_program_for_variant;
    let wgsl = |c: &wcontainers_shaders::Converted| {
        let m = &c.modules[0];
        naga::back::wgsl::write_string(&m.module, &m.info, naga::back::wgsl::WriterFlags::empty())
            .unwrap()
    };
    let linked = LinkedValues {
        location: 3,
        clip_distances: 2,
    };
    let key = VariantKey {
        flags: VariantFlags::DEPTH_CLAMP | VariantFlags::CLIP_DISTANCE,
        linked,
        samples: 1,
        ..Default::default()
    };
    let made = pixel_program_for_variant(&key).unwrap();
    let text = wgsl(&made);
    assert!(text.contains("@builtin(frag_depth)"), "{text}");
    assert!(text.contains("@location(3)"), "{text}");
    assert!(text.contains("discard"), "{text}");
    assert_eq!(made.reflection.helpers.len(), 1);
    assert_eq!(made.reflection.helpers[0].value, HelperValue::DepthRange);
    assert!(matches!(
        made.reflection.extras,
        StageExtras::Pixel { discards: true, .. }
    ));
    let counting = VariantKey {
        flags: VariantFlags::COUNTING,
        ..Default::default()
    };
    let text = wgsl(&pixel_program_for_variant(&counting).unwrap());
    assert!(text.contains("@location(0)"), "{text}");
    let plain = pixel_program_for_variant(&VariantKey::default()).unwrap();
    assert!(!wgsl(&plain).contains("@location"));
    assert_ne!(plain.modules[0].key, made.modules[0].key);
    // What is not built for any program is refused here too.
    let provoking = VariantKey {
        flags: VariantFlags::PROVOKING_LAST,
        ..Default::default()
    };
    assert_eq!(
        pixel_program_for_variant(&provoking).err(),
        Some(ConvertError::Unsupported(
            wcontainers_shaders::refusal::PROVOKING_REWRITE
        ))
    );
}
