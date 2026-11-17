//! Swap chains, presenting and the adapter description, checked on the recording back end.

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
    chain.set_max_latency(1).unwrap();
    assert!(chain.set_max_latency(0).is_err());
    for _ in 0..3 {
        chain.present(&mut b, 1).unwrap();
    }
    // The recording back end finishes every fence at once, so nothing stays queued.
    assert_eq!(chain.frames_queued(&mut b), 0);
    assert!(!chain.would_wait(&mut b));
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
    assert_eq!(chain.surface_format(), Format::Rgba16Float);
    assert!(b.named("create").iter().any(|c| c.contains("\"encode\"")));
    assert!(
        b.named("configure_surface")
            .last()
            .unwrap()
            .contains("Rgba16Float 128 32")
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
    assert_eq!(present_format(b.caps(), Format::Rgb10a2Unorm), Format::Rgba16Float);
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
