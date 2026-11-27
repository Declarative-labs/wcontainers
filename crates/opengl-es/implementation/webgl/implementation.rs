//! OpenGL ES over a browser's WebGL 2 context: each method makes the WebGL 2 call of the same command through
//! web-sys, with the context's objects held under the numbers OpenGL ES names them by.
//!
//! WebGL 2 is OpenGL ES 3.0 made safe for a page's script: the browser checks every call, so nothing here reaches
//! memory and the crate holds no code the workspace forbids. What WebGL 2 does not answer the way OpenGL ES does is answered here: the
//! extension names and their count (WebGL's own names, of the extensions this context enabled), the version
//! strings (OpenGL ES's form, WebGL's text in brackets), the lengths of logs and sources, the sample counts of a
//! format, and the extension commands, called on the extension's object.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::ffi::CStr;
use std::sync::atomic::{AtomicU64, Ordering};

use js_sys::{Array, Float32Array, Function, Int8Array, Int16Array, Int32Array, Object, Reflect, Uint8Array, Uint16Array, Uint32Array};
use wasm_bindgen::{JsCast, JsValue};
use wcontainers_opengl_es::constants as gl;
use wcontainers_opengl_es::{CompressedPixelUnpack, OpenGlEs, PixelPack, PixelUnpack};
use web_sys::{
    WebGl2RenderingContext as Context, WebGlBuffer, WebGlFramebuffer, WebGlProgram, WebGlQuery, WebGlRenderbuffer, WebGlSampler,
    WebGlShader, WebGlSync, WebGlTexture, WebGlUniformLocation, WebGlVertexArrayObject,
};

/// WebGL 2's value for the longest wait `clientWaitSync` takes (WebGL 2 specification §5.15).
const MAX_CLIENT_WAIT_TIMEOUT_WEBGL: u32 = 0x9247;

/// WebGL's extension giving the timer queries of EXT_disjoint_timer_query to a WebGL 2 context.
const TIMER_QUERY: &str = "EXT_disjoint_timer_query_webgl2";
/// WebGL's extension giving the commands of OES_draw_buffers_indexed.
const DRAW_BUFFERS_INDEXED: &str = "OES_draw_buffers_indexed";
/// WebGL's extension giving EXT_clip_control's command.
const CLIP_CONTROL: &str = "EXT_clip_control";
/// WebGL's extension giving ANGLE_polygon_mode's command, as `polygonModeWEBGL`.
const POLYGON_MODE: &str = "WEBGL_polygon_mode";
/// WebGL's extension giving EXT_polygon_offset_clamp's command.
const POLYGON_OFFSET_CLAMP: &str = "EXT_polygon_offset_clamp";
/// WebGL's extension giving ANGLE_provoking_vertex's command, as `provokingVertexWEBGL`.
const PROVOKING_VERTEX: &str = "WEBGL_provoking_vertex";

/// The numbers the contexts of the process are told apart by, so a fence of one is refused by another.
static CONTEXTS: AtomicU64 = AtomicU64::new(0);

/// A fence of one context: its number among the context's fences.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Fence {
    context: u64,
    number: u64,
}

/// A context's objects of one kind, under the numbers handed out for them (0 is never one).
struct Table<T> {
    last: u32,
    held: BTreeMap<u32, T>,
}

impl<T> Default for Table<T> {
    fn default() -> Self {
        Self { last: 0, held: BTreeMap::new() }
    }
}

impl<T: Clone + AsRef<JsValue>> Table<T> {
    /// Holds the object under a new number; None when every number has been handed out.
    fn add(&mut self, object: T) -> Option<u32> {
        let number = self.last.checked_add(1)?;
        self.last = number;
        self.held.insert(number, object);
        Some(number)
    }

    fn get(&self, number: u32) -> Option<T> {
        self.held.get(&number).cloned()
    }

    /// The number of the object the value is, if this table holds it.
    fn number_of(&self, value: &JsValue) -> Option<u32> {
        self.held.iter().find(|(_, object)| (*object).as_ref() == value).map(|(number, _)| *number)
    }
}

/// A program or a shader: OpenGL ES names both from one set of numbers.
#[derive(Clone)]
enum Unit {
    Program(WebGlProgram),
    Shader(WebGlShader),
}

impl AsRef<JsValue> for Unit {
    fn as_ref(&self) -> &JsValue {
        match self {
            Self::Program(program) => program.as_ref(),
            Self::Shader(shader) => shader.as_ref(),
        }
    }
}

/// A uniform location handed out: the program it is of, and WebGL's location object.
struct Location {
    program: u32,
    location: WebGlUniformLocation,
}

#[derive(Default)]
struct Objects {
    buffers: Table<WebGlBuffer>,
    framebuffers: Table<WebGlFramebuffer>,
    queries: Table<WebGlQuery>,
    renderbuffers: Table<WebGlRenderbuffer>,
    samplers: Table<WebGlSampler>,
    textures: Table<WebGlTexture>,
    vertex_arrays: Table<WebGlVertexArrayObject>,
    units: Table<Unit>,
    fences: BTreeMap<u64, WebGlSync>,
    last_fence: u64,
    locations: BTreeMap<i32, Location>,
    location_numbers: BTreeMap<(u32, Vec<u8>), i32>,
    last_location: i32,
    current_program: u32,
}

/// An OpenGL ES 3 context over a page's WebGL 2 context.
pub struct WebGl {
    context: Context,
    number: u64,
    objects: RefCell<Objects>,
    /// The first error this implementation recorded and `get_error` has not yet returned.
    pending: Cell<u32>,
    refusals: Cell<u64>,
    /// The extensions enabled, by WebGL name, with each one's object.
    extensions: Vec<(String, Object)>,
    longest_wait: f64,
}

impl WebGl {
    /// Takes a page's WebGL 2 context and enables every extension it supports but those named in `off`, as an
    /// OpenGL ES context has every extension it names on.
    pub fn new(context: Context, off: &[&str]) -> Self {
        let mut extensions = Vec::new();
        for name in context.get_supported_extensions().map(|names| names.iter().filter_map(|name| name.as_string()).collect::<Vec<_>>()).unwrap_or_default() {
            if off.contains(&name.as_str()) {
                continue;
            }
            if let Ok(Some(object)) = context.get_extension(&name) {
                extensions.push((name, object));
            }
        }
        let longest_wait = context.get_parameter(MAX_CLIENT_WAIT_TIMEOUT_WEBGL).ok().and_then(|value| value.as_f64()).unwrap_or(0.0);
        Self {
            context,
            number: CONTEXTS.fetch_add(1, Ordering::Relaxed),
            objects: RefCell::new(Objects::default()),
            pending: Cell::new(gl::NO_ERROR),
            refusals: Cell::new(0),
            extensions,
            longest_wait,
        }
    }

    /// The WebGL 2 context the methods call, for what OpenGL ES has no command for (the drawing buffer's size).
    pub fn context(&self) -> &Context {
        &self.context
    }

    /// How many calls this implementation refused itself, each with the error `get_error` gives.
    pub fn refusals(&self) -> u64 {
        self.refusals.get()
    }

    /// Records the error, as OpenGL ES keeps the first one until it is asked for.
    fn record(&self, error: u32) {
        if self.pending.get() == gl::NO_ERROR {
            self.pending.set(error);
        }
    }

