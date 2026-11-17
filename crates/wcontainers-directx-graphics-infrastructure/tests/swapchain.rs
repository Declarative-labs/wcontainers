//! Swap chains, presenting, the adapter description and format support, checked on the recording
//! back end.

mod support {
    pub mod record;
}

use support::record::Recorder;
use wcontainers_directx_graphics_infrastructure::*;
use wcontainers_gpu::{Backend, Format, TextureUsage};

fn desc(format: u32, buffers: u32, model: Model) -> SwapChainDesc {
    SwapChainDesc {
        width: 64,
        height: 32,
        format,
        buffers,
        model,
        usage: TextureUsage::empty(),
    }
}

#[test]
fn flip_model_rotates_and_copies_each_frame() {
    let mut b = Recorder::new();
    let mut chain = SwapChain::new(&mut b, 7, &desc(28, 3, Model::FlipDiscard)).unwrap();
    assert!(chain.buffer(2).is_some() && chain.buffer(3).is_none());
    assert_eq!(chain.surface_format(), Format::Rgba8Unorm);
    let mut indices = Vec::new();
    for _ in 0..4 {
        indices.push(chain.current_index());
        chain.present(&mut b, 1).unwrap();
    }
    assert_eq!(indices, [0, 1, 2, 0]);
    assert_eq!(chain.present_count(), 4);
    assert_eq!(b.named("present ").len(), 4);
    assert_eq!(b.named("draw ").len(), 4);
    assert_eq!(b.named("begin_pass ").len(), 4);
    assert!(b.named("create").iter().any(|c| c.contains("\"copy\"")));
    // Each frame's copy is submitted before the surface is shown.
    let order: Vec<_> = b
        .calls
        .iter()
        .filter(|c| c.starts_with("submit") || c.starts_with("present"))
        .map(|c| c.split(' ').next().unwrap())
        .collect();
    assert_eq!(order, ["submit", "present"].repeat(4));
}

#[test]
fn blit_model_has_one_buffer_and_srgb_is_encoded() {
    let mut b = Recorder::new();
    let mut chain = SwapChain::new(&mut b, 1, &desc(29, 2, Model::Discard)).unwrap();
    assert!(chain.buffer(0).is_some() && chain.buffer(1).is_none());
    chain.present(&mut b, 0).unwrap();
    chain.present(&mut b, 0).unwrap();
    assert_eq!(chain.current_index(), 0);
    assert!(b.named("create").iter().any(|c| c.contains("\"encode\"")));
    // Sync interval 0 configures the surface without waiting for the display, once.
    assert_eq!(
        b.named("configure_surface")
            .iter()
            .filter(|c| c.ends_with("false"))
            .count(),
        1
    );
}

#[test]
fn descriptions_dxgi_refuses_are_refused() {
    let mut b = Recorder::new();
    for bad in [
        desc(29, 2, Model::FlipDiscard),
        desc(28, 1, Model::FlipSequential),
        desc(28, 17, Model::Discard),
        desc(2, 2, Model::FlipDiscard),
    ] {
        assert!(matches!(
            SwapChain::new(&mut b, 1, &bad),
            Err(SwapChainError::InvalidCall(_))
        ));
    }
}

#[test]
fn present_blocks_past_the_frame_latency() {
    let mut b = Recorder::new();
    let mut chain = SwapChain::new(&mut b, 1, &desc(87, 2, Model::FlipSequential)).unwrap();
    assert_eq!(chain.surface_format(), Format::Bgra8Unorm);
    assert!(chain.set_max_latency(0).is_err());
    chain.set_max_latency(2).unwrap();
    // The recorder's GPU finishes nothing until the test says so or a wait reaches it.
    chain.present(&mut b, 1).unwrap();
    chain.present(&mut b, 1).unwrap();
    assert_eq!(chain.frames_queued(&mut b), 2);
    assert!(chain.would_wait(&mut b));
    assert!(b.named("wait_fence").is_empty());
    // A third frame is one more than the latency: present waits for the first frame's work.
    chain.present(&mut b, 1).unwrap();
    assert_eq!(b.named("wait_fence"), ["wait_fence 1"]);
    assert_eq!(chain.frames_queued(&mut b), 2);
    b.finish();
    assert_eq!(chain.frames_queued(&mut b), 0);
    assert!(!chain.would_wait(&mut b));
    chain.present(&mut b, 1).unwrap();
    assert_eq!(b.named("wait_fence").len(), 1);
}

#[test]
fn resize_remakes_buffers_and_surface() {
    let mut b = Recorder::new();
    let mut chain = SwapChain::new(&mut b, 1, &desc(28, 2, Model::FlipDiscard)).unwrap();
    chain.present(&mut b, 1).unwrap();
    chain.resize(&mut b, 0, 128, 0, 10).unwrap();
    assert_eq!(chain.desc().width, 128);
    assert_eq!(chain.desc().height, 32);
    assert_eq!(chain.current_index(), 0);
    assert_eq!(chain.surface_format(), Format::Rgba8Unorm);
    assert!(b.named("create").iter().any(|c| c.contains("\"encode\"")));
    assert!(
        b.named("configure_surface")
            .last()
            .unwrap()
            .contains("Rgba8Unorm 128 32")
    );
}

#[test]
fn the_adapter_answers_from_caps() {
    let b = Recorder::new();
    let [adapter] = adapters(b.caps());
    assert_eq!(adapter.description, "recording back end");
    assert_eq!((adapter.vendor_id, adapter.device_id), (0x10de, 0x2206));
    assert!(!adapter.software);
    assert_eq!(adapter.description_utf16()[18], 0);
    assert_eq!(
        present_format(b.caps(), Format::Rgb10a2Unorm),
        Format::Rgba8Unorm
    );
    assert_eq!(
        present_format(b.caps(), Format::Rgba16Float),
        Format::Rgba8Unorm
    );
}

