//! Runs a GLSL program on llvmpipe, Mesa's OpenGL driver on the processor, through the pinned
//! build's EGL on its software device, with no surface: the reference for GLSL programs. A compute dispatch,
//! with default-block uniforms set by `glUniform*`, and uniform and storage blocks bound by name.

use glow::HasContext;
use khronos_egl as egl;
use std::ffi::{c_char, c_void};
use wcontainers_nir::mesa::{EGL, Stage};

/// `EGL_PLATFORM_DEVICE_EXT` (EGL_EXT_platform_device), which khronos-egl does not name.
const PLATFORM_DEVICE_EXT: egl::Enum = 0x313F;
/// The extension of the device that is Mesa's software renderer, llvmpipe.
const SOFTWARE_DEVICE: &str = "EGL_MESA_device_software";
/// The most devices looked at.
const DEVICES: usize = 16;
/// The OpenGL llvmpipe gives in the compatibility profile: 4.5.
const GL_MAJOR: egl::Int = 4;
const GL_MINOR: egl::Int = 5;

pub type Error = String;

/// The EGL device that is Mesa's software renderer.
fn software_device(egl: &egl::DynamicInstance<egl::EGL1_5>) -> Result<*mut c_void, Error> {
    let function = |name: &str| {
        egl.get_proc_address(name)
            .ok_or_else(|| format!("llvmpipe: no {name}"))
    };
    // SAFETY: the addresses are EGL_EXT_device_enumeration's and EGL_EXT_device_query's functions,
    // of the types given.
    let (query_devices, query_string) = unsafe {
        (
            std::mem::transmute::<extern "system" fn(), QueryDevices>(function(
                "eglQueryDevicesEXT",
            )?),
            std::mem::transmute::<extern "system" fn(), QueryDeviceString>(function(
                "eglQueryDeviceStringEXT",
            )?),
        )
    };
    let mut devices = [std::ptr::null_mut(); DEVICES];
    let mut count = 0;
    // SAFETY: room for `DEVICES` devices.
    let listed = unsafe {
        query_devices(
            egl::Int::try_from(DEVICES).unwrap_or(egl::Int::MAX),
            devices.as_mut_ptr(),
            &mut count,
        )
    };
    if listed != egl::TRUE {
        return Err("llvmpipe: eglQueryDevicesEXT failed".into());
    }
    devices[..usize::try_from(count).unwrap_or(0)]
        .iter()
        .copied()
        .find(|&device| {
            // SAFETY: a device EGL listed; the string lives as long as the device.
            let extensions = unsafe { query_string(device, egl::EXTENSIONS) };
            !extensions.is_null()
                // SAFETY: EGL's NUL-terminated extension string.
                && unsafe { std::ffi::CStr::from_ptr(extensions) }
                    .to_string_lossy()
                    .split_whitespace()
                    .any(|e| e == SOFTWARE_DEVICE)
        })
        .ok_or_else(|| "llvmpipe: no software device".into())
}

/// One active resource of a program and what the comparison needs of it (`glGetProgramResourceiv`).
#[derive(Clone, Debug)]
pub struct Variable {
    pub name: String,
    pub ty: u32,
    pub array_size: i32,
    pub offset: i32,
    pub block: i32,
    pub array_stride: i32,
    pub matrix_stride: i32,
    pub row_major: bool,
    /// Storage blocks' members: the outermost array's size (0 runtime-sized) and stride.
    pub top_level_size: i32,
    pub top_level_stride: i32,
}

/// A uniform or storage block: its name and size.
#[derive(Clone, Debug)]
pub struct Block {
    pub name: String,
    pub size: u32,
}

/// What a linked program declares.
#[derive(Clone, Debug, Default)]
pub struct Reflection {
    /// Every active uniform, block members (`block` not -1) among them.
    pub uniforms: Vec<Variable>,
    pub uniform_blocks: Vec<Block>,
    pub storage_blocks: Vec<Block>,
    /// Every storage block member.
    pub buffer_variables: Vec<Variable>,
}

/// The values of one default-block uniform: its words, element after element, column after column.
pub struct UniformValue<'a> {
    pub variable: &'a Variable,
    pub words: Vec<u32>,
}

