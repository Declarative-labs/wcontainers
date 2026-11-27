//! The OpenGL ES conformance suite's tests that need no EGL (`ON_ANY_CONTEXT`), built for WebAssembly and run on a
//! canvas's WebGL 2 context in a browser page: browser/suite.mjs opens browser/suite.html once a test. Also the cost
//! of a query read as soon as it ended, which waits for the browser's turns (`suite_measure`).
#![cfg(target_arch = "wasm32")]

use std::cell::{Cell, RefCell};

use wasm_bindgen::prelude::*;
use wcontainers_opengl_es::OpenGlEs;
use wcontainers_opengl_es::constants as gl;
use wcontainers_opengl_es_conformance::{ON_ANY_CONTEXT, run_on_any_context};
use wcontainers_opengl_es_webgl::WebGl;

#[wasm_bindgen]
extern "C" {
    /// A WebGL 2 context on a new canvas of 4 by 4 pixels, made by the page.
    #[wasm_bindgen(js_namespace = window, js_name = makeContext)]
    fn make_context() -> web_sys::WebGl2RenderingContext;
    /// Hands the page a panic's message before the module traps.
    #[wasm_bindgen(js_namespace = window)]
    fn panicked(message: &str);
    /// The page's clock, in milliseconds.
    #[wasm_bindgen(js_namespace = performance)]
    fn now() -> f64;
}

thread_local! {
    /// The test `suite_test` chose, which `suite_run` runs.
    static CHOSEN: RefCell<String> = const { RefCell::new(String::new()) };
    /// The queries `suite_measure` reads.
    static QUERIES: Cell<u32> = const { Cell::new(0) };
    /// What the last measurement found, which `suite_report` hands the page.
    static REPORT: RefCell<String> = const { RefCell::new(String::new()) };
}

/// The tests a page may choose, separated by spaces.
#[wasm_bindgen]
pub fn suite_names() -> String {
    ON_ANY_CONTEXT.join(" ")
}

/// Chooses the test `name` for `suite_run`; an error for a name the suite has not.
#[wasm_bindgen]
pub fn suite_test(name: &str) -> Result<(), JsError> {
    std::panic::set_hook(Box::new(|info| panicked(&info.to_string())));
    if !ON_ANY_CONTEXT.contains(&name) {
        return Err(JsError::new(&format!("the suite has no test {name} that takes a context made without EGL")));
    }
    CHOSEN.set(name.to_string());
    Ok(())
}

/// Runs the chosen test on new WebGL 2 contexts. It takes and returns nothing, so its export is the plain WebAssembly
/// function, which the page calls through `WebAssembly.promising` so that a wait for a query's result can suspend it.
/// A failing test panics: the page gets the message through `window.panicked` and the module traps.
#[wasm_bindgen]
pub fn suite_run() {
    let name = CHOSEN.take();
    let ran = run_on_any_context(&name, &|| WebGl::new(make_context(), &[]), None);
    assert!(ran, "no test {name}");
}

/// Chooses how many queries `suite_measure` reads.
#[wasm_bindgen]
pub fn suite_measure_queries(queries: u32) {
    std::panic::set_hook(Box::new(|info| panicked(&info.to_string())));
    QUERIES.set(queries);
}

/// Measures a query read as soon as it ended against the same draw with no query: the milliseconds each takes and
/// the browser's turns each wait took. Called through `WebAssembly.promising`, as `suite_run`; `suite_report` gives
/// the result.
#[wasm_bindgen]
pub fn suite_measure() {
    let queries = QUERIES.get().max(1);
    let context = WebGl::new(make_context(), &[]);
    let vertex = shader(&context, gl::VERTEX_SHADER, "#version 300 es\nlayout(location = 0) in vec2 at;\nvoid main() { gl_Position = vec4(at, 0.0, 1.0); }\n");
    let fragment = shader(&context, gl::FRAGMENT_SHADER, "#version 300 es\nprecision mediump float;\nout vec4 drawn;\nvoid main() { drawn = vec4(1.0); }\n");
    let program = context.create_program();
    context.attach_shader(program, vertex);
    context.attach_shader(program, fragment);
    context.link_program(program);
    context.use_program(program);
    let mut buffers = [0_u32];
    context.gen_buffers(&mut buffers);
    context.bind_buffer(gl::ARRAY_BUFFER, buffers[0]);
    let triangle: Vec<u8> = [-1.0_f32, -1.0, 3.0, -1.0, -1.0, 3.0].iter().flat_map(|value| value.to_ne_bytes()).collect();
    context.buffer_data(gl::ARRAY_BUFFER, i64::try_from(triangle.len()).unwrap_or(0), Some(&triangle), gl::STATIC_DRAW);
    context.enable_vertex_attrib_array(0);
    context.vertex_attrib_pointer(0, 2, gl::FLOAT, false, 0, 0);
    let mut names = [0_u32];
    context.gen_queries(&mut names);
    let mut pixel = [0_u8; 4];
    // The same draw, read back so that it is finished, with no query.
    let start = now();
    for _ in 0..queries {
        context.draw_arrays(gl::TRIANGLES, 0, 3);
        context.read_pixels(0, 0, 1, 1, gl::RGBA, gl::UNSIGNED_BYTE, wcontainers_opengl_es::PixelPack::Bytes(&mut pixel));
    }
    let plain = (now() - start) / f64::from(queries);
    let turns_before = context.turns();
    let mut passed = 0_u32;
    let start = now();
    for _ in 0..queries {
        context.begin_query(gl::ANY_SAMPLES_PASSED, names[0]);
        context.draw_arrays(gl::TRIANGLES, 0, 3);
        context.end_query(gl::ANY_SAMPLES_PASSED);
        let mut result = [0_u32];
        context.get_query_objectuiv(names[0], gl::QUERY_RESULT, &mut result);
        passed += result[0];
    }
    let waited = (now() - start) / f64::from(queries);
    let turns = context.turns() - turns_before;
    assert_eq!(context.get_error(), gl::NO_ERROR);
    REPORT.set(format!(
        "queries {queries}, each passed {passed}; a draw read back {plain:.3} ms, a draw in a query read back as soon as it ended {waited:.3} ms; turns {turns} ({:.2} a query)",
        f64::from(u32::try_from(turns).unwrap_or(u32::MAX)) / f64::from(queries)
    ));
}

/// What the last `suite_measure` found.
#[wasm_bindgen]
pub fn suite_report() -> String {
    REPORT.take()
}

/// A compiled shader of the source.
fn shader(context: &WebGl, kind: u32, source: &str) -> u32 {
    let shader = context.create_shader(kind);
    context.shader_source(shader, &[source.as_bytes()]);
    context.compile_shader(shader);
    shader
}