    /// Refuses the call with the error.
    fn refuse(&self, error: u32) {
        self.refusals.set(self.refusals.get().saturating_add(1));
        self.record(error);
    }

    /// The object of a number, None for 0, refused when the table does not hold the number.
    fn found<T: Clone + AsRef<JsValue>>(&self, table: fn(&Objects) -> &Table<T>, number: u32) -> Result<Option<T>, ()> {
        if number == 0 {
            return Ok(None);
        }
        match table(&self.objects.borrow()).get(number) {
            Some(object) => Ok(Some(object)),
            None => {
                self.refuse(gl::INVALID_OPERATION);
                Err(())
            }
        }
    }

    /// The object of a number that must name one.
    fn named<T: Clone + AsRef<JsValue>>(&self, table: fn(&Objects) -> &Table<T>, number: u32) -> Option<T> {
        match self.found(table, number) {
            Ok(Some(object)) => Some(object),
            Ok(None) => {
                self.refuse(gl::INVALID_OPERATION);
                None
            }
            Err(()) => None,
        }
    }

    fn program(&self, number: u32) -> Option<WebGlProgram> {
        match self.objects.borrow().units.get(number) {
            Some(Unit::Program(program)) => Some(program),
            Some(Unit::Shader(_)) => {
                self.refuse(gl::INVALID_OPERATION);
                None
            }
            None => {
                self.refuse(gl::INVALID_VALUE);
                None
            }
        }
    }

    fn shader(&self, number: u32) -> Option<WebGlShader> {
        match self.objects.borrow().units.get(number) {
            Some(Unit::Shader(shader)) => Some(shader),
            Some(Unit::Program(_)) => {
                self.refuse(gl::INVALID_OPERATION);
                None
            }
            None => {
                self.refuse(gl::INVALID_VALUE);
                None
            }
        }
    }

    /// Makes one object per slot with `make`, writing its number (0 where the browser made none, which it does
    /// only for a lost context).
    fn generate<T: Clone + AsRef<JsValue>>(&self, numbers: &mut [u32], make: impl Fn(&Context) -> Option<T>, table: fn(&mut Objects) -> &mut Table<T>) {
        for slot in numbers {
            *slot = match make(&self.context) {
                Some(object) => table(&mut self.objects.borrow_mut()).add(object).unwrap_or_else(|| {
                    self.refuse(gl::OUT_OF_MEMORY);
                    0
                }),
                None => {
                    self.record(gl::CONTEXT_LOST);
                    0
                }
            };
        }
    }

    /// Deletes the objects of the numbers held (others are skipped, as OpenGL ES skips names it never made).
    fn delete<T: Clone + AsRef<JsValue>>(&self, numbers: &[u32], table: fn(&mut Objects) -> &mut Table<T>, delete: impl Fn(&Context, &T)) {
        for number in numbers {
            let removed = table(&mut self.objects.borrow_mut()).held.remove(number);
            if let Some(object) = removed {
                delete(&self.context, &object);
            }
        }
    }

    /// The number of a WebGL object a query answered with, from whichever table holds it.
    fn number_of(&self, value: &JsValue) -> Option<u32> {
        let objects = self.objects.borrow();
        objects
            .buffers
            .number_of(value)
            .or_else(|| objects.framebuffers.number_of(value))
            .or_else(|| objects.queries.number_of(value))
            .or_else(|| objects.renderbuffers.number_of(value))
            .or_else(|| objects.samplers.number_of(value))
            .or_else(|| objects.textures.number_of(value))
            .or_else(|| objects.vertex_arrays.number_of(value))
            .or_else(|| objects.units.number_of(value))
    }

    /// A query's answer as numbers: a number, a boolean, a list of either, a typed array, or an object of the
    /// context's as its number (none bound is 0). None for an answer that is text.
    fn numbers(&self, value: &JsValue) -> Option<Vec<f64>> {
        if value.is_null() || value.is_undefined() {
            return Some(vec![0.0]);
        }
        if let Some(number) = value.as_f64() {
            return Some(vec![number]);
        }
        if let Some(flag) = value.as_bool() {
            return Some(vec![f64::from(u8::from(flag))]);
        }
        if let Some(values) = value.dyn_ref::<Int32Array>() {
            return Some(values.to_vec().into_iter().map(f64::from).collect());
        }
        if let Some(values) = value.dyn_ref::<Uint32Array>() {
            return Some(values.to_vec().into_iter().map(f64::from).collect());
        }
        if let Some(values) = value.dyn_ref::<Float32Array>() {
            return Some(values.to_vec().into_iter().map(f64::from).collect());
        }
        if let Some(values) = value.dyn_ref::<Array>() {
            return values.iter().map(|value| value.as_f64().or_else(|| value.as_bool().map(|flag| f64::from(u8::from(flag))))).collect();
        }
        self.number_of(value).map(|number| vec![f64::from(number)])
    }

    /// The numbers of a state value, with the extension count answered here.
    fn parameter(&self, name: u32) -> Option<Vec<f64>> {
        if name == gl::NUM_EXTENSIONS {
            return Some(vec![self.extensions.len() as f64]);
        }
        match self.context.get_parameter(name) {
            Ok(value) => self.numbers(&value),
            Err(_) => {
                self.record(gl::INVALID_ENUM);
                Some(Vec::new())
            }
        }
    }

    /// Writes as many of the answer's values as the slice holds; text is refused.
    fn write_numbers<T>(&self, answer: Option<Vec<f64>>, into: &mut [T], convert: fn(f64) -> T) {
        match answer {
            Some(values) => {
                for (slot, value) in into.iter_mut().zip(values) {
                    *slot = convert(value);
                }
            }
            None => self.refuse(gl::INVALID_ENUM),
        }
    }

    /// The enabled extension's object, if this context enabled it.
    fn extension(&self, name: &str) -> Option<&Object> {
        self.extensions.iter().find(|(enabled, _)| enabled == name).map(|(_, object)| object)
    }

    /// Calls a method of an enabled extension's object: None where the extension is not enabled; a call the
    /// browser throws on is recorded as INVALID_OPERATION.
    fn extension_call(&self, extension: &str, method: &str, arguments: &[JsValue]) -> Option<JsValue> {
        let object = self.extension(extension)?;
        let answer = Reflect::get(object, &JsValue::from_str(method))
            .ok()
            .and_then(|function| function.dyn_into::<Function>().ok())
            .and_then(|function| function.apply(object, &arguments.iter().collect::<Array>()).ok());
        Some(answer.unwrap_or_else(|| {
            self.record(gl::INVALID_OPERATION);
            JsValue::UNDEFINED
        }))
    }

    /// The fence's sync object, refused when it is not one of this context's live fences.
    fn sync(&self, fence: &Fence) -> Option<WebGlSync> {
        let found = (fence.context == self.number).then(|| self.objects.borrow().fences.get(&fence.number).cloned()).flatten();
        if found.is_none() {
            self.refuse(gl::INVALID_VALUE);
        }
        found
    }

