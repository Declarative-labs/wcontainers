//! Methods of the types [`crate::reflect`] declares.

use crate::reflect::*;
use crate::shader::ShaderStage;
use alloc::vec::Vec;

impl ShaderStage {
    /// The binding in [`HELPER_SET`] of the uniform buffer holding this stage's helper constants.
    ///
    /// Each stage
    /// of a program has its own block, laid out by that stage's own [`Reflection::helpers`], so the vertex
    /// and fragment stages of one draw never read each other's values; the front writes both blocks.
    pub const fn helper_binding(self) -> u32 {
        match self {
            ShaderStage::Vertex | ShaderStage::Compute => 0,
            ShaderStage::Fragment => 12,
        }
    }
}

impl TargetCombineValue {
    pub fn new(
        target: &crate::state::RenderTargetBlend,
        logic_op: Option<crate::state::LogicOp>,
        constant: [f32; BLEND_CONSTANT_COMPONENTS],
    ) -> TargetCombineValue {
        use crate::{state::BlendComponent, state::BlendFactor, state::BlendOp};
        let off = BlendComponent {
            src: BlendFactor::One,
            dst: BlendFactor::Zero,
            op: BlendOp::Add,
        };
        let (color, alpha) = target.blend.unwrap_or((off, off));
        let codes = |c: BlendComponent| [c.src as u32, c.dst as u32, c.op as u32];
        TargetCombineValue {
            color: codes(color),
            alpha: codes(alpha),
            writes: u32::from(target.writes.bits()),
            logic_op: logic_op.unwrap_or(crate::state::LogicOp::Copy) as u32,
            constant,
        }
    }

