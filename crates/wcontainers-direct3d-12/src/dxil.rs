//! What a DXIL shader declares that a pipeline needs before converting it: its typed unordered
//! access views and typed buffers, and its multisampled array textures (as `dxbc` reads them from
//! DXBC), from the views of the module's resource list, which the shaders crate reads.

use crate::dxbc::{Kind, TypedView};
use wcontainers_gpu::ScalarKind;
use wcontainers_shaders::container::{DxilShape, DxilView, DxilViews, RANGE_WITHOUT_SIZE};

/// The (space, register) places of each of a list's views' elements, and every place of the list
/// sorted for the slots; `None` where a range runs past the last register.
#[allow(clippy::type_complexity)]
fn places(list: &[DxilView]) -> Option<(Vec<Vec<(u32, u32)>>, Vec<(u32, u32)>)> {
    let mut each = Vec::with_capacity(list.len());
    let mut places = Vec::with_capacity(list.len());
    for v in list {
        // An unbounded range is one place: the converter refuses such arrays.
        let count = match v.count {
            RANGE_WITHOUT_SIZE => 1,
            count => count,
        };
        let elements: Vec<(u32, u32)> = (0..count)
            .map(|i| Some((v.space, v.register.checked_add(i)?)))
            .collect::<Option<_>>()?;
        places.extend_from_slice(&elements);
        each.push(elements);
    }
    Some((each, sorted_places(&places)))
}

/// The typed views of one list: shader resource views (`storage` false, typed buffers only) or
/// unordered access views (textures and typed buffers), one per element of an array, as the
/// converter declares a view per element.
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
        out.extend(elements.into_iter().map(|place| TypedView {
            storage,
            buffer: v.shape == DxilShape::TypedBuffer,
            slot: slot(&all, place),
            kind,
        }));
    }
    Some(())
}

/// A class's (space, register) places, sorted, each once.
pub(crate) fn sorted_places(places: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut all = places.to_vec();
    all.sort_unstable();
    all.dedup();
    all
}

/// The slot of the view at `place` under Direct3D 12's numbering: its place among its class's
/// views in (space, register) order, as the converter numbers them.
pub(crate) fn slot(all: &[(u32, u32)], place: (u32, u32)) -> u32 {
    all.partition_point(|&p| p < place) as u32
}

/// What `declared` gives for a container.
pub type Declared = (Option<Vec<TypedView>>, Option<Vec<u32>>);

/// The typed unordered access views and typed buffers `code` declares (shader resource views of
/// textures left out), and the texture slots of its multisampled array textures
/// (`Texture2DMSArray`), from its module's views (the process's cache keeps them); each `None`
/// where `code` holds no DXIL module or the lists it is read from are malformed.
pub fn declared(code: &[u8]) -> Declared {
    of(crate::cache::dxil_views(code))
}

/// `declared`, from the views of a module.
fn of(lists: Option<DxilViews>) -> Declared {
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
        Some(arrays.flat_map(|(_, e)| e).map(|p| slot(&all, p)).collect())
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
            slot,
            kind,
        }
    }

    #[test]
    fn typed_buffers_are_read_from_the_resource_list() {
        // dxc's cs_6_0 of the shaders crate's texels.hlsl: Buffer<float4> t0, Buffer<uint4> t1,
        // Buffer<int4> t2, Buffer<float2> t3, RWBuffer<float4> u0, RWBuffer<uint> u1 and a
        // structured buffer at u2 (left out).
        let code = include_bytes!("../../wcontainers-shaders/tests/own/cs_texels.dxil");
        let mut views = declared(code).0.unwrap();
        views.sort_by_key(|v| (v.storage, v.slot));
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
        views.sort_by_key(|v| v.slot);
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
    fn arrays_of_storage_textures_are_a_view_per_element() {
        // arrays.hlsl's cs_image_array: RWTexture2D<uint> images[4] at u0.
        let code = include_bytes!("../../wcontainers-shaders/tests/own/cs_image_array.dxil");
        let mut views = declared(code).0.unwrap();
        views.sort_by_key(|v| v.slot);
        let elements: Vec<_> = (0..4).map(|s| view(true, false, s, Kind::Uint)).collect();
        assert_eq!(views, elements);
    }

    #[test]
    fn multisampled_arrays_are_read_from_the_resource_list() {
        // msarray.hlsl's ps_msarray: Texture2DMSArray<float4> t0.
        let code = include_bytes!("../../wcontainers-shaders/tests/own/ps_msarray.dxil");
        assert_eq!(declared(code).1, Some(vec![0]));
        let code = include_bytes!("../../wcontainers-shaders/tests/own/cs_texels.dxil");
        assert_eq!(declared(code).1, Some(vec![]));
    }

    #[test]
    fn slots_follow_space_then_register() {
        // u5 and u0 in space 1, u7 in space 0, u5 again in space 1 (a view and its counter).
        let all = sorted_places(&[(1, 5), (1, 0), (0, 7), (1, 5)]);
        let slots: Vec<u32> = [(0, 7), (1, 0), (1, 5)].map(|p| slot(&all, p)).to_vec();
        assert_eq!(slots, [0, 1, 2]);
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
