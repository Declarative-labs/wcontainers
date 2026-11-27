//! What each Khronos API crate carries (hand-written: the commands of the census, the rule's exception lines and
//! the reasons). Every command of a set is classified by src/khronos/classify.rs; a parameter that no rule and no
//! line here maps fails the generator.

/// How a parameter that the registry's own attributes do not make safe is carried, by an exception line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// An out-array whose length the registry gives as `COMPSIZE(pname)`: the glue passes a zeroed buffer of
    /// `guard` values of its own, refuses the listed values of `by`, and copies out as many as the caller's slice
    /// holds.
    GuardedOut { by: &'static str, guard: usize, refused: &'static [&'static str] },
    /// An in-array whose length is `COMPSIZE(..)`: the glue copies at most `most` values into a zeroed array of
    /// `most` and passes that.
    BoundedIn { most: usize },
    /// Pixels read by the call: bytes of the caller's, sized by the unpack state, or an offset into the bound
    /// pixel unpack buffer. `depth` names the third dimension's parameter when there is one.
    PixelUnpack { depth: bool },
    /// Pixels written by the call: bytes of the caller's, sized by the pack state, or an offset into the bound
    /// pixel pack buffer.
    PixelPack,
    /// Compressed bytes read by the call (their count is the size parameter), or an offset and a size into the
    /// bound pixel unpack buffer.
    CompressedUnpack,
    /// An offset into the bound array buffer (a vertex attribute's pointer): refused when no array buffer is bound.
    ArrayBufferOffset,
    /// An offset into the bound element array buffer: refused when none is bound.
    ElementBufferOffset,
    /// Bytes that may be absent (the store is then made and left undefined), of the given size.
    OptionalBytes,
    /// An EGL attribute list: copied, ended at its first `EGL_NONE` key, refused when a key has no value, and
    /// ended with `EGL_NONE` by the glue.
    AttributeList,
    /// One value the call writes (EGL marks no length).
    SingleOut,
    /// Handles the call writes, as many as `count` says room for; `written` is how many it wrote.
    HandlesOut { count: &'static str, written: &'static str },
    /// A handle the caller may leave out (`EGL_NO_CONTEXT`, `EGL_NO_SURFACE`, `EGL_NO_DISPLAY`).
    OptionalHandle,
    /// A native handle passed only as null (`EGL_DEFAULT_DISPLAY`): left out of the trait.
    NullOnly,
}

/// A value the glue forces into an attribute list whatever the caller asked, with its reason and its test.
#[derive(Clone, Copy, Debug)]
pub struct Forced {
    pub command: &'static str,
    pub attribute: &'static str,
    pub value: &'static str,
    pub reason: &'static str,
    pub test: &'static str,
}

/// A command carried by a method of the generator's own making, from calls of the registry.
#[derive(Clone, Copy, Debug)]
pub struct Composed {
    pub name: &'static str,
    pub calls: &'static [&'static str],
    pub reason: &'static str,
}

pub struct Set {
    /// The crate's folder under crates/.
    pub folder: &'static str,
    /// The trait's name.
    pub top: &'static str,
    pub summary: &'static str,
    /// The registry's API name (`gles2`, `egl`) and the newest core version whose values are written.
    pub api: &'static str,
    pub core_up_to: &'static str,
    /// The command prefix and the value prefix the names lose.
    pub command_prefix: &'static str,
    pub value_prefix: &'static str,
    /// The commands carried, by the registry's name.
    pub commands: &'static [&'static str],
    /// Commands loaded for the glue's own checks and not offered (or offered by another command).
    pub inside: &'static [(&'static str, &'static str)],
    /// Extensions whose values are written beside the core ones.
    pub extensions: &'static [&'static str],
    /// Values the registry declares that no feature or extension of the API requires, written by name, with the
    /// reason each is carried.
    pub values: &'static [(&'static str, &'static str)],
    pub forms: &'static [(&'static str, &'static str, Form)],
    pub forced: &'static [Forced],
    pub composed: &'static [Composed],
    /// Commands checked before every draw (`draws`): each enabled vertex attribute has a buffer.
    pub draws: &'static [&'static str],
    /// Commands of the API left out of this set by name, with the reason.
    pub left_out: &'static [(&'static str, &'static str)],
}