#[test]
fn outputs_offer_their_current_mode() {
    let output = OutputDesc {
        name: r"\\.\DISPLAY1".into(),
        desktop: [0, 0, 2560, 1440],
        refresh_numerator: 144000,
        refresh_denominator: 1000,
    };
    assert_eq!(output.modes(28).len(), 1);
    assert_eq!(output.modes(2).len(), 0);
    let mode = output.closest_mode(&DisplayMode {
        width: 800,
        height: 600,
        refresh_numerator: 60,
        refresh_denominator: 1,
        format: 0,
    });
    assert_eq!((mode.width, mode.height, mode.format), (2560, 1440, 28));
}

#[test]
fn format_support_follows_the_back_ends_format_uses() {
    use support1::*;
    let caps = Recorder::new().caps().clone();
    // R8G8B8A8_UNORM: a vertex format; sampled, rendered, blended, multisampled, resolved and
    // shown; no storage.
    let rgba = format_support(&caps, 28);
    assert_eq!(
        rgba.support1,
        IA_VERTEX_BUFFER
            | TEXTURE1D
            | TEXTURE2D
            | TEXTURE3D
            | TEXTURECUBE
            | MIP
            | SHADER_LOAD
            | SHADER_SAMPLE
            | SHADER_GATHER
            | RENDER_TARGET
            | BLENDABLE
            | MULTISAMPLE_RENDERTARGET
            | MULTISAMPLE_RESOLVE
            | DISPLAY
            | BACK_BUFFER_CAST
    );
    assert_eq!(rgba.support2, 0);
    // The typeless group: shapes only, no casts without FORMAT_REINTERPRET.
    assert_eq!(
        format_support(&caps, 27).support1,
        TEXTURE1D | TEXTURE2D | TEXTURE3D | TEXTURECUBE | MIP
    );
    // R32_UINT: an index and vertex format, a target without blending, stored into but not loaded.
    let r32 = format_support(&caps, 42);
    assert_eq!(
        r32.support1 & (IA_INDEX_BUFFER | IA_VERTEX_BUFFER | BLENDABLE | SHADER_SAMPLE),
        IA_INDEX_BUFFER | IA_VERTEX_BUFFER
    );
    assert_ne!(r32.support1 & TYPED_UNORDERED_ACCESS_VIEW, 0);
    assert_eq!(r32.support2, support2::UAV_TYPED_STORE);
    // Direct3D resolves no integer format and loads samples only of targets, whatever a back end
    // declares of the format.
    let mut generous = caps.clone();
    for f in [
        wcontainers_gpu::Format::R32Uint,
        wcontainers_gpu::Format::Bc1RgbaUnorm,
    ] {
        generous.formats.uses[f.index()] |=
            wcontainers_gpu::FormatUse::RESOLVE | wcontainers_gpu::FormatUse::SAMPLE;
        generous.formats.samples[f.index()] =
            generous.formats.samples[wcontainers_gpu::Format::Rgba8Unorm.index()];
    }
    assert_eq!(
        format_support(&generous, 42).support1 & MULTISAMPLE_RESOLVE,
        0
    );
    assert_eq!(format_support(&generous, 71).support1 & MULTISAMPLE_LOAD, 0);
    // R32_FLOAT views depth stored as D32_FLOAT, so it compares.
    assert_ne!(
        format_support(&caps, 41).support1 & SHADER_SAMPLE_COMPARISON,
        0
    );
    // D32_FLOAT is a depth target with no 3D shape; its R24 view of D24S8 loads and compares.
    assert_eq!(
        format_support(&caps, 40).support1,
        TEXTURE1D | TEXTURE2D | TEXTURECUBE | MIP | DEPTH_STENCIL | MULTISAMPLE_RENDERTARGET
    );
    assert_eq!(
        format_support(&caps, 46).support1,
        TEXTURE1D
            | TEXTURE2D
            | TEXTURECUBE
            | MIP
            | SHADER_LOAD
            | SHADER_SAMPLE_COMPARISON
            | SHADER_GATHER_COMPARISON
    );
    // Formats the back end lacks or the interface does not hold are refused.
    assert!(format_support(&caps, 65).is_empty());
    assert!(format_support(&caps, 66).is_empty());
    assert_eq!(format_support(&caps, 0).support1, BUFFER);
    // Sample counts: the recorder's are 1 and 4; depth views take one only.
    assert!(takes_samples(&caps, 28, 1) && takes_samples(&caps, 28, 4));
    assert!(!takes_samples(&caps, 28, 2) && !takes_samples(&caps, 65, 1));
    assert!(takes_samples(&caps, 27, 4) && takes_samples(&caps, 45, 4));
    assert!(!takes_samples(&caps, 46, 4) && takes_samples(&caps, 46, 1));
    assert_eq!(
        [0, 28, 20, 45, 47, 103, 66, 116].map(plane_count),
        [
            Some(1),
            Some(1),
            Some(2),
            Some(2),
            Some(2),
            Some(2),
            None,
            None
        ]
    );
}

#[test]
fn outputs_present_sdr_only() {
    // R8G8B8A8_UNORM and R16G16B16A16_FLOAT back buffers; G2084 P2020 is HDR10.
    assert!(presents_in(28, COLOR_SPACE_SRGB) && !presents_in(10, COLOR_SPACE_SRGB));
    assert!(presents_in(10, COLOR_SPACE_LINEAR) && !presents_in(28, COLOR_SPACE_LINEAR));
    assert!(!presents_in(24, 12) && !presents_in(10, 12));
}
