//! The shared back-end suite on the browser's WebGPU, one test per page (web/suite.html): each
//! device on the adapter a high-performance request gets, each window a new canvas.

use std::cell::RefCell;

use wasm_bindgen::prelude::*;
use wcontainers_gpu_webgpu::{WebGpu, block_on};

use super::draw;

#[wasm_bindgen]
extern "C" {
    /// A new canvas in the page.
    #[wasm_bindgen(js_namespace = window, js_name = makeCanvas)]
    fn make_canvas() -> web_sys::HtmlCanvasElement;
    /// What the last canvas shows, as a PNG file.
    #[wasm_bindgen(js_namespace = window, js_name = shownPng)]
    fn shown_png() -> Vec<u8>;
    /// Hands the page a panic's message before the module traps.
    #[wasm_bindgen(js_namespace = window)]
    fn panicked(message: &str);
}

thread_local! {
    /// The instance the test's device was made on, which its surfaces must come from too.
    static INSTANCE: RefCell<Option<wgpu::Instance>> = const { RefCell::new(None) };
}

struct Chrome;

impl Chrome {
    fn make() -> WebGpu {
        let instance = draw::instance(wgpu::Backends::BROWSER_WEBGPU);
        let (b, _) = block_on(draw::device_on(&instance)).expect("a WebGPU adapter");
        INSTANCE.set(Some(instance));
        b
    }

    fn window(b: &mut WebGpu) -> u32 {
        const CANVAS_WINDOW_HANDLE: u32 = 1;
        let surface = INSTANCE.with_borrow(|i| {
            let instance = i.as_ref().expect("make() first");
            instance.create_surface(wgpu::SurfaceTarget::Canvas(make_canvas()))
        });
        b.add_window(CANVAS_WINDOW_HANDLE, surface.expect("a surface"));
        CANVAS_WINDOW_HANDLE
    }

    fn shown(_: &mut WebGpu, _: u32) -> Vec<u8> {
        let png = shown_png();
        let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
        decoder.set_transformations(png::Transformations::ALPHA);
        let mut reader = decoder.read_info().expect("the canvas as a PNG file");
        let mut rgba = vec![0; reader.output_buffer_size().expect("its size")];
        let info = reader.next_frame(&mut rgba).expect("its texels");
        assert_eq!(
            (info.color_type, info.bit_depth),
            (png::ColorType::Rgba, png::BitDepth::Eight)
        );
        rgba.truncate(info.buffer_size());
        rgba
    }
}

impl Chrome {
    /// A device with a canvas registered as window 1, which the conformance suite presents into.
    fn make_presenting() -> WebGpu {
        let mut b = Self::make();
        Self::window(&mut b);
        b
    }
}

/// What is drawn into the surface's current texture is what the canvas shows, frame after frame.
fn present_shows_the_frame() {
    use wcontainers_gpu::{
        Gpu, draw::ColorAttachment, draw::LoadOp, draw::PassDesc, draw::StoreOp, format::Format,
    };
    let mut b = Chrome::make();
    let window = Chrome::window(&mut b);
    let surface = b.create_surface(window).expect("create_surface");
    b.configure_surface(&surface, Format::Rgba8Unorm, 4, 4, true)
        .expect("configure_surface");
    for (value, texel) in [
        ([0.2f32, 0.4, 0.6, 1.0], [51u8, 102, 153, 255]),
        ([1.0, 0.0, 0.2, 1.0], [255, 0, 51, 255]),
    ] {
        let v = b.current_texture(&surface).expect("current_texture");
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view: &v,
            depth_slice: 0,
            resolve: None,
            load: LoadOp::Clear(value.map(f32::to_bits)),
            store: StoreOp::Store,
        });
        b.begin_pass(&pass);
        b.end_pass();
        b.present(&surface).expect("present");
        assert_eq!(Chrome::shown(&mut b, window), texel.repeat(16));
    }
}

/// Every suite test, by name.
macro_rules! suite {
    ($($test:ident),* $(,)?) => {
        const SUITE: &[(&str, fn())] = &[
            $((stringify!($test), || wcontainers_gpu_conformance::$test(Chrome::make_presenting())),)*
            ("present_shows_the_frame", present_shows_the_frame),
        ];
    };
}

suite_but_present!(suite {});

/// Runs the suite test `name`. A failing test panics: the page gets the message through
/// `window.panicked` and the module traps, so a page runs one test.
#[wasm_bindgen(jspi)]
pub fn suite_test(name: &str) -> Result<(), JsError> {
    std::panic::set_hook(Box::new(|info| panicked(&info.to_string())));
    let (_, test) = SUITE
        .iter()
        .find(|(n, _)| *n == name)
        .ok_or_else(|| JsError::new(&format!("the browser host has no suite test {name}")))?;
    test();
    Ok(())
}