/// The value lists a `Get` refuses: their length is another query's answer, so no fixed room holds them.
const VARIABLE_LISTS: &[&str] = &["GL_COMPRESSED_TEXTURE_FORMATS", "GL_PROGRAM_BINARY_FORMATS", "GL_SHADER_BINARY_FORMATS"];

/// The room a `Get` of a fixed count writes into: no value of OpenGL ES 3.2's state tables (chapter 21) is longer
/// than 16 values (a 4x4 matrix), so 64 leaves room for any extension's.
const GET_GUARD: usize = 64;

pub const OPENGL_ES: Set = Set {
    folder: "opengl-es",
    top: "OpenGlEs",
    summary: "OpenGL ES 3, the Khronos graphics API the WebGL route draws through natively (ANGLE, Mesa), as a safe trait.",
    api: "gles2",
    core_up_to: "3.2",
    command_prefix: "gl",
    value_prefix: "GL_",
    commands: &[
        "glActiveTexture", "glAttachShader", "glBeginQuery", "glBeginTransformFeedback", "glBindBuffer", "glBindBufferBase",
        "glBindBufferRange", "glBindFramebuffer", "glBindRenderbuffer", "glBindSampler", "glBindTexture", "glBindVertexArray",
        "glBlendColor", "glBlendEquationSeparate", "glBlendFuncSeparate", "glBlitFramebuffer", "glBufferData",
        "glBufferSubData", "glCheckFramebufferStatus", "glClearBufferfi", "glClearBufferfv", "glClearBufferiv",
        "glClearBufferuiv", "glClientWaitSync", "glColorMask", "glCompileShader", "glCompressedTexSubImage2D",
        "glCompressedTexSubImage3D", "glCopyBufferSubData", "glCopyTexSubImage2D", "glCopyTexSubImage3D", "glCreateProgram",
        "glCreateShader", "glCullFace", "glDeleteBuffers", "glDeleteFramebuffers", "glDeleteProgram", "glDeleteQueries",
        "glDeleteRenderbuffers", "glDeleteSamplers", "glDeleteShader", "glDeleteSync", "glDeleteTextures",
        "glDeleteVertexArrays", "glDepthFunc", "glDepthMask", "glDepthRangef", "glDetachShader", "glDisable",
        "glDisableVertexAttribArray", "glDrawArrays", "glDrawArraysInstanced", "glDrawBuffers", "glDrawElementsInstanced",
        "glEnable", "glEnableVertexAttribArray", "glEndQuery", "glEndTransformFeedback", "glFenceSync", "glFinish", "glFlush",
        "glFramebufferRenderbuffer", "glFramebufferTexture2D", "glFramebufferTextureLayer", "glFrontFace", "glGenBuffers",
        "glGenFramebuffers", "glGenQueries", "glGenRenderbuffers", "glGenSamplers", "glGenTextures", "glGenVertexArrays",
        "glGenerateMipmap", "glGetBooleanv", "glGetError", "glGetFloatv", "glGetInteger64v", "glGetIntegerv",
        "glGetInternalformativ", "glGetProgramInfoLog", "glGetProgramiv", "glGetQueryObjectuiv", "glGetShaderInfoLog",
        "glGetShaderiv", "glGetString", "glGetStringi", "glGetSynciv", "glGetUniformBlockIndex", "glGetUniformLocation",
        "glGetVertexAttribiv", "glInvalidateFramebuffer", "glLinkProgram", "glPixelStorei", "glPolygonOffset", "glReadPixels",
        "glRenderbufferStorage", "glRenderbufferStorageMultisample", "glSamplerParameterf", "glSamplerParameteri", "glScissor",
        "glShaderSource", "glStencilFunc", "glStencilFuncSeparate", "glStencilMask", "glStencilOp", "glStencilOpSeparate", "glTexParameteri", "glTexStorage2D",
        "glTexStorage3D", "glTexSubImage2D", "glTexSubImage3D", "glTransformFeedbackVaryings", "glUniform1f", "glUniform1fv",
        "glUniform1i", "glUniform1iv", "glUniform1ui", "glUniform1uiv", "glUniform2f", "glUniform2fv", "glUniform2iv",
        "glUniform2uiv", "glUniform3fv", "glUniform3i", "glUniform3iv", "glUniform3uiv", "glUniform4fv", "glUniform4iv",
        "glUniform4uiv", "glUniformBlockBinding", "glUniformMatrix2fv", "glUniformMatrix2x3fv", "glUniformMatrix2x4fv",
        "glUniformMatrix3fv", "glUniformMatrix3x2fv", "glUniformMatrix3x4fv", "glUniformMatrix4fv", "glUniformMatrix4x2fv",
        "glUniformMatrix4x3fv", "glUseProgram", "glVertexAttribDivisor", "glVertexAttribIPointer", "glVertexAttribPointer",
        "glViewport",
        // Extensions (each offered only where the context names its extension and gives its address).
        "glBlendEquationSeparateiOES", "glBlendFuncSeparateiOES", "glClipControlEXT", "glColorMaskiOES", "glDisableiOES",
        "glEnableiOES", "glGetQueryObjectui64vEXT", "glGetQueryivEXT", "glPolygonModeANGLE", "glPolygonOffsetClampEXT",
        "glProvokingVertexANGLE", "glQueryCounterEXT",
    ],
    inside: &[
        ("glMapBufferRange", "lends memory until glUnmapBuffer: offered only inside map_buffer_range_read, which copies it out"),
        ("glUnmapBuffer", "ends the lending of glMapBufferRange: called only inside map_buffer_range_read"),
    ],
    extensions: &[
        "GL_EXT_clip_control", "GL_ANGLE_provoking_vertex", "GL_EXT_disjoint_timer_query", "GL_OES_draw_buffers_indexed",
        "GL_ANGLE_polygon_mode", "GL_EXT_polygon_offset_clamp", "GL_KHR_parallel_shader_compile",
        "GL_EXT_texture_compression_bptc", "GL_OES_texture_half_float", "GL_EXT_robustness", "GL_EXT_texture_compression_s3tc", "GL_EXT_texture_compression_rgtc",
        "GL_EXT_texture_format_BGRA8888", "GL_EXT_color_buffer_float", "GL_EXT_float_blend", "GL_OES_texture_float_linear",
        "GL_EXT_depth_clamp", "GL_KHR_texture_compression_astc_ldr", "GL_EXT_texture_norm16",
        // NEEDED-FROM-API-CRATES.md "After GI7" item 2: the values these four add, which the WebGL back end names.
        "GL_EXT_clip_cull_distance", "GL_EXT_texture_filter_anisotropic", "GL_EXT_texture_mirror_clamp_to_edge",
        "GL_EXT_blend_func_extended",
    ],
    values: &[(
        "GL_UNMASKED_RENDERER_WEBGL",
        "WebGL's WEBGL_debug_renderer_info value, which gl.xml declares and no OpenGL ES feature or extension requires: the browser route asks the renderer's name with it",
    )],
    forms: &[
        ("glGetBooleanv", "data", Form::GuardedOut { by: "pname", guard: GET_GUARD, refused: VARIABLE_LISTS }),
        ("glGetFloatv", "data", Form::GuardedOut { by: "pname", guard: GET_GUARD, refused: VARIABLE_LISTS }),
        ("glGetIntegerv", "data", Form::GuardedOut { by: "pname", guard: GET_GUARD, refused: VARIABLE_LISTS }),
        ("glGetInteger64v", "data", Form::GuardedOut { by: "pname", guard: GET_GUARD, refused: VARIABLE_LISTS }),
        // OpenGL ES 3.2 §7.12 and the extensions: the longest program value is COMPUTE_WORK_GROUP_SIZE's 3.
        ("glGetProgramiv", "params", Form::GuardedOut { by: "pname", guard: 16, refused: &[] }),
        ("glGetShaderiv", "params", Form::GuardedOut { by: "pname", guard: 16, refused: &[] }),
        ("glGetQueryObjectuiv", "params", Form::GuardedOut { by: "pname", guard: 16, refused: &[] }),
        ("glGetQueryObjectui64vEXT", "params", Form::GuardedOut { by: "pname", guard: 16, refused: &[] }),
        ("glGetQueryivEXT", "params", Form::GuardedOut { by: "pname", guard: 16, refused: &[] }),
        // The registry gives 4, which CURRENT_VERTEX_ATTRIB writes; the guard holds that and more.
        ("glGetVertexAttribiv", "params", Form::GuardedOut { by: "pname", guard: 16, refused: &[] }),
        // OpenGL ES 3.2 §15.2.3: a colour buffer reads four values, depth or stencil one.
        ("glClearBufferfv", "value", Form::BoundedIn { most: 4 }),
        ("glClearBufferiv", "value", Form::BoundedIn { most: 4 }),
        ("glClearBufferuiv", "value", Form::BoundedIn { most: 4 }),
        ("glTexSubImage2D", "pixels", Form::PixelUnpack { depth: false }),
        ("glTexSubImage3D", "pixels", Form::PixelUnpack { depth: true }),
        ("glReadPixels", "pixels", Form::PixelPack),
        ("glCompressedTexSubImage2D", "data", Form::CompressedUnpack),
        ("glCompressedTexSubImage3D", "data", Form::CompressedUnpack),
        ("glVertexAttribPointer", "pointer", Form::ArrayBufferOffset),
        ("glVertexAttribIPointer", "pointer", Form::ArrayBufferOffset),
        ("glDrawElementsInstanced", "indices", Form::ElementBufferOffset),
        ("glBufferData", "data", Form::OptionalBytes),
    ],
    forced: &[],
    composed: &[Composed {
        name: "map_buffer_range_read",
        calls: &["glMapBufferRange", "glUnmapBuffer"],
        reason: "glMapBufferRange returns memory lent until glUnmapBuffer; the method maps the range for reading only, copies it into the caller's bytes and unmaps, so nothing is lent past the call",
    }],
    draws: &["glDrawArrays", "glDrawArraysInstanced", "glDrawElementsInstanced"],
    left_out: &[],
};