    /// The value as the helper block holds it, little-endian.
    pub fn bytes(&self) -> Vec<u8> {
        let words = self
            .color
            .iter()
            .chain(&self.alpha)
            .chain([&self.writes, &self.logic_op]);
        words
            .copied()
            .chain(self.constant.map(f32::to_bits))
            .flat_map(u32::to_le_bytes)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_ranges_follow_each_other_without_overlap() {
        let kinds = [
            SlotKind::Uniform,
            SlotKind::Sampler,
            SlotKind::Texture,
            SlotKind::Storage,
            SlotKind::Counter,
        ];
        for pair in kinds.windows(2) {
            assert_eq!(
                crate::reflect::SLOT_KINDS[pair[0] as usize].first_binding
                    + crate::reflect::SLOT_KINDS[pair[0] as usize].slots,
                crate::reflect::SLOT_KINDS[pair[1] as usize].first_binding
            );
        }
        assert_eq!(
            (127 < crate::reflect::SLOT_KINDS[SlotKind::Texture as usize].slots).then(|| {
                crate::reflect::SLOT_KINDS[SlotKind::Texture as usize].first_binding + 127
            }),
            Some(159)
        );
        assert_eq!(
            (128 < crate::reflect::SLOT_KINDS[SlotKind::Texture as usize].slots).then(|| {
                crate::reflect::SLOT_KINDS[SlotKind::Texture as usize].first_binding + 128
            }),
            None
        );
        assert_eq!(
            (63 < crate::reflect::SLOT_KINDS[SlotKind::Storage as usize].slots)
                .then(|| crate::reflect::SLOT_KINDS[SlotKind::Storage as usize].first_binding + 63),
            Some(223)
        );
        assert_eq!(
            (0 < crate::reflect::SLOT_KINDS[SlotKind::Counter as usize].slots)
                .then(|| crate::reflect::SLOT_KINDS[SlotKind::Counter as usize].first_binding),
            Some(224)
        );
    }

    #[test]
    fn sampler_state_codes_keep_their_numbers() {
        use crate::{resource::AddressMode, resource::Filter};
        assert_eq!([Filter::Nearest, Filter::Linear].map(|f| f as u32), [0, 1]);
        let modes = [
            AddressMode::Repeat,
            AddressMode::Mirror,
            AddressMode::Clamp,
            AddressMode::Border,
            AddressMode::MirrorOnce,
        ];
        assert_eq!(modes.map(|m| m as u32), [0, 1, 2, 3, 4]);
        use crate::resource::CompareFn as C;
        let compares = [
            C::Never,
            C::Less,
            C::Equal,
            C::LessEqual,
            C::Greater,
            C::NotEqual,
            C::GreaterEqual,
            C::Always,
        ];
        assert_eq!(compares.map(|c| c as u32), [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn a_bindings_slot_is_its_place_in_its_kinds_range() {
        let kinds = [
            SlotKind::Uniform,
            SlotKind::Sampler,
            SlotKind::Texture,
            SlotKind::Storage,
            SlotKind::Counter,
        ];
        for kind in kinds {
            for slot in 0..crate::reflect::SLOT_KINDS[kind as usize].slots {
                let b = ReflectedBinding {
                    kind,
                    space: 0,
                    register: 0,
                    count: 1,
                    set: 1,
                    binding: (slot < crate::reflect::SLOT_KINDS[kind as usize].slots)
                        .then(|| crate::reflect::SLOT_KINDS[kind as usize].first_binding + slot)
                        .unwrap(),
                    added: None,
                    stride: 0,
                };
                assert_eq!(
                    b.binding - crate::reflect::SLOT_KINDS[b.kind as usize].first_binding,
                    slot,
                    "{kind:?}"
                );
            }
            assert_eq!(
                ((crate::reflect::SLOT_KINDS[kind as usize].slots)
                    < crate::reflect::SLOT_KINDS[kind as usize].slots)
                    .then(|| crate::reflect::SLOT_KINDS[kind as usize].first_binding
                        + (crate::reflect::SLOT_KINDS[kind as usize].slots)),
                None,
                "{kind:?}"
            );
        }
        // The ranges follow one another, and a counter has the number of its storage slot.
        for pair in kinds.windows(2) {
            assert_eq!(
                crate::reflect::SLOT_KINDS[pair[0] as usize].first_binding
                    + crate::reflect::SLOT_KINDS[pair[0] as usize].slots,
                crate::reflect::SLOT_KINDS[pair[1] as usize].first_binding,
                "{pair:?}"
            );
        }
        assert_eq!(
            crate::reflect::SLOT_KINDS[SlotKind::Counter as usize].slots,
            crate::reflect::SLOT_KINDS[SlotKind::Storage as usize].slots
        );
    }

    /// A target's combine value is laid out in the helper block as its type is.
    #[test]
    fn target_combine_values_are_written_as_laid_out() {
        let target = crate::state::RenderTargetBlend {
            blend: None,
            writes: crate::state::ColorWrites::R | crate::state::ColorWrites::A,
        };
        let value = TargetCombineValue::new(&target, None, [0.5; 4]);
        let bytes = value.bytes();
        assert_eq!(bytes.len(), core::mem::size_of::<TargetCombineValue>());
        let word = |offset: usize| {
            u32::from_le_bytes(
                bytes[offset..][..core::mem::size_of::<u32>()]
                    .try_into()
                    .unwrap(),
            )
        };
        use core::mem::offset_of;
        assert_eq!(word(offset_of!(TargetCombineValue, writes)), value.writes);
        assert_eq!(
            word(offset_of!(TargetCombineValue, logic_op)),
            crate::state::LogicOp::Copy as u32
        );
        assert_eq!(
            word(offset_of!(TargetCombineValue, alpha)),
            crate::state::BlendFactor::One as u32
        );
        assert_eq!(
            word(offset_of!(TargetCombineValue, constant)),
            0.5f32.to_bits()
        );
    }

    /// Every binding of the helper set, each a single one or a range from its base (the clamped
    /// samplers' open-ended), meets no other.
    #[test]
    fn helper_set_bindings_never_meet() {
        let pairs = u32::MAX - CLAMPED_SAMPLER_BINDING;
        let ranges = [
            (ShaderStage::Vertex.helper_binding(), 1),
            (ShaderStage::Fragment.helper_binding(), 1),
            (TARGET_COPY_BINDING, crate::MAX_COLOR_TARGETS as u32),
            (EARLY_DEPTH_BINDING, 1),
            (EARLY_STENCIL_BINDING, 1),
            (DRAW_ARGUMENTS_BINDING, 1),
            (DOMAIN_PATCHES_BINDING, 1),
            (DOMAIN_POINTS_BINDING, 1),
            (COUNTER_BINDING, 1),
            (CLAMPED_SAMPLER_BINDING, pairs),
        ];
        assert_eq!(ShaderStage::Compute.helper_binding(), ranges[0].0);
        for (i, &(a, n)) in ranges.iter().enumerate() {
            for &(b, m) in &ranges[i + 1..] {
                assert!(a + n <= b || b + m <= a, "{a}+{n} meets {b}+{m}");
            }
        }
    }
}
