//! What a DXIL shader declares that a pipeline needs before converting it: its typed unordered
//! access views and typed buffers, and its multisampled array textures (as `dxbc` reads them from
//! DXBC), from the views of the module's resource list, which the shaders crate reads.

use crate::dxbc::{Kind, TypedView};
use std::ops::Range;
use wcontainers_gpu::ScalarKind;
use wcontainers_shaders::container::{DxilShape, DxilView, DxilViews, RANGE_WITHOUT_SIZE};

/// A view's elements: its space, first register and element count.
pub(crate) type Elements = (u32, u32, u32);

/// The elements of each of a list's views, and the list's places for the slots; `None` where a
/// range runs past the last register.
fn places(list: &[DxilView]) -> Option<(Vec<Elements>, Places)> {
    let each = list
        .iter()
        .map(|v| {
            // An unbounded range is one place: the converter refuses such arrays.
            let count = match v.count {
                RANGE_WITHOUT_SIZE => 1,
                count => count,
            };
            if let Some(last) = count.checked_sub(1) {
                v.register.checked_add(last)?;
            }
            Some((v.space, v.register, count))
        })
        .collect::<Option<Vec<_>>>()?;
    let all = Places::of(each.iter().copied());
    Some((each, all))
}

/// The typed views of one list: shader resource views (`storage` false, typed buffers only) or
/// unordered access views (textures and typed buffers), with a slot per element of an array, as
/// the converter declares a view per element.
fn typed(list: &[DxilView], storage: bool, out: &mut Vec<TypedView>) -> Option<()> {
    let (each, all) = places(list)?;
    for (v, elements) in list.iter().zip(each) {
        let typed = match storage {
            true => v.shape != DxilShape::Other,
            false => v.shape == DxilShape::TypedBuffer,
        };
        if !typed {
            continue;
        }
        let kind = match v.value? {
            ScalarKind::Float => Kind::Float,
            ScalarKind::Uint => Kind::Uint,
            ScalarKind::Sint => Kind::Sint,
        };
        let slots = all.slots(elements);
        if !slots.is_empty() {
            out.push(TypedView {
                storage,
                buffer: v.shape == DxilShape::TypedBuffer,
                slots,
                kind,
            });
        }
    }
    Some(())
}

/// A class's (space, register) places under Direct3D 12's numbering, which numbers its views in
/// (space, register) order, each place once: the runs of consecutive places its views take, each
/// with the count of places before it. Two numbers per declared range however long it is, as a
/// driver keeps a range.
pub(crate) struct Places {
    /// (space, first register, end register, places before), sorted, not overlapping or touching.
    runs: Vec<(u32, u64, u64, u64)>,
}

impl Places {
    /// The places of views' `elements`.
    pub(crate) fn of(elements: impl IntoIterator<Item = Elements>) -> Places {
        let mut sorted: Vec<(u32, u64, u64)> = elements
            .into_iter()
            .filter(|e| e.2 != 0)
            .map(|(space, register, count)| {
                let start = u64::from(register);
                (space, start, start + u64::from(count))
            })
            .collect();
        sorted.sort_unstable();
        let mut runs: Vec<(u32, u64, u64, u64)> = Vec::with_capacity(sorted.len());
        for (space, start, end) in sorted {
            match runs.last_mut() {
                Some(last) if last.0 == space && start <= last.2 => last.2 = last.2.max(end),
                last => {
                    let before = last.map_or(0, |l| l.3 + (l.2 - l.1));
                    runs.push((space, start, end, before));
                }
            }
        }
        Places { runs }
    }

    /// The slot of the view at (`space`, `register`): the count of the class's places before it,
    /// as the converter numbers them.
    pub(crate) fn slot(&self, space: u32, register: u32) -> u32 {
        let register = u64::from(register);
        let at = self
            .runs
            .partition_point(|r| (r.0, r.1) <= (space, register));
        let Some(&(s, start, end, before)) = at.checked_sub(1).and_then(|i| self.runs.get(i))
        else {
            return 0;
        };
        let within = match s == space {
            true => register.min(end) - start,
            false => end - start,
        };
        (before + within) as u32
    }

    /// The slots of a view's elements, consecutive: its places are all the class's.
    pub(crate) fn slots(&self, (space, register, count): Elements) -> Range<u32> {
        let first = self.slot(space, register);
        first..first.saturating_add(count)
    }
}

/// What `declared` gives for a container.
pub type Declared = (Option<Vec<TypedView>>, Option<Vec<Range<u32>>>);

/// The typed unordered access views and typed buffers `code` declares (shader resource views of
/// textures left out), and the texture slots of its multisampled array textures
/// (`Texture2DMSArray`, a range of slots each), from its module's views (the process's cache keeps them); each `None`
/// where `code` holds no DXIL module or the lists it is read from are malformed.
pub fn declared(code: &[u8]) -> Declared {
    of(crate::cache::dxil_views(code))
}

/// Whether a module of views `lists` declares a shader resource or unordered access view array
/// without a length, which takes the rest of the root signature's range
/// (`VariantKey::register_ranges`).
pub(crate) fn declares_unbounded_array(lists: &DxilViews) -> bool {
    [&lists.views, &lists.storage]
        .into_iter()
        .flatten()
        .flatten()
        .any(|v| v.count == RANGE_WITHOUT_SIZE)
}