type QueryDevices = unsafe extern "C" fn(egl::Int, *mut *mut c_void, *mut egl::Int) -> egl::Boolean;
type QueryDeviceString = unsafe extern "C" fn(*mut c_void, egl::Int) -> *const c_char;
type GetProgramInterfaceiv = unsafe extern "C" fn(u32, u32, u32, *mut i32);
type GetProgramResourceiv =
    unsafe extern "C" fn(u32, u32, u32, i32, *const u32, i32, *mut i32, *mut i32);
type GetProgramResourceName = unsafe extern "C" fn(u32, u32, u32, i32, *mut i32, *mut c_char);

pub struct Llvmpipe {
    egl: egl::DynamicInstance<egl::EGL1_5>,
    display: egl::Display,
    context: egl::Context,
    gl: glow::Context,
    get_program_interface: GetProgramInterfaceiv,
    get_program_resource: GetProgramResourceiv,
    get_program_resource_name: GetProgramResourceName,
}

impl Drop for Llvmpipe {
    fn drop(&mut self) {
        let _ = self.egl.make_current(self.display, None, None, None);
        let _ = self.egl.destroy_context(self.display, self.context);
        let _ = self.egl.terminate(self.display);
    }
}

fn shader_type(stage: Stage) -> u32 {
    match stage {
        Stage::Vertex => glow::VERTEX_SHADER,
        Stage::Fragment => glow::FRAGMENT_SHADER,
        Stage::Compute => glow::COMPUTE_SHADER,
    }
}

impl Llvmpipe {
    /// An OpenGL compatibility context on llvmpipe, current on this thread, with no surface.
    pub fn open() -> Result<Self, Error> {
        // SAFETY: Mesa's libEGL is a plain EGL library; loading it runs only its constructors.
        let egl = unsafe { egl::DynamicInstance::<egl::EGL1_5>::load_required_from_filename(EGL) }
            .map_err(|e| format!("llvmpipe: loading {EGL}: {e}"))?;
        let device = software_device(&egl)?;
        // SAFETY: the device platform takes a device EGL gave and no attributes.
        let display =
            unsafe { egl.get_platform_display(PLATFORM_DEVICE_EXT, device, &[egl::ATTRIB_NONE]) }
                .map_err(|e| format!("llvmpipe: eglGetPlatformDisplay: {e}"))?;
        egl.initialize(display)
            .map_err(|e| format!("llvmpipe: eglInitialize: {e}"))?;
        egl.bind_api(egl::OPENGL_API)
            .map_err(|e| format!("llvmpipe: eglBindAPI: {e}"))?;
        let config = egl
            .choose_first_config(
                display,
                // EGL's default surface type is a window, which a device has none of.
                &[
                    egl::RENDERABLE_TYPE,
                    egl::OPENGL_BIT,
                    egl::SURFACE_TYPE,
                    egl::PBUFFER_BIT,
                    egl::NONE,
                ],
            )
            .map_err(|e| format!("llvmpipe: eglChooseConfig: {e}"))?
            .ok_or_else(|| {
                format!(
                    "llvmpipe: no OpenGL configuration among {:?} ({:?})",
                    egl.get_config_count(display),
                    egl.query_string(Some(display), egl::CLIENT_APIS)
                )
            })?;
        let context = egl
            .create_context(
                display,
                config,
                None,
                &[
                    egl::CONTEXT_MAJOR_VERSION,
                    GL_MAJOR,
                    egl::CONTEXT_MINOR_VERSION,
                    GL_MINOR,
                    egl::CONTEXT_OPENGL_PROFILE_MASK,
                    egl::CONTEXT_OPENGL_COMPATIBILITY_PROFILE_BIT,
                    egl::NONE,
                ],
            )
            .map_err(|e| format!("llvmpipe: eglCreateContext: {e}"))?;
        egl.make_current(display, None, None, Some(context))
            .map_err(|e| format!("llvmpipe: eglMakeCurrent: {e}"))?;
        let address = |name: &str| {
            egl.get_proc_address(name)
                .map_or(std::ptr::null(), |f| f as *const c_void)
        };
        // SAFETY: the context is current; EGL gives every OpenGL function's address.
        let gl = unsafe { glow::Context::from_loader_function(address) };
        let raw = |name: &str| {
            let f = address(name);
            (!f.is_null())
                .then_some(f)
                .ok_or_else(|| format!("llvmpipe: no {name}"))
        };
        // SAFETY: each address is the named OpenGL 4.3 function, of the type given.
        // (glow's glGetProgramResourceiv asks for the values' count with no room, and gets none.)
        let (get_program_interface, get_program_resource, get_program_resource_name) = unsafe {
            (
                std::mem::transmute::<*const c_void, GetProgramInterfaceiv>(raw(
                    "glGetProgramInterfaceiv",
                )?),
                std::mem::transmute::<*const c_void, GetProgramResourceiv>(raw(
                    "glGetProgramResourceiv",
                )?),
                std::mem::transmute::<*const c_void, GetProgramResourceName>(raw(
                    "glGetProgramResourceName",
                )?),
            )
        };
        Ok(Self {
            egl,
            display,
            context,
            gl,
            get_program_interface,
            get_program_resource,
            get_program_resource_name,
        })
    }

