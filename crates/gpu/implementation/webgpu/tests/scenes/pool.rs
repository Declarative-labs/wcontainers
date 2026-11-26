//! Textures written with `Discard` while recorded work uses them, renamed from a pool (plan 4.6).
//!
//! Counted where the including module can read the back end's counts (`made`). Shared by the
//! crate's unit tests and the browser page.

use super::draw;
use super::renames::{Kit, LOAD_WGSL, begin, draw_at, finish, kit, one_texel};
use wcontainers_gpu::{
    Gpu, Object, error::DeviceStatus, format::Format, resource::BindingEntry,
    resource::BindingResource, resource::TextureUsage, shader::StageSet, slow::SlowPath,
};
use wcontainers_gpu_webgpu::WebGpu;

/// Draws four frames of four draws, each loading a texture that a `Discard` write fills first.
///
/// The writes are Direct3D 11's `Map(WRITE_DISCARD)` of a dynamic texture: the writes
/// after a frame's first draw rename the texture, the first frame's renames make textures and later
/// frames' take the pool's, nothing splits the pass, and each pixel is its own write's red. Then a
/// `Discard` of the texture the open pass draws into cannot rename it: it splits the pass.
pub fn discarded_textures_are_renamed_from_the_pool(b: &mut WebGpu) {
    let (target, view) = draw::texture_2d(
        b,
        Format::Rgba8Unorm,
        4,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
    )
    .unwrap();
    let k = Kit {
        program: draw::program(b, LOAD_WGSL).unwrap(),
        target,
        view,
        ..kit(b, Format::Rgba8Unorm, 4, &draw::opaque())
    };
    let (texture, texture_view) = draw::texture_2d(
        b,
        Format::Rgba8Unorm,
        1,
        1,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
    )
    .unwrap();
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: 32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(&texture_view),
        }])
        .unwrap();
    let red = |frame: u32, i: u32| (frame * 4 + i) as u8 * 15 + 7;
    let mut seen = Vec::new();
    for frame in 0..4 {
        let before = super::made(b);
        begin(b, &k, [0.0, 1.0, 0.0, 1.0]);
        for i in 0..4 {
            one_texel(b, &texture, 0, [red(frame, i), 9, 9, 255]);
            draw_at(b, &k, &set, Some(i));
        }
        let got = finish(b, &k, 4);
        // The frame's work is done, so the storage it gave up is free for the next.
        let fence = b.submit();
        b.wait_fence(&fence);
        let want: Vec<u8> = (0..4).flat_map(|i| [red(frame, i), 0, 0, 255]).collect();
        assert_eq!(got, want, "frame {frame}");
        let after = super::made(b);
        if let (Some(renamed), Some(reused)) = (
            draw::since(after, before, |m| m.texture_renames),
            draw::since(after, before, |m| m.spares_reused),
        ) {
            seen.push((renamed, reused));
        }
    }
    eprintln!("texture renames and spares reused per frame: {seen:?}");
    if super::made(b).is_some() {
        assert_eq!(seen, [(3, 0), (3, 3), (3, 3), (3, 3)]);
    }
    assert_eq!(b.slow_paths().counts[SlowPath::WriteSplitPass as usize], 0);
    // The pass draws into the target: a discard of it is ordered in the pass instead.
    let before = super::made(b);
    begin(b, &k, [0.0, 1.0, 0.0, 1.0]);
    draw_at(b, &k, &set, Some(0));
    one_texel(b, &k.target, 3, [1, 2, 3, 4]);
    draw_at(b, &k, &set, Some(1));
    let got = finish(b, &k, 4);
    let last = red(3, 3);
    assert_eq!(
        got,
        [
            [last, 0, 0, 255],
            [last, 0, 0, 255],
            [0, 255, 0, 255],
            [1, 2, 3, 4]
        ]
        .concat()
    );
    let renamed = draw::since(super::made(b), before, |m| m.texture_renames);
    assert!(
        matches!(renamed, Some(0) | None),
        "the target the pass draws into was renamed: {renamed:?}"
    );
    assert_eq!(b.slow_paths().counts[SlowPath::WriteSplitPass as usize], 1);
    b.destroy(Object::Texture(texture));
    let fence = b.submit();
    b.wait_fence(&fence);
    b.submit();
    if let Some(made) = super::made(b) {
        assert_eq!(made.spares_dropped, 3, "{made:?}");
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}
