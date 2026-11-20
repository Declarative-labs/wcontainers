//! Runs a GLSL program on llvmpipe, Mesa's OpenGL driver on the processor, through the pinned
//! build's EGL on its software device, with no surface: the reference for GLSL programs. A compute
//! dispatch, or a draw of points one a pixel of a row, with default-block uniforms set by
//! `glUniform*` and uniform and storage blocks bound by their index.

use crate::interface::Kind;
use glow::HasContext;
use khronos_egl as egl;
use std::ffi::{c_char, c_void};
use wcontainers_nir::mesa::{EGL, Stage};

/// `EGL_PLATFORM_DEVICE_EXT` (EGL_EXT_platform_device), which khronos-egl does not name.
const PLATFORM_DEVICE_EXT: egl::Enum = 0x313F;
/// `GL_CLAMP_VERTEX_COLOR` (the compatibility profile), which glow does not name.
const CLAMP_VERTEX_COLOR: u32 = 0x891A;
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

/// A stage input or output of a program (`GL_PROGRAM_INPUT`, `GL_PROGRAM_OUTPUT`); location -1
/// for OpenGL's own.
#[derive(Clone, Debug)]
pub struct StageValue {
    pub name: String,
    pub ty: u32,
    pub array_size: i32,
    pub location: i32,
}

/// One vertex attribute: its location, kind and components (a matrix is one a column), and every
/// vertex's values.
pub struct VertexAttribute {
    pub location: u32,
    pub kind: Kind,
    pub components: u32,
    pub words: Vec<u32>,
}

/// What a draw left: each colour target's texels (four 32-bit components each), the depth
/// target's, and each storage block's bytes.
#[derive(Debug, PartialEq)]
pub struct Drawn {
    pub targets: Vec<Vec<u8>>,
    pub depth: Vec<u8>,
    pub storage: Vec<Vec<u8>>,
}

/// What a draw takes.
pub struct DrawOptions<'a> {
    pub uniforms: &'a [UniformValue<'a>],
    pub uniform_blocks: &'a [Vec<u8>],
    pub storage_blocks: &'a [Vec<u8>],
    pub attributes: &'a [VertexAttribute],
    pub points: u32,
    /// The kind of the colour target at each location (None where there is none).
    pub targets: &'a [Option<Kind>],
    pub depth: bool,
    /// The bits every colour target texel starts as.
    pub clear: [u32; 4],
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
    pub inputs: Vec<StageValue>,
    pub outputs: Vec<StageValue>,
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
/// `glClipControl` and `glClampColor`.
type SetTwo = unsafe extern "C" fn(u32, u32);
/// `glProgramParameteri` and `glFramebufferParameteri`.
type Parameteri = unsafe extern "C" fn(u32, u32, i32);

