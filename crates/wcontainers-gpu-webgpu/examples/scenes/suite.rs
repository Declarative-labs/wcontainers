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

impl wcontainers_gpu_test::Host for Chrome {
    type Backend = WebGpu;

    fn make() -> WebGpu {
        let instance = draw::instance(wgpu::Backends::BROWSER_WEBGPU);
        let (b, _) = block_on(draw::device_on(&instance)).expect("a WebGPU adapter");
        INSTANCE.set(Some(instance));
        b
    }

    fn window(b: &mut WebGpu) -> u32 {
        const WINDOW: u32 = 1;
        let surface = INSTANCE.with_borrow(|i| {
            let instance = i.as_ref().expect("make() first");
            instance.create_surface(wgpu::SurfaceTarget::Canvas(make_canvas()))
        });
        b.add_window(WINDOW, surface.expect("a surface"));
        WINDOW
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

/// Every suite test, by name.
macro_rules! suite {
    ($($test:ident),* $(,)?) => {
        const SUITE: &[(&str, fn())] = &[
            $((stringify!($test), wcontainers_gpu_test::suite::$test::<Chrome>),)*
            (
                "present_shows_the_frame",
                wcontainers_gpu_test::suite::present_shows_the_frame::<Chrome>,
            ),
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