    /// The uniform location object of a number, for the program in use: None for -1 (OpenGL ES ignores it) and
    /// refused for a number not handed out or of another program.
    fn location(&self, number: i32) -> Result<Option<WebGlUniformLocation>, ()> {
        if number == -1 {
            return Ok(None);
        }
        let found = {
            let objects = self.objects.borrow();
            objects.locations.get(&number).filter(|held| held.program == objects.current_program).map(|held| held.location.clone())
        };
        if found.is_none() {
            self.refuse(gl::INVALID_OPERATION);
            return Err(());
        }
        Ok(found)
    }

    /// Calls `set` with the location's object, skipping -1 and refusing a location of no program in use.
    fn uniform(&self, number: i32, set: impl FnOnce(&Context, Option<&WebGlUniformLocation>)) {
        if let Ok(Some(location)) = self.location(number) {
            set(&self.context, Some(&location));
        }
    }

    /// Records the browser's refusal of a call that throws.
    fn thrown<T>(&self, result: Result<T, JsValue>) {
        if result.is_err() {
            self.record(gl::INVALID_OPERATION);
        }
    }

    /// Writes a text as OpenGL ES writes a log: at most the slice less one byte, then a zero; the length written
    /// without the zero.
    fn write_text(text: &[u8], into: &mut [u8]) -> i32 {
        let Some(room) = into.len().checked_sub(1) else { return 0 };
        let written = text.len().min(room);
        into[..written].copy_from_slice(&text[..written]);
        into[written] = 0;
        i32::try_from(written).unwrap_or(i32::MAX)
    }

    /// A text's length as OpenGL ES gives a log's or a source's: with its zero, 0 for none.
    fn text_length(text: Option<String>) -> f64 {
        text.filter(|text| !text.is_empty()).map_or(0.0, |text| text.len() as f64 + 1.0)
    }
}

/// The browser's typed array over a copy of the bytes, of the element type WebGL 2 requires for the pixel type
/// (WebGL 2 specification §5.14.8, "pixel types and typed arrays"); None for a type no array is given for or bytes
/// that are not whole elements (both refused as INVALID_ENUM, the error WebGL gives a mismatched array's type).
fn pixel_view(type_: u32, bytes: &[u8]) -> Option<Object> {
    let copied = Uint8Array::from(bytes);
    let buffer = copied.buffer();
    let whole = |size: usize| bytes.len().is_multiple_of(size).then(|| u32::try_from(bytes.len() / size).ok()).flatten();
    Some(match type_ {
        gl::UNSIGNED_BYTE => copied.into(),
        gl::BYTE => Int8Array::new_with_byte_offset_and_length(&buffer, 0, whole(1)?).into(),
        gl::UNSIGNED_SHORT | gl::HALF_FLOAT | gl::UNSIGNED_SHORT_5_6_5 | gl::UNSIGNED_SHORT_4_4_4_4 | gl::UNSIGNED_SHORT_5_5_5_1 => {
            Uint16Array::new_with_byte_offset_and_length(&buffer, 0, whole(2)?).into()
        }
        gl::SHORT => Int16Array::new_with_byte_offset_and_length(&buffer, 0, whole(2)?).into(),
        gl::UNSIGNED_INT
        | gl::UNSIGNED_INT_2_10_10_10_REV
        | gl::UNSIGNED_INT_10F_11F_11F_REV
        | gl::UNSIGNED_INT_5_9_9_9_REV
        | gl::UNSIGNED_INT_24_8
        | gl::FLOAT_32_UNSIGNED_INT_24_8_REV => Uint32Array::new_with_byte_offset_and_length(&buffer, 0, whole(4)?).into(),
        gl::INT => Int32Array::new_with_byte_offset_and_length(&buffer, 0, whole(4)?).into(),
        gl::FLOAT => Float32Array::new_with_byte_offset_and_length(&buffer, 0, whole(4)?).into(),
        _ => return None,
    })
}

/// A state value as OpenGL ES gives an integer: rounded, and an unsigned 32-bit value (a mask) as its bits.
fn to_i32(value: f64) -> i32 {
    if value > f64::from(i32::MAX) && value <= f64::from(u32::MAX) { (value as u32).cast_signed() } else { value.round() as i32 }
}

/// A state value as OpenGL ES gives a 64-bit integer.
fn to_i64(value: f64) -> i64 {
    value.round() as i64
}

/// A state value as OpenGL ES gives a float.
fn to_f32(value: f64) -> f32 {
    value as f32
}

/// A state value as OpenGL ES gives a boolean.
fn to_bool(value: f64) -> bool {
    value != 0.0
}

/// A list of numbers as the browser's array.
fn number_list(values: &[u32]) -> Array {
    values.iter().map(|value| JsValue::from_f64(f64::from(*value))).collect()
}

impl OpenGlEs for WebGl {
    type Fence = Fence;

    fn active_texture(&self, texture: u32) {
        self.context.active_texture(texture);
    }

    fn attach_shader(&self, program: u32, shader: u32) {
        if let (Some(program), Some(shader)) = (self.program(program), self.shader(shader)) {
            self.context.attach_shader(&program, &shader);
        }
    }

    fn begin_query(&self, target: u32, id: u32) {
        if let Some(query) = self.named(|objects| &objects.queries, id) {
            self.context.begin_query(target, &query);
        }
    }

    fn begin_transform_feedback(&self, primitive_mode: u32) {
        self.context.begin_transform_feedback(primitive_mode);
    }

    fn bind_buffer(&self, target: u32, buffer: u32) {
        if let Ok(buffer) = self.found(|objects| &objects.buffers, buffer) {
            self.context.bind_buffer(target, buffer.as_ref());
        }
    }

    fn bind_buffer_base(&self, target: u32, index: u32, buffer: u32) {
        if let Ok(buffer) = self.found(|objects| &objects.buffers, buffer) {
            self.context.bind_buffer_base(target, index, buffer.as_ref());
        }
    }

    fn bind_buffer_range(&self, target: u32, index: u32, buffer: u32, offset: i64, size: i64) {
        if let Ok(buffer) = self.found(|objects| &objects.buffers, buffer) {
            self.context.bind_buffer_range_with_f64_and_f64(target, index, buffer.as_ref(), offset as f64, size as f64);
        }
    }

    fn bind_framebuffer(&self, target: u32, framebuffer: u32) {
        if let Ok(framebuffer) = self.found(|objects| &objects.framebuffers, framebuffer) {
            self.context.bind_framebuffer(target, framebuffer.as_ref());
        }
    }

    fn bind_renderbuffer(&self, target: u32, renderbuffer: u32) {
        if let Ok(renderbuffer) = self.found(|objects| &objects.renderbuffers, renderbuffer) {
            self.context.bind_renderbuffer(target, renderbuffer.as_ref());
        }
    }

    fn bind_sampler(&self, unit: u32, sampler: u32) {
        if let Ok(sampler) = self.found(|objects| &objects.samplers, sampler) {
            self.context.bind_sampler(unit, sampler.as_ref());
        }
    }

    fn bind_texture(&self, target: u32, texture: u32) {
        if let Ok(texture) = self.found(|objects| &objects.textures, texture) {
            self.context.bind_texture(target, texture.as_ref());
        }
    }

    fn bind_vertex_array(&self, array: u32) {
        if let Ok(array) = self.found(|objects| &objects.vertex_arrays, array) {
            self.context.bind_vertex_array(array.as_ref());
        }
    }

    fn blend_color(&self, red: f32, green: f32, blue: f32, alpha: f32) {
        self.context.blend_color(red, green, blue, alpha);
    }