pub const EGL: Set = Set {
    folder: "egl",
    top: "Egl",
    summary: "EGL, the Khronos interface between OpenGL ES and the platform (ANGLE's libEGL, Mesa's), as a safe trait.",
    api: "egl",
    core_up_to: "1.5",
    command_prefix: "egl",
    value_prefix: "EGL_",
    commands: &[
        "eglBindAPI", "eglChooseConfig", "eglCreateContext", "eglCreatePbufferSurface", "eglDestroyContext",
        "eglDestroySurface", "eglGetConfigAttrib", "eglGetDisplay", "eglGetError", "eglInitialize", "eglMakeCurrent",
        "eglQueryString", "eglQuerySurface", "eglSwapBuffers", "eglSwapInterval",
    ],
    inside: &[
        ("eglGetProcAddress", "loads the OpenGL ES entry points inside the OpenGL ES crate's glue; no pointer reaches safe code"),
        ("eglGetCurrentContext", "tells the OpenGL ES glue whether its context is current on the thread before each call"),
    ],
    values: &[],
    extensions: &["EGL_KHR_surfaceless_context", "EGL_ANGLE_direct_composition", "EGL_KHR_create_context", "EGL_EXT_create_context_robustness"],
    forms: &[
        ("eglChooseConfig", "attrib_list", Form::AttributeList),
        ("eglChooseConfig", "configs", Form::HandlesOut { count: "config_size", written: "num_config" }),
        ("eglCreateContext", "attrib_list", Form::AttributeList),
        ("eglCreateContext", "share_context", Form::OptionalHandle),
        ("eglCreatePbufferSurface", "attrib_list", Form::AttributeList),
        ("eglGetConfigAttrib", "value", Form::SingleOut),
        ("eglGetDisplay", "display_id", Form::NullOnly),
        ("eglInitialize", "major", Form::SingleOut),
        ("eglInitialize", "minor", Form::SingleOut),
        ("eglMakeCurrent", "draw", Form::OptionalHandle),
        ("eglMakeCurrent", "read", Form::OptionalHandle),
        ("eglMakeCurrent", "ctx", Form::OptionalHandle),
        ("eglQueryString", "dpy", Form::OptionalHandle),
        ("eglQuerySurface", "value", Form::SingleOut),
    ],
    forced: &[Forced {
        command: "eglCreateContext",
        attribute: "EGL_CONTEXT_OPENGL_ROBUST_ACCESS",
        value: "EGL_TRUE",
        reason: "OpenGL ES leaves a shader's or a draw's read outside a buffer undefined (it may end the program) unless the context has robust buffer access (EGL 1.5 §3.7.1.5); every context is made with it, and none is made where the implementation cannot",
        test: "robust_access_is_forced",
    }],
    composed: &[],
    draws: &[],
    left_out: &[
        ("eglCreateWindowSurface", "the native window is a loan the surface keeps: its shape is the owner's question (docs/status-graphics-implementations.md, a window given through the interface)"),
        ("eglTerminate", "the display is the process's, shared by every context; terminating it would end every other user's objects"),
    ],
};