pub struct Llvmpipe {
    egl: egl::DynamicInstance<egl::EGL1_5>,
    display: egl::Display,
    context: egl::Context,
    gl: glow::Context,
    get_program_interface: GetProgramInterfaceiv,
    get_program_resource: GetProgramResourceiv,
    get_program_resource_name: GetProgramResourceName,
    clip_control: SetTwo,
    clamp_color: SetTwo,
    program_parameter: Parameteri,
    framebuffer_parameter: Parameteri,
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
        // SAFETY: each address is the named OpenGL 4.1, 4.3 or 4.5 function, of the type given.
        // (glow's glGetProgramResourceiv asks for the values' count with no room, and gets none.)
        let (
            get_program_interface,
            get_program_resource,
            get_program_resource_name,
            clip_control,
            clamp_color,
            program_parameter,
            framebuffer_parameter,
        ) = unsafe {
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
                std::mem::transmute::<*const c_void, SetTwo>(raw("glClipControl")?),
                std::mem::transmute::<*const c_void, SetTwo>(raw("glClampColor")?),
                std::mem::transmute::<*const c_void, Parameteri>(raw("glProgramParameteri")?),
                std::mem::transmute::<*const c_void, Parameteri>(raw("glFramebufferParameteri")?),
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
            clip_control,
            clamp_color,
            program_parameter,
            framebuffer_parameter,
        })
    }

    /// The program of `sources`, compiled and linked, or the driver's log.
    pub fn link(&self, sources: &[(Stage, &str)]) -> Result<glow::Program, Error> {
        self.link_program(sources, false)
    }

    /// The program of `sources` linked as a separable program: every stage input and output kept.
    pub fn link_separable(&self, sources: &[(Stage, &str)]) -> Result<glow::Program, Error> {
        self.link_program(sources, true)
    }

    fn link_program(
        &self,
        sources: &[(Stage, &str)],
        separable: bool,
    ) -> Result<glow::Program, Error> {
        let gl = &self.gl;
        // SAFETY: plain OpenGL calls on the current context, on objects made here.
        unsafe {
            let program = gl.create_program()?;
            (self.program_parameter)(
                program.0.get(),
                glow::PROGRAM_SEPARABLE,
                i32::from(separable),
            );
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

    fn stage_values(&self, program: glow::Program, interface: u32) -> Vec<StageValue> {
        let properties = [glow::TYPE, glow::ARRAY_SIZE, glow::LOCATION];
        let names = self.resources(program, interface);
        (0..)
            .zip(names)
            .map(|(index, name)| {
                let [ty, array_size, location] =
                    self.resource(program, interface, index, &properties)[..]
                else {
                    unreachable!("one value a property")
                };
                StageValue {
                    name,
                    ty: u32::try_from(ty).unwrap_or(glow::NONE),
                    array_size,
                    location,
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
            inputs: self.stage_values(program, glow::PROGRAM_INPUT),
            outputs: self.stage_values(program, glow::PROGRAM_OUTPUT),
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

    /// Uses the program with its uniforms and blocks (block i bound at i); the buffers made, the
    /// storage blocks' last.
    fn bind_program(
        &self,
        program: glow::Program,
        uniforms: &[UniformValue],
        uniform_blocks: &[Vec<u8>],
        storage_blocks: &[Vec<u8>],
    ) -> Result<(Vec<glow::Buffer>, Vec<glow::Buffer>), Error> {
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
            Ok((made, storage))
        }
    }

    /// Each storage block's bytes after the work, then every buffer deleted; or OpenGL's error.
    fn finish(
        &self,
        made: Vec<glow::Buffer>,
        storage: Vec<glow::Buffer>,
        storage_blocks: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>, Error> {
        let gl = &self.gl;
        // SAFETY: plain OpenGL calls on the current context, on buffers made by `bind_program`.
        unsafe {
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
        let (made, storage) =
            self.bind_program(program, uniforms, uniform_blocks, storage_blocks)?;
        // SAFETY: a dispatch of the program in use.
        unsafe { self.gl.dispatch_compute(groups[0], groups[1], groups[2]) };
        self.finish(made, storage, storage_blocks)
    }

    /// Draws `points` points with the program, into colour targets one row of `points` texels
    /// (four 32-bit components each) and a 32-bit float depth target, window depth from 0 to 1
    /// as Vulkan's.
    #[allow(clippy::too_many_lines, reason = "one draw, step by step")]
    pub fn draw(&self, program: glow::Program, options: &DrawOptions) -> Result<Drawn, Error> {
        let gl = &self.gl;
        let (made, storage) = self.bind_program(
            program,
            options.uniforms,
            options.uniform_blocks,
            options.storage_blocks,
        )?;
        let width = i32::try_from(options.points).map_err(|e| e.to_string())?;
        // SAFETY: plain OpenGL calls on the current context, on objects made here.
        unsafe {
            let array = gl.create_vertex_array()?;
            gl.bind_vertex_array(Some(array));
            let mut vertex_buffers = Vec::new();
            for a in options.attributes {
                let bytes: Vec<u8> = a.words.iter().flat_map(|w| w.to_le_bytes()).collect();
                let buffer = gl.create_buffer()?;
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer));
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &bytes, glow::STATIC_DRAW);
                let size = i32::try_from(a.components).map_err(|e| e.to_string())?;
                match a.kind {
                    Kind::Float => {
                        gl.vertex_attrib_pointer_f32(a.location, size, glow::FLOAT, false, 0, 0);
                    }
                    Kind::Sint => gl.vertex_attrib_pointer_i32(a.location, size, glow::INT, 0, 0),
                    Kind::Uint => {
                        gl.vertex_attrib_pointer_i32(a.location, size, glow::UNSIGNED_INT, 0, 0);
                    }
                }
                gl.enable_vertex_attrib_array(a.location);
                vertex_buffers.push(buffer);
            }
            let framebuffer = gl.create_framebuffer()?;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
            // The size drawn at when the program writes no target.
            (self.framebuffer_parameter)(glow::FRAMEBUFFER, glow::FRAMEBUFFER_DEFAULT_WIDTH, width);
            (self.framebuffer_parameter)(glow::FRAMEBUFFER, glow::FRAMEBUFFER_DEFAULT_HEIGHT, 1);
            let mut textures = Vec::new();
            let mut draw_buffers = Vec::new();
            for (location, kind) in (0..).zip(options.targets) {
                let Some(kind) = *kind else {
                    draw_buffers.push(glow::NONE);
                    continue;
                };
                let internal = match kind {
                    Kind::Float => glow::RGBA32F,
                    Kind::Sint => glow::RGBA32I,
                    Kind::Uint => glow::RGBA32UI,
                };
                let texture = gl.create_texture()?;
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                gl.tex_storage_2d(glow::TEXTURE_2D, 1, internal, width, 1);
                let attachment = glow::COLOR_ATTACHMENT0 + location;
                gl.framebuffer_texture_2d(
                    glow::FRAMEBUFFER,
                    attachment,
                    glow::TEXTURE_2D,
                    Some(texture),
                    0,
                );
                draw_buffers.push(attachment);
                textures.push((location, attachment, kind, texture));
            }
            if options.depth {
                let texture = gl.create_texture()?;
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                gl.tex_storage_2d(glow::TEXTURE_2D, 1, glow::DEPTH_COMPONENT32F, width, 1);
                gl.framebuffer_texture_2d(
                    glow::FRAMEBUFFER,
                    glow::DEPTH_ATTACHMENT,
                    glow::TEXTURE_2D,
                    Some(texture),
                    0,
                );
                textures.push((0, glow::DEPTH_ATTACHMENT, Kind::Float, texture));
                gl.enable(glow::DEPTH_TEST);
                gl.depth_func(glow::ALWAYS);
                gl.depth_mask(true);
                gl.clear_buffer_f32_slice(glow::DEPTH, 0, &[0.0]);
            } else {
                gl.disable(glow::DEPTH_TEST);
            }
            gl.draw_buffers(&draw_buffers);
            let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
            if status != glow::FRAMEBUFFER_COMPLETE {
                return Err(format!("llvmpipe: framebuffer status {status:#x}"));
            }
            for &(location, attachment, kind, _) in &textures {
                if attachment == glow::DEPTH_ATTACHMENT {
                    continue;
                }
                match kind {
                    Kind::Float => {
                        let clear = options.clear.map(f32::from_bits);
                        gl.clear_buffer_f32_slice(glow::COLOR, location, &clear);
                    }
                    Kind::Sint => {
                        let clear = options.clear.map(u32::cast_signed);
                        gl.clear_buffer_i32_slice(glow::COLOR, location, &clear);
                    }
                    Kind::Uint => gl.clear_buffer_u32_slice(glow::COLOR, location, &options.clear),
                }
            }
            gl.viewport(0, 0, width, 1);
            (self.clip_control)(glow::LOWER_LEFT, glow::ZERO_TO_ONE);
            // OpenGL's vertex colour clamping is a draw-time variant of the driver's, which the
            // route does not take.
            (self.clamp_color)(CLAMP_VERTEX_COLOR, glow::FALSE.into());
            gl.enable(glow::PROGRAM_POINT_SIZE);
            gl.draw_arrays(glow::POINTS, 0, width);
            gl.memory_barrier(glow::ALL_BARRIER_BITS);
            let texels = usize::try_from(options.points).map_err(|e| e.to_string())?;
            let mut drawn = Drawn {
                targets: Vec::new(),
                depth: Vec::new(),
                storage: Vec::new(),
            };
            for &(_, attachment, kind, _) in &textures {
                let (format, ty, components) = match (attachment, kind) {
                    (glow::DEPTH_ATTACHMENT, _) => (glow::DEPTH_COMPONENT, glow::FLOAT, 1),
                    (_, Kind::Float) => (glow::RGBA, glow::FLOAT, options.clear.len()),
                    (_, Kind::Sint) => (glow::RGBA_INTEGER, glow::INT, options.clear.len()),
                    (_, Kind::Uint) => {
                        (glow::RGBA_INTEGER, glow::UNSIGNED_INT, options.clear.len())
                    }
                };
                let mut bytes = vec![0; texels * components * size_of::<u32>()];
                if attachment != glow::DEPTH_ATTACHMENT {
                    gl.read_buffer(attachment);
                }
                gl.read_pixels(
                    0,
                    0,
                    width,
                    1,
                    format,
                    ty,
                    glow::PixelPackData::Slice(Some(&mut bytes)),
                );
                if attachment == glow::DEPTH_ATTACHMENT {
                    drawn.depth = bytes;
                } else {
                    drawn.targets.push(bytes);
                }
            }
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.delete_framebuffer(framebuffer);
            for (_, _, _, texture) in textures {
                gl.delete_texture(texture);
            }
            for buffer in vertex_buffers {
                gl.delete_buffer(buffer);
            }
            gl.delete_vertex_array(array);
            drawn.storage = self.finish(made, storage, options.storage_blocks)?;
            Ok(drawn)
        }
    }
}