    fn blend_equation_separate(&self, mode_rgb: u32, mode_alpha: u32) {
        self.context.blend_equation_separate(mode_rgb, mode_alpha);
    }

    fn blend_func_separate(&self, sfactor_rgb: u32, dfactor_rgb: u32, sfactor_alpha: u32, dfactor_alpha: u32) {
        self.context.blend_func_separate(sfactor_rgb, dfactor_rgb, sfactor_alpha, dfactor_alpha);
    }

    fn blit_framebuffer(&self, src_x0: i32, src_y0: i32, src_x1: i32, src_y1: i32, dst_x0: i32, dst_y0: i32, dst_x1: i32, dst_y1: i32, mask: u32, filter: u32) {
        self.context.blit_framebuffer(src_x0, src_y0, src_x1, src_y1, dst_x0, dst_y0, dst_x1, dst_y1, mask, filter);
    }

    fn buffer_data(&self, target: u32, size: i64, data: Option<&[u8]>, usage: u32) {
        let Ok(length) = usize::try_from(size) else {
            self.refuse(gl::INVALID_VALUE);
            return;
        };
        match data {
            None => self.context.buffer_data_with_f64(target, size as f64, usage),
            Some(bytes) => match bytes.get(..length) {
                Some(bytes) => self.context.buffer_data_with_u8_array(target, bytes, usage),
                None => self.refuse(gl::INVALID_VALUE),
            },
        }
    }

    fn buffer_sub_data(&self, target: u32, offset: i64, data: &[u8]) {
        self.context.buffer_sub_data_with_f64_and_u8_array(target, offset as f64, data);
    }

    fn check_framebuffer_status(&self, target: u32) -> u32 {
        self.context.check_framebuffer_status(target)
    }

    fn clear_bufferfi(&self, buffer: u32, drawbuffer: i32, depth: f32, stencil: i32) {
        self.context.clear_bufferfi(buffer, drawbuffer, depth, stencil);
    }

    fn clear_bufferfv(&self, buffer: u32, drawbuffer: i32, value: &[f32]) {
        self.context.clear_bufferfv_with_f32_array(buffer, drawbuffer, value);
    }

    fn clear_bufferiv(&self, buffer: u32, drawbuffer: i32, value: &[i32]) {
        self.context.clear_bufferiv_with_i32_array(buffer, drawbuffer, value);
    }

    fn clear_bufferuiv(&self, buffer: u32, drawbuffer: i32, value: &[u32]) {
        self.context.clear_bufferuiv_with_u32_array(buffer, drawbuffer, value);
    }

    fn client_wait_sync(&self, sync: &Fence, flags: u32, timeout: u64) -> u32 {
        match self.sync(sync) {
            // WebGL 2 refuses a wait longer than its longest (often 0: a page's script may not block).
            Some(sync) => self.context.client_wait_sync_with_f64(&sync, flags, (timeout as f64).min(self.longest_wait)),
            None => gl::WAIT_FAILED,
        }
    }

    fn color_mask(&self, red: bool, green: bool, blue: bool, alpha: bool) {
        self.context.color_mask(red, green, blue, alpha);
    }

    fn compile_shader(&self, shader: u32) {
        if let Some(shader) = self.shader(shader) {
            self.context.compile_shader(&shader);
        }
    }