    /// The program of `sources`, compiled and linked, or the driver's log.
    pub fn link(&self, sources: &[(Stage, &str)]) -> Result<glow::Program, Error> {
        let gl = &self.gl;
        // SAFETY: plain OpenGL calls on the current context, on objects made here.
        unsafe {
            let program = gl.create_program()?;
            let mut log = String::new();
            for &(stage, text) in sources {
                let shader = gl.create_shader(shader_type(stage))?;
                gl.shader_source(shader, text);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    log += &gl.get_shader_info_log(shader);
                }
                gl.attach_shader(program, shader);
                gl.delete_shader(shader);
            }
            if log.is_empty() {
                gl.link_program(program);
                if gl.get_program_link_status(program) {
                    return Ok(program);
                }
                log += &gl.get_program_info_log(program);
            }
            gl.delete_program(program);
            Err(format!("llvmpipe refused the program: {log}"))
        }
    }

    fn resources(&self, program: glow::Program, interface: u32) -> Vec<String> {
        let id = program.0.get();
        let (mut count, mut longest) = (0, 0);
        // SAFETY: OpenGL 4.3 queries on a linked program, into locals of the right size.
        unsafe {
            (self.get_program_interface)(id, interface, glow::ACTIVE_RESOURCES, &mut count);
            (self.get_program_interface)(id, interface, glow::MAX_NAME_LENGTH, &mut longest);
        }
        (0..u32::try_from(count).unwrap_or(0))
            .map(|index| {
                let mut name = vec![0u8; usize::try_from(longest).unwrap_or(0) + 1];
                let mut length = 0;
                // SAFETY: `name` holds the longest name and its NUL.
                unsafe {
                    (self.get_program_resource_name)(
                        id,
                        interface,
                        index,
                        i32::try_from(name.len()).unwrap_or(i32::MAX),
                        &mut length,
                        name.as_mut_ptr().cast(),
                    );
                }
                name.truncate(usize::try_from(length).unwrap_or(0));
                String::from_utf8_lossy(&name).into_owned()
            })
            .collect()
    }

    /// The values of `properties` of one active resource, one a property.
    fn resource(
        &self,
        program: glow::Program,
        interface: u32,
        index: u32,
        properties: &[u32],
    ) -> Vec<i32> {
        let mut values = vec![0; properties.len()];
        let count = i32::try_from(properties.len()).unwrap_or(i32::MAX);
        // SAFETY: room for one value a property, each of which gives one.
        unsafe {
            (self.get_program_resource)(
                program.0.get(),
                interface,
                index,
                count,
                properties.as_ptr(),
                count,
                std::ptr::null_mut(),
                values.as_mut_ptr(),
            );
        }
        values
    }

    fn variables(&self, program: glow::Program, interface: u32) -> Vec<Variable> {
        let storage = interface == glow::BUFFER_VARIABLE;
        let mut properties = vec![
            glow::TYPE,
            glow::ARRAY_SIZE,
            glow::OFFSET,
            glow::BLOCK_INDEX,
            glow::ARRAY_STRIDE,
            glow::MATRIX_STRIDE,
            glow::IS_ROW_MAJOR,
        ];
        if storage {
            properties.extend([glow::TOP_LEVEL_ARRAY_SIZE, glow::TOP_LEVEL_ARRAY_STRIDE]);
        }
        let names = self.resources(program, interface);
        (0..)
            .zip(names)
            .map(|(index, name)| {
                let v = self.resource(program, interface, index, &properties);
                let [
                    ty,
                    array_size,
                    offset,
                    block,
                    array_stride,
                    matrix_stride,
                    row_major,
                    ..,
                ] = v[..]
                else {
                    unreachable!("one value a property")
                };
                Variable {
                    name,
                    ty: u32::try_from(ty).unwrap_or(glow::NONE),
                    array_size,
                    offset,
                    block,
                    array_stride,
                    matrix_stride,
                    row_major: row_major != 0,
                    top_level_size: v
                        .get(properties.len() - 2)
                        .copied()
                        .filter(|_| storage)
                        .unwrap_or(0),
                    top_level_stride: v
                        .get(properties.len() - 1)
                        .copied()
                        .filter(|_| storage)
                        .unwrap_or(0),
                }
            })
            .collect()
    }

    fn blocks(&self, program: glow::Program, interface: u32) -> Vec<Block> {
        let names = self.resources(program, interface);
        (0..)
            .zip(names)
            .map(|(index, name)| {
                let size = self.resource(program, interface, index, &[glow::BUFFER_DATA_SIZE]);
                Block {
                    name,
                    size: u32::try_from(size[0]).unwrap_or(0),
                }
            })
            .collect()
    }

    pub fn reflect(&self, program: glow::Program) -> Reflection {
        Reflection {
            uniforms: self.variables(program, glow::UNIFORM),
            uniform_blocks: self.blocks(program, glow::UNIFORM_BLOCK),
            storage_blocks: self.blocks(program, glow::SHADER_STORAGE_BLOCK),
            buffer_variables: self.variables(program, glow::BUFFER_VARIABLE),
        }
    }

    /// Sets each default-block uniform with `glUniform*`.
    fn set_uniforms(&self, program: glow::Program, values: &[UniformValue]) -> Result<(), Error> {
        let gl = &self.gl;
        for value in values {
            let v = value.variable;
            // SAFETY: the program is current and linked; the slices hold the uniform's elements.
            unsafe {
                let at = gl.get_uniform_location(program, &v.name);
                let Some(at) = at.as_ref() else {
                    return Err(format!("no location for the uniform {}", v.name));
                };
                let floats: Vec<f32> = value.words.iter().map(|w| f32::from_bits(*w)).collect();
                let ints: Vec<i32> = value.words.iter().map(|w| w.cast_signed()).collect();
                let u = &value.words;
                match v.ty {
                    glow::FLOAT => gl.uniform_1_f32_slice(Some(at), &floats),
                    glow::FLOAT_VEC2 => gl.uniform_2_f32_slice(Some(at), &floats),
                    glow::FLOAT_VEC3 => gl.uniform_3_f32_slice(Some(at), &floats),
                    glow::FLOAT_VEC4 => gl.uniform_4_f32_slice(Some(at), &floats),
                    glow::INT | glow::BOOL => gl.uniform_1_i32_slice(Some(at), &ints),
                    glow::INT_VEC2 | glow::BOOL_VEC2 => gl.uniform_2_i32_slice(Some(at), &ints),
                    glow::INT_VEC3 | glow::BOOL_VEC3 => gl.uniform_3_i32_slice(Some(at), &ints),
                    glow::INT_VEC4 | glow::BOOL_VEC4 => gl.uniform_4_i32_slice(Some(at), &ints),
                    glow::UNSIGNED_INT => gl.uniform_1_u32_slice(Some(at), u),
                    glow::UNSIGNED_INT_VEC2 => gl.uniform_2_u32_slice(Some(at), u),
                    glow::UNSIGNED_INT_VEC3 => gl.uniform_3_u32_slice(Some(at), u),
                    glow::UNSIGNED_INT_VEC4 => gl.uniform_4_u32_slice(Some(at), u),
                    glow::FLOAT_MAT2 => gl.uniform_matrix_2_f32_slice(Some(at), false, &floats),
                    glow::FLOAT_MAT3 => gl.uniform_matrix_3_f32_slice(Some(at), false, &floats),
                    glow::FLOAT_MAT4 => gl.uniform_matrix_4_f32_slice(Some(at), false, &floats),
                    glow::FLOAT_MAT2x3 => {
                        gl.uniform_matrix_2x3_f32_slice(Some(at), false, &floats);
                    }
                    glow::FLOAT_MAT2x4 => {
                        gl.uniform_matrix_2x4_f32_slice(Some(at), false, &floats);
                    }
                    glow::FLOAT_MAT3x2 => {
                        gl.uniform_matrix_3x2_f32_slice(Some(at), false, &floats);
                    }
                    glow::FLOAT_MAT3x4 => {
                        gl.uniform_matrix_3x4_f32_slice(Some(at), false, &floats);
                    }
                    glow::FLOAT_MAT4x2 => {
                        gl.uniform_matrix_4x2_f32_slice(Some(at), false, &floats);
                    }
                    glow::FLOAT_MAT4x3 => {
                        gl.uniform_matrix_4x3_f32_slice(Some(at), false, &floats);
                    }
                    other => return Err(format!("a uniform of type {other:#x} is not set here")),
                }
            }
        }
        Ok(())
    }

    fn buffer(&self, target: u32, index: u32, bytes: &[u8]) -> Result<glow::Buffer, Error> {
        let gl = &self.gl;
        // SAFETY: a new buffer filled from `bytes` and bound at `index` of `target`.
        unsafe {
            let buffer = gl.create_buffer()?;
            gl.bind_buffer(target, Some(buffer));
            gl.buffer_data_u8_slice(target, bytes, glow::DYNAMIC_COPY);
            gl.bind_buffer_base(target, index, Some(buffer));
            Ok(buffer)
        }
    }

    /// Dispatches `groups` of the compute program with the given uniforms and blocks (block i
    /// bound at i), and gives each storage block's bytes afterwards.
    pub fn compute(
        &self,
        program: glow::Program,
        uniforms: &[UniformValue],
        uniform_blocks: &[Vec<u8>],
        storage_blocks: &[Vec<u8>],
        groups: [u32; 3],
    ) -> Result<Vec<Vec<u8>>, Error> {
        let gl = &self.gl;
        // SAFETY: plain OpenGL calls on the current context, on objects made here.
        unsafe {
            gl.use_program(Some(program));
            self.set_uniforms(program, uniforms)?;
            let mut made = Vec::new();
            for (i, bytes) in (0..).zip(uniform_blocks) {
                gl.uniform_block_binding(program, i, i);
                made.push(self.buffer(glow::UNIFORM_BUFFER, i, bytes)?);
            }
            let mut storage = Vec::new();
            for (i, bytes) in (0..).zip(storage_blocks) {
                gl.shader_storage_block_binding(program, i, i);
                storage.push(self.buffer(glow::SHADER_STORAGE_BUFFER, i, bytes)?);
            }
            gl.dispatch_compute(groups[0], groups[1], groups[2]);
            gl.memory_barrier(glow::ALL_BARRIER_BITS);
            let mut out = Vec::new();
            for (buffer, bytes) in storage.iter().zip(storage_blocks) {
                let mut read = vec![0; bytes.len()];
                gl.bind_buffer(glow::SHADER_STORAGE_BUFFER, Some(*buffer));
                gl.get_buffer_sub_data(glow::SHADER_STORAGE_BUFFER, 0, &mut read);
                out.push(read);
            }
            let error = gl.get_error();
            for buffer in made.into_iter().chain(storage) {
                gl.delete_buffer(buffer);
            }
            if error != glow::NO_ERROR {
                return Err(format!("llvmpipe: OpenGL error {error:#x}"));
            }
            Ok(out)
        }
    }
}