/// `declared`, from the views of a module.
pub(crate) fn of(lists: Option<DxilViews>) -> Declared {
    let Some(lists) = lists else {
        return (None, None);
    };
    let views = (|| {
        let mut out = Vec::new();
        typed(lists.views.as_deref()?, false, &mut out)?;
        typed(lists.storage.as_deref()?, true, &mut out)?;
        Some(out)
    })();
    let multisampled = lists.views.as_deref().and_then(|list| {
        let (each, all) = places(list)?;
        let arrays = list.iter().zip(each);
        let arrays = arrays.filter(|(v, _)| v.shape == DxilShape::MultisampledArray);
        let slots = arrays.map(|(_, e)| all.slots(e)).filter(|s| !s.is_empty());
        Some(slots.collect())
    });
    (views, multisampled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(storage: bool, buffer: bool, slot: u32, kind: Kind) -> TypedView {
        TypedView {
            storage,
            buffer,
            slots: one(slot),
            kind,
        }
    }

    /// The slots of one element at `slot`.
    fn one(slot: u32) -> Range<u32> {
        slot..slot + 1
    }

    #[test]
    fn typed_buffers_are_read_from_the_resource_list() {
        // dxc's cs_6_0 of the shaders crate's texels.hlsl: Buffer<float4> t0, Buffer<uint4> t1,
        // Buffer<int4> t2, Buffer<float2> t3, RWBuffer<float4> u0, RWBuffer<uint> u1 and a
        // structured buffer at u2 (left out).
        let code = include_bytes!("../../wcontainers-shaders/tests/own/cs_texels.dxil");
        let mut views = declared(code).0.unwrap();
        views.sort_by_key(|v| (v.storage, v.slots.start));
        assert_eq!(
            views,
            [
                view(false, true, 0, Kind::Float),
                view(false, true, 1, Kind::Uint),
                view(false, true, 2, Kind::Sint),
                view(false, true, 3, Kind::Float),
                view(true, true, 0, Kind::Float),
                view(true, true, 1, Kind::Uint),
            ]
        );
    }

    #[test]
    fn storage_textures_are_read_from_the_resource_list() {
        // compute.hlsl's cs_images: RWTexture2D<float4> u0, RWTexture2D<float> u1,
        // RWTexture2DArray<uint4> u2, RWTexture2D<int2> u3.
        let code = include_bytes!("../../wcontainers-shaders/tests/own/cs_images.dxil");
        let mut views = declared(code).0.unwrap();
        views.sort_by_key(|v| v.slots.start);
        assert_eq!(
            views,
            [
                view(true, false, 0, Kind::Float),
                view(true, false, 1, Kind::Float),
                view(true, false, 2, Kind::Uint),
                view(true, false, 3, Kind::Sint),
            ]
        );
    }

    #[test]
    fn arrays_of_storage_textures_are_a_slot_per_element() {
        // arrays.hlsl's cs_image_array: RWTexture2D<uint> images[4] at u0.
        let code = include_bytes!("../../wcontainers-shaders/tests/own/cs_image_array.dxil");
        let views = declared(code).0.unwrap();
        let elements = TypedView {
            slots: 0..4,
            ..view(true, false, 0, Kind::Uint)
        };
        assert_eq!(views, [elements]);
    }

    #[test]
    fn multisampled_arrays_are_read_from_the_resource_list() {
        // msarray.hlsl's ps_msarray: Texture2DMSArray<float4> t0.
        let code = include_bytes!("../../wcontainers-shaders/tests/own/ps_msarray.dxil");
        assert_eq!(declared(code).1, Some(vec![one(0)]));
        let code = include_bytes!("../../wcontainers-shaders/tests/own/cs_texels.dxil");
        assert_eq!(declared(code).1, Some(vec![]));
    }

    #[test]
    fn slots_follow_space_then_register() {
        // u5 and u0 in space 1, u7 in space 0, u5 again in space 1 (a view and its counter).
        let all = Places::of([(1, 5, 1), (1, 0, 1), (0, 7, 1), (1, 5, 1)]);
        let slots: Vec<u32> = [(0, 7), (1, 0), (1, 5)]
            .map(|p| all.slot(p.0, p.1))
            .to_vec();
        assert_eq!(slots, [0, 1, 2]);
        // Ranges overlapping and touching in space 0 (u2 to u9), one in space 1 (u0 to u3): a slot
        // per place, each place once.
        let all = Places::of([(0, 4, 6), (0, 2, 3), (1, 0, 4), (0, 0, 0)]);
        assert_eq!(all.slots((0, 2, 3)), 0..3);
        assert_eq!(all.slots((0, 4, 6)), 2..8);
        assert_eq!(all.slots((1, 0, 4)), 8..12);
        assert_eq!(all.slot(0, 100), 8);
        assert_eq!(all.slot(2, 0), 12);
    }

    #[test]
    fn a_container_read_again_declares_what_its_module_does() {
        let code = include_bytes!("../../wcontainers-shaders/tests/own/cs_texels.dxil");
        let first = declared(code);
        assert_eq!(declared(code), first);
        assert_eq!(first, of(wcontainers_shaders::container::dxil_views(code)));
        // Its first half is another container, read on its own.
        let mut cut = code.to_vec();
        cut.truncate(code.len() / 2);
        assert_eq!(declared(&cut), (None, None));
    }

    #[test]
    fn dxbc_has_no_resource_list() {
        let code = include_bytes!("../../wcontainers-shaders/tests/own/cs_images.dxbc");
        assert_eq!(declared(code), (None, None));
    }
}