    fn compressed_tex_sub_image_2d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, width: i32, height: i32, format: u32, data: CompressedPixelUnpack<'_>) {
        match data {
            CompressedPixelUnpack::Bytes(bytes) => {
                let view = Uint8Array::from(bytes);
                self.context.compressed_tex_sub_image_2d_with_array_buffer_view(target, level, xoffset, yoffset, width, height, format, &view);
            }
            CompressedPixelUnpack::Offset { offset, size } => {
                self.context.compressed_tex_sub_image_2d_with_i32_and_f64(target, level, xoffset, yoffset, width, height, format, size, offset as f64);
            }
        }
    }

    fn compressed_tex_sub_image_3d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, zoffset: i32, width: i32, height: i32, depth: i32, format: u32, data: CompressedPixelUnpack<'_>) {
        match data {
            CompressedPixelUnpack::Bytes(bytes) => {
                let view = Uint8Array::from(bytes);
                self.context.compressed_tex_sub_image_3d_with_array_buffer_view(target, level, xoffset, yoffset, zoffset, width, height, depth, format, &view);
            }
            CompressedPixelUnpack::Offset { offset, size } => {
                self.context.compressed_tex_sub_image_3d_with_i32_and_f64(target, level, xoffset, yoffset, zoffset, width, height, depth, format, size, offset as f64);
            }
        }
    }

    fn copy_buffer_sub_data(&self, read_target: u32, write_target: u32, read_offset: i64, write_offset: i64, size: i64) {
        self.context.copy_buffer_sub_data_with_f64_and_f64_and_f64(read_target, write_target, read_offset as f64, write_offset as f64, size as f64);
    }

    fn copy_tex_sub_image_2d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, x: i32, y: i32, width: i32, height: i32) {
        self.context.copy_tex_sub_image_2d(target, level, xoffset, yoffset, x, y, width, height);
    }

    fn copy_tex_sub_image_3d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, zoffset: i32, x: i32, y: i32, width: i32, height: i32) {
        self.context.copy_tex_sub_image_3d(target, level, xoffset, yoffset, zoffset, x, y, width, height);
    }

    fn create_program(&self) -> u32 {
        let mut number = [0];
        self.generate(&mut number, |context| context.create_program().map(Unit::Program), |objects| &mut objects.units);
        number[0]
    }

    fn create_shader(&self, type_: u32) -> u32 {
        let mut number = [0];
        self.generate(&mut number, |context| context.create_shader(type_).map(Unit::Shader), |objects| &mut objects.units);
        number[0]
    }

    fn cull_face(&self, mode: u32) {
        self.context.cull_face(mode);
    }

    fn delete_buffers(&self, buffers: &[u32]) {
        self.delete(buffers, |objects| &mut objects.buffers, |context, buffer| context.delete_buffer(Some(buffer)));
    }

    fn delete_framebuffers(&self, framebuffers: &[u32]) {
        self.delete(framebuffers, |objects| &mut objects.framebuffers, |context, framebuffer| context.delete_framebuffer(Some(framebuffer)));
    }

    fn delete_program(&self, program: u32) {
        if program == 0 {
            return;
        }
        if let Some(program_object) = self.program(program) {
            {
                let mut objects = self.objects.borrow_mut();
                objects.units.held.remove(&program);
                objects.locations.retain(|_, held| held.program != program);
                objects.location_numbers.retain(|(of, _), _| *of != program);
            }
            self.context.delete_program(Some(&program_object));
        }
    }

    fn delete_queries(&self, ids: &[u32]) {
        self.delete(ids, |objects| &mut objects.queries, |context, query| context.delete_query(Some(query)));
    }

    fn delete_renderbuffers(&self, renderbuffers: &[u32]) {
        self.delete(renderbuffers, |objects| &mut objects.renderbuffers, |context, renderbuffer| context.delete_renderbuffer(Some(renderbuffer)));
    }

    fn delete_samplers(&self, samplers: &[u32]) {
        self.delete(samplers, |objects| &mut objects.samplers, |context, sampler| context.delete_sampler(Some(sampler)));
    }

    fn delete_shader(&self, shader: u32) {
        if shader == 0 {
            return;
        }
        if let Some(shader_object) = self.shader(shader) {
            self.objects.borrow_mut().units.held.remove(&shader);
            self.context.delete_shader(Some(&shader_object));
        }
    }

    fn delete_sync(&self, sync: &Fence) {
        if let Some(object) = self.sync(sync) {
            self.objects.borrow_mut().fences.remove(&sync.number);
            self.context.delete_sync(Some(&object));
        }
    }

    fn delete_textures(&self, textures: &[u32]) {
        self.delete(textures, |objects| &mut objects.textures, |context, texture| context.delete_texture(Some(texture)));
    }

    fn delete_vertex_arrays(&self, arrays: &[u32]) {
        self.delete(arrays, |objects| &mut objects.vertex_arrays, |context, array| context.delete_vertex_array(Some(array)));
    }

    fn depth_func(&self, func: u32) {
        self.context.depth_func(func);
    }

    fn depth_mask(&self, flag: bool) {
        self.context.depth_mask(flag);
    }

    fn depth_rangef(&self, n: f32, f: f32) {
        self.context.depth_range(n, f);
    }

    fn detach_shader(&self, program: u32, shader: u32) {
        if let (Some(program), Some(shader)) = (self.program(program), self.shader(shader)) {
            self.context.detach_shader(&program, &shader);
        }
    }

    fn disable(&self, cap: u32) {
        self.context.disable(cap);
    }

    fn disable_vertex_attrib_array(&self, index: u32) {
        self.context.disable_vertex_attrib_array(index);
    }

    fn draw_arrays(&self, mode: u32, first: i32, count: i32) {
        self.context.draw_arrays(mode, first, count);
    }

    fn draw_arrays_instanced(&self, mode: u32, first: i32, count: i32, instancecount: i32) {
        self.context.draw_arrays_instanced(mode, first, count, instancecount);
    }

    fn draw_buffers(&self, bufs: &[u32]) {
        self.context.draw_buffers(&number_list(bufs));
    }

    fn draw_elements_instanced(&self, mode: u32, count: i32, type_: u32, indices: u64, instancecount: i32) {
        self.context.draw_elements_instanced_with_f64(mode, count, type_, indices as f64, instancecount);
    }

    fn enable(&self, cap: u32) {
        self.context.enable(cap);
    }

    fn enable_vertex_attrib_array(&self, index: u32) {
        self.context.enable_vertex_attrib_array(index);
    }

    fn end_query(&self, target: u32) {
        self.context.end_query(target);
    }

    fn end_transform_feedback(&self) {
        self.context.end_transform_feedback();
    }

    fn fence_sync(&self, condition: u32, flags: u32) -> Option<Fence> {
        let Some(sync) = self.context.fence_sync(condition, flags) else {
            self.record(gl::INVALID_OPERATION);
            return None;
        };
        let mut objects = self.objects.borrow_mut();
        let number = objects.last_fence.checked_add(1)?;
        objects.last_fence = number;
        objects.fences.insert(number, sync);
        Some(Fence { context: self.number, number })
    }

    fn finish(&self) {
        self.context.finish();
    }

    fn flush(&self) {
        self.context.flush();
    }

    fn framebuffer_renderbuffer(&self, target: u32, attachment: u32, renderbuffertarget: u32, renderbuffer: u32) {
        if let Ok(renderbuffer) = self.found(|objects| &objects.renderbuffers, renderbuffer) {
            self.context.framebuffer_renderbuffer(target, attachment, renderbuffertarget, renderbuffer.as_ref());
        }
    }

    fn framebuffer_texture_2d(&self, target: u32, attachment: u32, textarget: u32, texture: u32, level: i32) {
        if let Ok(texture) = self.found(|objects| &objects.textures, texture) {
            self.context.framebuffer_texture_2d(target, attachment, textarget, texture.as_ref(), level);
        }
    }

    fn framebuffer_texture_layer(&self, target: u32, attachment: u32, texture: u32, level: i32, layer: i32) {
        if let Ok(texture) = self.found(|objects| &objects.textures, texture) {
            self.context.framebuffer_texture_layer(target, attachment, texture.as_ref(), level, layer);
        }
    }

    fn front_face(&self, mode: u32) {
        self.context.front_face(mode);
    }

    fn gen_buffers(&self, buffers: &mut [u32]) {
        self.generate(buffers, Context::create_buffer, |objects| &mut objects.buffers);
    }

    fn gen_framebuffers(&self, framebuffers: &mut [u32]) {
        self.generate(framebuffers, Context::create_framebuffer, |objects| &mut objects.framebuffers);
    }

    fn gen_queries(&self, ids: &mut [u32]) {
        self.generate(ids, Context::create_query, |objects| &mut objects.queries);
    }

    fn gen_renderbuffers(&self, renderbuffers: &mut [u32]) {
        self.generate(renderbuffers, Context::create_renderbuffer, |objects| &mut objects.renderbuffers);
    }

    fn gen_samplers(&self, samplers: &mut [u32]) {
        self.generate(samplers, Context::create_sampler, |objects| &mut objects.samplers);
    }

    fn gen_textures(&self, textures: &mut [u32]) {
        self.generate(textures, Context::create_texture, |objects| &mut objects.textures);
    }

    fn gen_vertex_arrays(&self, arrays: &mut [u32]) {
        self.generate(arrays, Context::create_vertex_array, |objects| &mut objects.vertex_arrays);
    }

    fn generate_mipmap(&self, target: u32) {
        self.context.generate_mipmap(target);
    }

    fn get_booleanv(&self, pname: u32, data: &mut [bool]) {
        self.write_numbers(self.parameter(pname), data, to_bool);
    }

    fn get_error(&self) -> u32 {
        let pending = self.pending.replace(gl::NO_ERROR);
        if pending == gl::NO_ERROR { self.context.get_error() } else { pending }
    }

    fn get_floatv(&self, pname: u32, data: &mut [f32]) {
        self.write_numbers(self.parameter(pname), data, to_f32);
    }

    fn get_integer64v(&self, pname: u32, data: &mut [i64]) {
        self.write_numbers(self.parameter(pname), data, to_i64);
    }

    fn get_integerv(&self, pname: u32, data: &mut [i32]) {
        self.write_numbers(self.parameter(pname), data, to_i32);
    }

    fn get_internalformativ(&self, target: u32, internalformat: u32, pname: u32, params: &mut [i32]) {
        // WebGL 2 answers SAMPLES only; their count is the list's length.
        let asked = if pname == gl::NUM_SAMPLE_COUNTS { gl::SAMPLES } else { pname };
        let answer = match self.context.get_internalformat_parameter(target, internalformat, asked) {
            Ok(value) => self.numbers(&value),
            Err(_) => {
                self.record(gl::INVALID_ENUM);
                Some(Vec::new())
            }
        };
        let answer = if pname == gl::NUM_SAMPLE_COUNTS { answer.map(|samples| vec![samples.len() as f64]) } else { answer };
        self.write_numbers(answer, params, to_i32);
    }

    fn get_program_info_log(&self, program: u32, info_log: &mut [u8]) -> i32 {
        let Some(program) = self.program(program) else { return 0 };
        Self::write_text(self.context.get_program_info_log(&program).unwrap_or_default().as_bytes(), info_log)
    }

    fn get_programiv(&self, program: u32, pname: u32, params: &mut [i32]) {
        let Some(program) = self.program(program) else { return };
        let answer = if pname == gl::INFO_LOG_LENGTH {
            Some(vec![Self::text_length(self.context.get_program_info_log(&program))])
        } else {
            self.numbers(&self.context.get_program_parameter(&program, pname))
        };
        self.write_numbers(answer, params, to_i32);
    }

    fn get_query_objectuiv(&self, id: u32, pname: u32, params: &mut [u32]) {
        if let Some(query) = self.named(|objects| &objects.queries, id) {
            let answer = self.numbers(&self.context.get_query_parameter(&query, pname));
            self.write_numbers(answer, params, |value| value as u32);
        }
    }

    fn get_shader_info_log(&self, shader: u32, info_log: &mut [u8]) -> i32 {
        let Some(shader) = self.shader(shader) else { return 0 };
        Self::write_text(self.context.get_shader_info_log(&shader).unwrap_or_default().as_bytes(), info_log)
    }

    fn get_shaderiv(&self, shader: u32, pname: u32, params: &mut [i32]) {
        let Some(shader) = self.shader(shader) else { return };
        let answer = match pname {
            gl::INFO_LOG_LENGTH => Some(vec![Self::text_length(self.context.get_shader_info_log(&shader))]),
            gl::SHADER_SOURCE_LENGTH => Some(vec![Self::text_length(self.context.get_shader_source(&shader))]),
            _ => self.numbers(&self.context.get_shader_parameter(&shader, pname)),
        };
        self.write_numbers(answer, params, to_i32);
    }

    fn get_string(&self, name: u32) -> Option<Vec<u8>> {
        if name == gl::EXTENSIONS {
            return Some(self.extensions.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>().join(" ").into_bytes());
        }
        let text = match self.context.get_parameter(name) {
            Ok(value) => value.as_string(),
            Err(_) => None,
        };
        let Some(text) = text else {
            self.record(gl::INVALID_ENUM);
            return None;
        };
        // OpenGL ES 3.2 §20.2: VERSION starts "OpenGL ES N.M", SHADING_LANGUAGE_VERSION "OpenGL ES GLSL ES N.MM".
        Some(
            match name {
                gl::VERSION => format!("OpenGL ES 3.0 ({text})"),
                gl::SHADING_LANGUAGE_VERSION => format!("OpenGL ES GLSL ES 3.00 ({text})"),
                _ => text,
            }
            .into_bytes(),
        )
    }

    fn get_stringi(&self, name: u32, index: u32) -> Option<Vec<u8>> {
        if name != gl::EXTENSIONS {
            self.record(gl::INVALID_ENUM);
            return None;
        }
        let found = usize::try_from(index).ok().and_then(|index| self.extensions.get(index));
        if found.is_none() {
            self.record(gl::INVALID_VALUE);
        }
        found.map(|(name, _)| name.clone().into_bytes())
    }

    fn get_synciv(&self, sync: &Fence, pname: u32, values: &mut [i32]) -> i32 {
        let Some(sync) = self.sync(sync) else { return 0 };
        let answer = self.numbers(&self.context.get_sync_parameter(&sync, pname));
        let written = answer.as_ref().map_or(0, |answer| answer.len().min(values.len()));
        self.write_numbers(answer, values, to_i32);
        i32::try_from(written).unwrap_or(i32::MAX)
    }

    fn get_uniform_block_index(&self, program: u32, uniform_block_name: &CStr) -> u32 {
        let Some(program) = self.program(program) else { return gl::INVALID_INDEX };
        let Ok(name) = uniform_block_name.to_str() else {
            self.refuse(gl::INVALID_VALUE);
            return gl::INVALID_INDEX;
        };
        self.context.get_uniform_block_index(&program, name)
    }

    fn get_uniform_location(&self, program: u32, name: &CStr) -> i32 {
        let Some(program_object) = self.program(program) else { return -1 };
        let key = (program, name.to_bytes().to_vec());
        if let Some(number) = self.objects.borrow().location_numbers.get(&key) {
            return *number;
        }
        let Ok(text) = name.to_str() else {
            self.refuse(gl::INVALID_VALUE);
            return -1;
        };
        let Some(location) = self.context.get_uniform_location(&program_object, text) else { return -1 };
        let mut objects = self.objects.borrow_mut();
        let Some(number) = objects.last_location.checked_add(1) else {
            drop(objects);
            self.refuse(gl::OUT_OF_MEMORY);
            return -1;
        };
        objects.last_location = number;
        objects.locations.insert(number, Location { program, location });
        objects.location_numbers.insert(key, number);
        number
    }

    fn get_vertex_attribiv(&self, index: u32, pname: u32, params: &mut [i32]) {
        let answer = match self.context.get_vertex_attrib(index, pname) {
            Ok(value) => self.numbers(&value),
            Err(_) => {
                self.record(gl::INVALID_VALUE);
                Some(Vec::new())
            }
        };
        self.write_numbers(answer, params, to_i32);
    }

    fn invalidate_framebuffer(&self, target: u32, attachments: &[u32]) {
        let result = self.context.invalidate_framebuffer(target, &number_list(attachments));
        self.thrown(result);
    }

    fn link_program(&self, program: u32) {
        if let Some(program_object) = self.program(program) {
            // Linking again makes new locations: the numbers handed out for the old ones are no longer the program's.
            {
                let mut objects = self.objects.borrow_mut();
                objects.locations.retain(|_, held| held.program != program);
                objects.location_numbers.retain(|(of, _), _| *of != program);
            }
            self.context.link_program(&program_object);
        }
    }

    fn pixel_storei(&self, pname: u32, param: i32) {
        self.context.pixel_storei(pname, param);
    }

    fn polygon_offset(&self, factor: f32, units: f32) {
        self.context.polygon_offset(factor, units);
    }

    fn read_pixels(&self, x: i32, y: i32, width: i32, height: i32, format: u32, type_: u32, pixels: PixelPack<'_>) {
        match pixels {
            PixelPack::Bytes(bytes) => {
                let Some(view) = pixel_view(type_, bytes) else {
                    self.refuse(gl::INVALID_ENUM);
                    return;
                };
                let result = self.context.read_pixels_with_opt_array_buffer_view(x, y, width, height, format, type_, Some(&view));
                if result.is_ok() {
                    if let Some(typed) = view.dyn_ref::<Uint8Array>() {
                        typed.copy_to(bytes);
                    } else if let Ok(buffer) = Reflect::get(&view, &JsValue::from_str("buffer")) {
                        Uint8Array::new(&buffer).copy_to(bytes);
                    }
                }
                self.thrown(result);
            }
            PixelPack::Offset(offset) => {
                let result = self.context.read_pixels_with_f64(x, y, width, height, format, type_, offset as f64);
                self.thrown(result);
            }
        }
    }

    fn renderbuffer_storage(&self, target: u32, internalformat: u32, width: i32, height: i32) {
        self.context.renderbuffer_storage(target, internalformat, width, height);
    }

    fn renderbuffer_storage_multisample(&self, target: u32, samples: i32, internalformat: u32, width: i32, height: i32) {
        self.context.renderbuffer_storage_multisample(target, samples, internalformat, width, height);
    }

    fn sampler_parameterf(&self, sampler: u32, pname: u32, param: f32) {
        if let Some(sampler) = self.named(|objects| &objects.samplers, sampler) {
            self.context.sampler_parameterf(&sampler, pname, param);
        }
    }

    fn sampler_parameteri(&self, sampler: u32, pname: u32, param: i32) {
        if let Some(sampler) = self.named(|objects| &objects.samplers, sampler) {
            self.context.sampler_parameteri(&sampler, pname, param);
        }
    }

    fn scissor(&self, x: i32, y: i32, width: i32, height: i32) {
        self.context.scissor(x, y, width, height);
    }

    fn shader_source(&self, shader: u32, string: &[&[u8]]) {
        let Some(shader) = self.shader(shader) else { return };
        match String::from_utf8(string.concat()) {
            Ok(source) => self.context.shader_source(&shader, &source),
            Err(_) => self.refuse(gl::INVALID_VALUE),
        }
    }

    fn stencil_func(&self, func: u32, ref_: i32, mask: u32) {
        self.context.stencil_func(func, ref_, mask);
    }

    fn stencil_func_separate(&self, face: u32, func: u32, ref_: i32, mask: u32) {
        self.context.stencil_func_separate(face, func, ref_, mask);
    }

    fn stencil_mask(&self, mask: u32) {
        self.context.stencil_mask(mask);
    }

    fn stencil_op(&self, fail: u32, zfail: u32, zpass: u32) {
        self.context.stencil_op(fail, zfail, zpass);
    }

    fn stencil_op_separate(&self, face: u32, sfail: u32, dpfail: u32, dppass: u32) {
        self.context.stencil_op_separate(face, sfail, dpfail, dppass);
    }

    fn tex_parameteri(&self, target: u32, pname: u32, param: i32) {
        self.context.tex_parameteri(target, pname, param);
    }

    fn tex_storage_2d(&self, target: u32, levels: i32, internalformat: u32, width: i32, height: i32) {
        self.context.tex_storage_2d(target, levels, internalformat, width, height);
    }

    fn tex_storage_3d(&self, target: u32, levels: i32, internalformat: u32, width: i32, height: i32, depth: i32) {
        self.context.tex_storage_3d(target, levels, internalformat, width, height, depth);
    }

    fn tex_sub_image_2d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, width: i32, height: i32, format: u32, type_: u32, pixels: PixelUnpack<'_>) {
        let result = match pixels {
            PixelUnpack::Bytes(bytes) => {
                let Some(view) = pixel_view(type_, bytes) else {
                    self.refuse(gl::INVALID_ENUM);
                    return;
                };
                self.context.tex_sub_image_2d_with_i32_and_i32_and_u32_and_type_and_opt_array_buffer_view(target, level, xoffset, yoffset, width, height, format, type_, Some(&view))
            }
            PixelUnpack::Offset(offset) => {
                self.context.tex_sub_image_2d_with_i32_and_i32_and_u32_and_type_and_f64(target, level, xoffset, yoffset, width, height, format, type_, offset as f64)
            }
        };
        self.thrown(result);
    }

    fn tex_sub_image_3d(&self, target: u32, level: i32, xoffset: i32, yoffset: i32, zoffset: i32, width: i32, height: i32, depth: i32, format: u32, type_: u32, pixels: PixelUnpack<'_>) {
        let result = match pixels {
            PixelUnpack::Bytes(bytes) => {
                let Some(view) = pixel_view(type_, bytes) else {
                    self.refuse(gl::INVALID_ENUM);
                    return;
                };
                self.context.tex_sub_image_3d_with_opt_array_buffer_view(target, level, xoffset, yoffset, zoffset, width, height, depth, format, type_, Some(&view))
            }
            PixelUnpack::Offset(offset) => {
                self.context.tex_sub_image_3d_with_f64(target, level, xoffset, yoffset, zoffset, width, height, depth, format, type_, offset as f64)
            }
        };
        self.thrown(result);
    }

    fn transform_feedback_varyings(&self, program: u32, varyings: &[&CStr], buffer_mode: u32) {
        let Some(program) = self.program(program) else { return };
        let names: Option<Array> = varyings.iter().map(|name| name.to_str().ok().map(JsValue::from_str)).collect();
        match names {
            Some(names) => self.context.transform_feedback_varyings(&program, &names, buffer_mode),
            None => self.refuse(gl::INVALID_VALUE),
        }
    }

    fn uniform1f(&self, location: i32, v0: f32) {
        self.uniform(location, |context, location| context.uniform1f(location, v0));
    }

    fn uniform1fv(&self, location: i32, value: &[f32]) {
        self.uniform(location, |context, location| context.uniform1fv_with_f32_array(location, value));
    }

    fn uniform1i(&self, location: i32, v0: i32) {
        self.uniform(location, |context, location| context.uniform1i(location, v0));
    }

    fn uniform1iv(&self, location: i32, value: &[i32]) {
        self.uniform(location, |context, location| context.uniform1iv_with_i32_array(location, value));
    }

    fn uniform1ui(&self, location: i32, v0: u32) {
        self.uniform(location, |context, location| context.uniform1ui(location, v0));
    }

    fn uniform1uiv(&self, location: i32, value: &[u32]) {
        self.uniform(location, |context, location| context.uniform1uiv_with_u32_array(location, value));
    }

    fn uniform2f(&self, location: i32, v0: f32, v1: f32) {
        self.uniform(location, |context, location| context.uniform2f(location, v0, v1));
    }

    fn uniform2fv(&self, location: i32, value: &[f32]) {
        self.uniform(location, |context, location| context.uniform2fv_with_f32_array(location, value));
    }

    fn uniform2iv(&self, location: i32, value: &[i32]) {
        self.uniform(location, |context, location| context.uniform2iv_with_i32_array(location, value));
    }

    fn uniform2uiv(&self, location: i32, value: &[u32]) {
        self.uniform(location, |context, location| context.uniform2uiv_with_u32_array(location, value));
    }

    fn uniform3fv(&self, location: i32, value: &[f32]) {
        self.uniform(location, |context, location| context.uniform3fv_with_f32_array(location, value));
    }

    fn uniform3i(&self, location: i32, v0: i32, v1: i32, v2: i32) {
        self.uniform(location, |context, location| context.uniform3i(location, v0, v1, v2));
    }

    fn uniform3iv(&self, location: i32, value: &[i32]) {
        self.uniform(location, |context, location| context.uniform3iv_with_i32_array(location, value));
    }

    fn uniform3uiv(&self, location: i32, value: &[u32]) {
        self.uniform(location, |context, location| context.uniform3uiv_with_u32_array(location, value));
    }

    fn uniform4fv(&self, location: i32, value: &[f32]) {
        self.uniform(location, |context, location| context.uniform4fv_with_f32_array(location, value));
    }

    fn uniform4iv(&self, location: i32, value: &[i32]) {
        self.uniform(location, |context, location| context.uniform4iv_with_i32_array(location, value));
    }

    fn uniform4uiv(&self, location: i32, value: &[u32]) {
        self.uniform(location, |context, location| context.uniform4uiv_with_u32_array(location, value));
    }

    fn uniform_block_binding(&self, program: u32, uniform_block_index: u32, uniform_block_binding: u32) {
        if let Some(program) = self.program(program) {
            self.context.uniform_block_binding(&program, uniform_block_index, uniform_block_binding);
        }
    }

    fn uniform_matrix2fv(&self, location: i32, transpose: bool, value: &[f32]) {
        self.uniform(location, |context, location| context.uniform_matrix2fv_with_f32_array(location, transpose, value));
    }

    fn uniform_matrix2x3fv(&self, location: i32, transpose: bool, value: &[f32]) {
        self.uniform(location, |context, location| context.uniform_matrix2x3fv_with_f32_array(location, transpose, value));
    }

    fn uniform_matrix2x4fv(&self, location: i32, transpose: bool, value: &[f32]) {
        self.uniform(location, |context, location| context.uniform_matrix2x4fv_with_f32_array(location, transpose, value));
    }

    fn uniform_matrix3fv(&self, location: i32, transpose: bool, value: &[f32]) {
        self.uniform(location, |context, location| context.uniform_matrix3fv_with_f32_array(location, transpose, value));
    }

    fn uniform_matrix3x2fv(&self, location: i32, transpose: bool, value: &[f32]) {
        self.uniform(location, |context, location| context.uniform_matrix3x2fv_with_f32_array(location, transpose, value));
    }

    fn uniform_matrix3x4fv(&self, location: i32, transpose: bool, value: &[f32]) {
        self.uniform(location, |context, location| context.uniform_matrix3x4fv_with_f32_array(location, transpose, value));
    }

    fn uniform_matrix4fv(&self, location: i32, transpose: bool, value: &[f32]) {
        self.uniform(location, |context, location| context.uniform_matrix4fv_with_f32_array(location, transpose, value));
    }

    fn uniform_matrix4x2fv(&self, location: i32, transpose: bool, value: &[f32]) {
        self.uniform(location, |context, location| context.uniform_matrix4x2fv_with_f32_array(location, transpose, value));
    }

    fn uniform_matrix4x3fv(&self, location: i32, transpose: bool, value: &[f32]) {
        self.uniform(location, |context, location| context.uniform_matrix4x3fv_with_f32_array(location, transpose, value));
    }

    fn use_program(&self, program: u32) {
        let object = if program == 0 { Some(None) } else { self.program(program).map(Some) };
        if let Some(object) = object {
            self.objects.borrow_mut().current_program = program;
            self.context.use_program(object.as_ref());
        }
    }

    fn vertex_attrib_divisor(&self, index: u32, divisor: u32) {
        self.context.vertex_attrib_divisor(index, divisor);
    }

    fn vertex_attrib_i_pointer(&self, index: u32, size: i32, type_: u32, stride: i32, pointer: u64) {
        self.context.vertex_attrib_i_pointer_with_f64(index, size, type_, stride, pointer as f64);
    }

    fn vertex_attrib_pointer(&self, index: u32, size: i32, type_: u32, normalized: bool, stride: i32, pointer: u64) {
        self.context.vertex_attrib_pointer_with_f64(index, size, type_, normalized, stride, pointer as f64);
    }

    fn viewport(&self, x: i32, y: i32, width: i32, height: i32) {
        self.context.viewport(x, y, width, height);
    }

    fn blend_equation_separatei_oes(&self, buf: u32, mode_rgb: u32, mode_alpha: u32) -> Option<()> {
        self.extension_call(DRAW_BUFFERS_INDEXED, "blendEquationSeparateiOES", &[buf.into(), mode_rgb.into(), mode_alpha.into()]).map(drop)
    }

    fn blend_func_separatei_oes(&self, buf: u32, src_rgb: u32, dst_rgb: u32, src_alpha: u32, dst_alpha: u32) -> Option<()> {
        self.extension_call(DRAW_BUFFERS_INDEXED, "blendFuncSeparateiOES", &[buf.into(), src_rgb.into(), dst_rgb.into(), src_alpha.into(), dst_alpha.into()]).map(drop)
    }

    fn clip_control_ext(&self, origin: u32, depth: u32) -> Option<()> {
        self.extension_call(CLIP_CONTROL, "clipControlEXT", &[origin.into(), depth.into()]).map(drop)
    }

    fn color_maski_oes(&self, index: u32, r: bool, g: bool, b: bool, a: bool) -> Option<()> {
        self.extension_call(DRAW_BUFFERS_INDEXED, "colorMaskiOES", &[index.into(), r.into(), g.into(), b.into(), a.into()]).map(drop)
    }

    fn disablei_oes(&self, target: u32, index: u32) -> Option<()> {
        self.extension_call(DRAW_BUFFERS_INDEXED, "disableiOES", &[target.into(), index.into()]).map(drop)
    }

    fn enablei_oes(&self, target: u32, index: u32) -> Option<()> {
        self.extension_call(DRAW_BUFFERS_INDEXED, "enableiOES", &[target.into(), index.into()]).map(drop)
    }

    fn get_query_objectui64v_ext(&self, id: u32, pname: u32, params: &mut [u64]) -> Option<()> {
        self.extension(TIMER_QUERY)?;
        if let Some(query) = self.named(|objects| &objects.queries, id) {
            let answer = self.numbers(&self.context.get_query_parameter(&query, pname));
            self.write_numbers(answer, params, |value| value as u64);
        }
        Some(())
    }

    fn get_queryiv_ext(&self, target: u32, pname: u32, params: &mut [i32]) -> Option<()> {
        self.extension(TIMER_QUERY)?;
        let answer = self.numbers(&self.context.get_query(target, pname));
        self.write_numbers(answer, params, to_i32);
        Some(())
    }

    fn polygon_mode_angle(&self, face: u32, mode: u32) -> Option<()> {
        self.extension_call(POLYGON_MODE, "polygonModeWEBGL", &[face.into(), mode.into()]).map(drop)
    }

    fn polygon_offset_clamp_ext(&self, factor: f32, units: f32, clamp: f32) -> Option<()> {
        self.extension_call(POLYGON_OFFSET_CLAMP, "polygonOffsetClampEXT", &[factor.into(), units.into(), clamp.into()]).map(drop)
    }

    fn provoking_vertex_angle(&self, provoke_mode: u32) -> Option<()> {
        self.extension_call(PROVOKING_VERTEX, "provokingVertexWEBGL", &[provoke_mode.into()]).map(drop)
    }

    fn query_counter_ext(&self, id: u32, target: u32) -> Option<()> {
        self.extension(TIMER_QUERY)?;
        if let Some(query) = self.named(|objects| &objects.queries, id) {
            self.extension_call(TIMER_QUERY, "queryCounterEXT", &[query.into(), target.into()]);
        }
        Some(())
    }

    fn map_buffer_range_read(&self, target: u32, offset: i64, data: &mut [u8]) -> bool {
        if offset < 0 {
            self.refuse(gl::INVALID_VALUE);
            return false;
        }
        // WebGL 2 copies with getBufferSubData (it has no mapping); an error it raised stays for get_error.
        self.context.get_buffer_sub_data_with_f64_and_u8_array(target, offset as f64, data);
        let error = self.context.get_error();
        if error != gl::NO_ERROR {
            self.record(error);
        }
        error == gl::NO_ERROR
    }
}
