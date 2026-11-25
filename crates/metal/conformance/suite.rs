//! The tests every implementation of wcontainers-metal runs, written against its traits alone.

use wcontainers_metal::{
    BlitCommand, BlitOption, ColorWriteMask, CommandBufferStatus, CommitError, CompareFunction, ComputeCommand,
    DepthStencilDescriptor, EncodeRenderCommandsError, FunctionSource, LoadAction, MAX_COLOR_ATTACHMENTS, Metal,
    Origin, PixelFormat, PrimitiveTopologyClass, PrimitiveType, ReadContentsError, RenderCommand,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineColorAttachmentDescriptor,
    RenderPipelineDescriptor, ResourceSlot, SamplerAddressMode, SamplerBorderColor, SamplerDescriptor,
    SamplerMinMagFilter, SamplerMipFilter, ScissorRect, Size, StoreAction, TextureDescriptor, TextureType,
    TextureUsage, TextureViewDescriptor, VertexAttribute, VertexBufferLayout, VertexFormat, VertexStepFunction,
    Viewport, WaitUntilCompletedError, WriteContentsError,
};

/// The side of the square textures the tests draw into and copy, in texels.
const SIDE: u32 = 4;

/// The bytes of one `rgba8Unorm` texel.
const TEXEL: u64 = 4;

/// The buffer slot of the tests' immediates, which no test reads.
const IMMEDIATES: u32 = 29;

/// The buffer slot of the lengths the bounds checks read.
const SIZES: u32 = 30;

/// The buffer slot of the tests' one vertex buffer.
const VERTICES: u32 = 20;

/// The tests' functions: a triangle whose color comes from a uniform, one sampling a texture, and a doubling compute.
const PROGRAMS: &str = "
struct Color { value: vec4<f32> }
@group(0) @binding(0) var<uniform> color: Color;
@group(0) @binding(16) var samp: sampler;
@group(0) @binding(32) var tex: texture_2d<f32>;
@group(0) @binding(48) var<storage, read_write> words: array<u32>;
@vertex fn vs(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p, 0.5, 1.0);
}
@fragment fn flat() -> @location(0) vec4<f32> {
    return color.value;
}
@fragment fn sampled(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, at.xy / 4.0);
}
@compute @workgroup_size(4) fn double(@builtin(global_invocation_id) id: vec3<u32>) {
    words[id.x] = words[id.x] * 2u;
}
";

/// A triangle covering every pixel centre of the target.
const COVERING: [f32; 6] = [-1.0, -1.0, 3.0, -1.0, -1.0, 3.0];

/// The bytes of the covering triangle's positions.
const COVERING_BYTES: u64 = 24;

/// The bytes of one color of four floats.
const COLOR_BYTES: u64 = 16;

/// The bytes of the words the compute test doubles.
const WORDS_BYTES: u64 = 32;

/// The slots of the tests' resources: the uniform, the sampler, the texture and the storage buffer.
const SLOTS: [ResourceSlot; 4] = [
    ResourceSlot { group: 0, binding: 0, index: 0 },
    ResourceSlot { group: 0, binding: 16, index: 0 },
    ResourceSlot { group: 0, binding: 32, index: 0 },
    ResourceSlot { group: 0, binding: 48, index: 1 },
];

/// Bytes no test writes by accident.
fn pattern(length: usize) -> Vec<u8> {
    (0..length).map(|i| (i as u8).wrapping_mul(37).wrapping_add(11)).collect()
}

/// Parses and validates the tests' functions.
fn programs() -> (naga::Module, naga::valid::ModuleInfo) {
    let module = naga::front::wgsl::parse_str(PROGRAMS).expect("the test functions parse");
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
        .validate(&module)
        .expect("the test functions validate");
    (module, info)
}

/// Compiles one entry point of the tests' functions.
fn function<M: Metal>(subject: &mut M, stage: naga::ShaderStage, entry_point: &str) -> M::Function {
    let (module, info) = programs();
    let vertex_buffers = [VertexBufferLayout {
        buffer_index: VERTICES,
        stride: 8,
        step_function: VertexStepFunction::PerVertex,
        attributes: vec![VertexAttribute { location: 0, offset: 0, format: VertexFormat::Float2 }],
    }];
    subject
        .make_function(&FunctionSource {
            module: &module,
            info: &info,
            stage,
            entry_point,
            resources: &SLOTS,
            immediates_buffer_index: IMMEDIATES,
            sizes_buffer_index: SIZES,
            vertex_buffers: &vertex_buffers,
            point_size: false,
        })
        .expect("make_function")
}

/// Makes an `rgba8Unorm` texture of `SIDE` by `SIDE` the tests draw into, copy and sample.
fn target<M: Metal>(subject: &mut M) -> M::Texture {
    subject
        .make_texture(&TextureDescriptor {
            texture_type: TextureType::Type2D,
            pixel_format: PixelFormat::Rgba8Unorm,
            width: SIDE,
            height: SIDE,
            depth: 1,
            mipmap_level_count: 1,
            sample_count: 1,
            array_length: 1,
            usage: TextureUsage { shader_read: true, shader_write: false, render_target: true, pixel_format_view: false },
        })
        .expect("make_texture")
}

/// Makes a buffer holding `bytes`.
fn filled<M: Metal>(subject: &mut M, bytes: &[u8]) -> M::Buffer {
    let buffer = subject.make_buffer(bytes.len() as u64).expect("make_buffer");
    subject.write_contents(&buffer, 0, bytes).expect("write_contents");
    buffer
}

/// Commits a command buffer, waits for it, and checks that it completed.
fn finish<M: Metal>(subject: &mut M, command_buffer: &M::CommandBuffer) {
    subject.commit(command_buffer).expect("commit");
    subject.wait_until_completed(command_buffer).expect("wait_until_completed");
    assert_eq!(subject.status(command_buffer), CommandBufferStatus::Completed);
}

/// Copies a `SIDE` by `SIDE` `rgba8Unorm` texture into a new buffer and returns its bytes.
fn read_texture<M: Metal>(subject: &mut M, texture: &M::Texture) -> Vec<u8> {
    let row = u64::from(SIDE) * TEXEL;
    let buffer = subject.make_buffer(row * u64::from(SIDE)).expect("make_buffer");
    let command_buffer = subject.make_command_buffer().expect("make_command_buffer");
    subject
        .encode_blit_commands(
            &command_buffer,
            &[BlitCommand::CopyFromTextureToBuffer {
                source_texture: texture.clone(),
                source_slice: 0,
                source_level: 0,
                source_origin: Origin { x: 0, y: 0, z: 0 },
                source_size: Size { width: SIDE, height: SIDE, depth: 1 },
                destination_buffer: buffer.clone(),
                destination_offset: 0,
                destination_bytes_per_row: row,
                destination_bytes_per_image: row * u64::from(SIDE),
                options: BlitOption::None,
            }],
        )
        .expect("encode_blit_commands");
    finish(subject, &command_buffer);
    let mut bytes = vec![0; (row * u64::from(SIDE)) as usize];
    subject.read_contents(&buffer, 0, &mut bytes).expect("read_contents");
    bytes
}

/// Makes a pipeline drawing `fragment` into one `rgba8Unorm` target.
fn pipeline<M: Metal>(subject: &mut M, fragment: &str) -> M::RenderPipelineState {
    let vertex = function(subject, naga::ShaderStage::Vertex, "vs");
    let fragment = function(subject, naga::ShaderStage::Fragment, fragment);
    let mut color_attachments = [None; MAX_COLOR_ATTACHMENTS];
    color_attachments[0] = Some(RenderPipelineColorAttachmentDescriptor {
        pixel_format: PixelFormat::Rgba8Unorm,
        blending: None,
        write_mask: ColorWriteMask { red: true, green: true, blue: true, alpha: true },
    });
    subject
        .make_render_pipeline_state(&RenderPipelineDescriptor {
            vertex_function: &vertex,
            fragment_function: Some(&fragment),
            color_attachments,
            depth_attachment_pixel_format: None,
            stencil_attachment_pixel_format: None,
            raster_sample_count: 1,
            alpha_to_coverage_enabled: false,
            rasterization_enabled: true,
            input_primitive_topology: PrimitiveTopologyClass::Triangle,
        })
        .expect("make_render_pipeline_state")
}

/// Returns a pass drawing into `texture`, cleared to transparent black.
fn pass<M: Metal>(texture: &M::Texture) -> RenderPassDescriptor<'_, M> {
    let mut color_attachments = [const { None }; MAX_COLOR_ATTACHMENTS];
    color_attachments[0] = Some(RenderPassColorAttachment {
        texture,
        level: 0,
        slice: 0,
        depth_plane: 0,
        load_action: LoadAction::Clear,
        store_action: StoreAction::Store,
        clear_color: [0.0; 4],
        resolve: None,
    });
    RenderPassDescriptor {
        color_attachments,
        depth_attachment: None,
        stencil_attachment: None,
        visibility_result_buffer: None,
        render_target_width: SIDE,
        render_target_height: SIDE,
        default_raster_sample_count: 1,
    }
}

/// Returns the commands that draw the covering triangle with `pipeline`, after `bindings`.
fn covering_draw<M: Metal>(
    pipeline: &M::RenderPipelineState,
    vertices: &M::Buffer,
    depth_stencil: &M::DepthStencilState,
    bindings: Vec<RenderCommand<M>>,
) -> Vec<RenderCommand<M>> {
    let mut commands = vec![
        RenderCommand::SetRenderPipelineState(pipeline.clone()),
        RenderCommand::SetDepthStencilState(depth_stencil.clone()),
        RenderCommand::SetVertexBuffer { buffer: vertices.clone(), offset: 0, length: COVERING_BYTES, index: VERTICES },
        RenderCommand::SetViewports(vec![Viewport {
            origin_x: 0.0,
            origin_y: 0.0,
            width: f64::from(SIDE),
            height: f64::from(SIDE),
            znear: 0.0,
            zfar: 1.0,
        }]),
        RenderCommand::SetScissorRects(vec![ScissorRect { x: 0, y: 0, width: SIDE, height: SIDE }]),
    ];
    commands.extend(bindings);
    commands.push(RenderCommand::DrawPrimitives {
        primitive_type: PrimitiveType::Triangle,
        vertex_start: 0,
        vertex_count: 3,
        instance_count: 1,
        base_instance: 0,
    });
    commands
}

/// Returns a depth and stencil state that tests nothing.
fn no_depth<M: Metal>(subject: &mut M) -> M::DepthStencilState {
    subject
        .make_depth_stencil_state(&DepthStencilDescriptor {
            depth_compare_function: CompareFunction::Always,
            depth_write_enabled: false,
            front_face_stencil: None,
            back_face_stencil: None,
        })
        .expect("make_depth_stencil_state")
}

/// Checks that the device has a name.
pub fn has_a_name<M: Metal>(subject: M) {
    assert!(!subject.name().is_empty());
}

/// Checks that bytes written into a buffer read back, and that ranges past its end are refused.
pub fn writes_and_reads_buffer_contents<M: Metal>(mut subject: M) {
    let bytes = pattern(64);
    let buffer = filled(&mut subject, &bytes);
    subject.write_contents(&buffer, 3, &[1, 2, 3]).expect("write_contents");
    let mut expected = bytes.clone();
    expected[3..6].copy_from_slice(&[1, 2, 3]);
    let mut read = vec![0; 64];
    subject.read_contents(&buffer, 0, &mut read).expect("read_contents");
    assert_eq!(read, expected);
    assert_eq!(subject.write_contents(&buffer, 62, &[0; 3]), Err(WriteContentsError::OutOfRange));
    assert_eq!(subject.read_contents(&buffer, 65, &mut [0; 1]), Err(ReadContentsError::OutOfRange));
}

/// Checks that a buffer an uncommitted command buffer uses is refused to the CPU, and a command buffer committed twice.
pub fn refuses_contents_of_uncommitted_work_and_a_second_commit<M: Metal>(mut subject: M) {
    let buffer = filled(&mut subject, &pattern(16));
    let command_buffer = subject.make_command_buffer().expect("make_command_buffer");
    assert_eq!(subject.wait_until_completed(&command_buffer), Err(WaitUntilCompletedError::NotCommitted));
    subject
        .encode_blit_commands(&command_buffer, &[BlitCommand::FillBuffer { buffer: buffer.clone(), offset: 0, length: 4, value: 9 }])
        .expect("encode_blit_commands");
    assert_eq!(subject.write_contents(&buffer, 0, &[0]), Err(WriteContentsError::UsedByUncommittedWork));
    finish(&mut subject, &command_buffer);
    assert_eq!(subject.commit(&command_buffer), Err(CommitError::AlreadyCommitted));
    let mut read = [0; 4];
    subject.read_contents(&buffer, 0, &mut read).expect("read_contents");
    assert_eq!(read, [9; 4]);
}

/// Checks that copies between buffers and a texture, and a fill, move exactly the bytes they name.
pub fn copies_and_fills_move_exact_bytes<M: Metal>(mut subject: M) {
    let row = u64::from(SIDE) * TEXEL;
    let bytes = pattern((row * u64::from(SIDE)) as usize);
    let source = filled(&mut subject, &bytes);
    let texture = target(&mut subject);
    let copy = filled(&mut subject, &[0; 32]);
    let command_buffer = subject.make_command_buffer().expect("make_command_buffer");
    subject
        .encode_blit_commands(
            &command_buffer,
            &[
                BlitCommand::CopyFromBufferToTexture {
                    source_buffer: source.clone(),
                    source_offset: 0,
                    source_bytes_per_row: row,
                    source_bytes_per_image: row * u64::from(SIDE),
                    source_size: Size { width: SIDE, height: SIDE, depth: 1 },
                    destination_texture: texture.clone(),
                    destination_slice: 0,
                    destination_level: 0,
                    destination_origin: Origin { x: 0, y: 0, z: 0 },
                    options: BlitOption::None,
                },
                BlitCommand::CopyFromBufferToBuffer {
                    source_buffer: source.clone(),
                    source_offset: 5,
                    destination_buffer: copy.clone(),
                    destination_offset: 1,
                    size: 7,
                },
                BlitCommand::FillBuffer { buffer: copy.clone(), offset: 20, length: 4, value: 0xab },
            ],
        )
        .expect("encode_blit_commands");
    finish(&mut subject, &command_buffer);
    assert_eq!(read_texture(&mut subject, &texture), bytes);
    let mut expected = vec![0; 32];
    expected[1..8].copy_from_slice(&bytes[5..12]);
    expected[20..24].fill(0xab);
    let mut read = vec![0; 32];
    subject.read_contents(&copy, 0, &mut read).expect("read_contents");
    assert_eq!(read, expected);
}

/// Checks that a triangle drawn in a uniform's color covers the target, and a draw missing its uniform is refused.
pub fn draws_in_a_uniform_color_and_refuses_an_empty_slot<M: Metal>(mut subject: M) {
    let state = pipeline(&mut subject, "flat");
    let depth_stencil = no_depth(&mut subject);
    let vertices = filled(&mut subject, &COVERING.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>());
    let color = filled(&mut subject, &[0.0f32, 1.0, 0.0, 1.0].iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>());
    let texture = target(&mut subject);
    let command_buffer = subject.make_command_buffer().expect("make_command_buffer");
    let missing = covering_draw(&state, &vertices, &depth_stencil, Vec::new());
    assert!(matches!(
        subject.encode_render_commands(&command_buffer, &pass(&texture), &missing),
        Err(EncodeRenderCommandsError::Unbound(_))
    ));
    let bound = vec![RenderCommand::SetFragmentBuffer { buffer: color, offset: 0, length: COLOR_BYTES, index: 0 }];
    let commands = covering_draw(&state, &vertices, &depth_stencil, bound);
    subject.encode_render_commands(&command_buffer, &pass(&texture), &commands).expect("encode_render_commands");
    finish(&mut subject, &command_buffer);
    assert_eq!(read_texture(&mut subject, &texture), [0, 255, 0, 255].repeat((SIDE * SIDE) as usize));
}

/// Checks that a draw sampling a view of a texture through a nearest sampler reads each texel.
pub fn samples_a_texture_view<M: Metal>(mut subject: M) {
    let row = u64::from(SIDE) * TEXEL;
    let bytes = pattern((row * u64::from(SIDE)) as usize);
    let source = filled(&mut subject, &bytes);
    let texture = target(&mut subject);
    let view = subject
        .make_texture_view(
            &texture,
            &TextureViewDescriptor {
                pixel_format: PixelFormat::Rgba8Unorm,
                texture_type: TextureType::Type2D,
                first_level: 0,
                level_count: 1,
                first_slice: 0,
                slice_count: 1,
            },
        )
        .expect("make_texture_view");
    let sampler = subject
        .make_sampler_state(&SamplerDescriptor {
            min_filter: SamplerMinMagFilter::Nearest,
            mag_filter: SamplerMinMagFilter::Nearest,
            mip_filter: SamplerMipFilter::NotMipmapped,
            s_address_mode: SamplerAddressMode::ClampToEdge,
            t_address_mode: SamplerAddressMode::ClampToEdge,
            r_address_mode: SamplerAddressMode::ClampToEdge,
            border_color: SamplerBorderColor::TransparentBlack,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            max_anisotropy: 1,
            compare_function: None,
        })
        .expect("make_sampler_state");
    let state = pipeline(&mut subject, "sampled");
    let depth_stencil = no_depth(&mut subject);
    let vertices = filled(&mut subject, &COVERING.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>());
    let drawn = target(&mut subject);
    let command_buffer = subject.make_command_buffer().expect("make_command_buffer");
    subject
        .encode_blit_commands(
            &command_buffer,
            &[BlitCommand::CopyFromBufferToTexture {
                source_buffer: source,
                source_offset: 0,
                source_bytes_per_row: row,
                source_bytes_per_image: row * u64::from(SIDE),
                source_size: Size { width: SIDE, height: SIDE, depth: 1 },
                destination_texture: texture,
                destination_slice: 0,
                destination_level: 0,
                destination_origin: Origin { x: 0, y: 0, z: 0 },
                options: BlitOption::None,
            }],
        )
        .expect("encode_blit_commands");
    let bound = vec![
        RenderCommand::SetFragmentTexture { texture: view, index: 0 },
        RenderCommand::SetFragmentSamplerState { sampler, index: 0 },
    ];
    let commands = covering_draw(&state, &vertices, &depth_stencil, bound);
    subject.encode_render_commands(&command_buffer, &pass(&drawn), &commands).expect("encode_render_commands");
    finish(&mut subject, &command_buffer);
    assert_eq!(read_texture(&mut subject, &drawn), bytes);
}

/// Checks that a compute function doubles every word of a storage buffer.
pub fn dispatches_a_compute_function<M: Metal>(mut subject: M) {
    let words: Vec<u32> = (1..=8).collect();
    let buffer = filled(&mut subject, &words.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>());
    let compute = function(&mut subject, naga::ShaderStage::Compute, "double");
    let state = subject.make_compute_pipeline_state(&compute).expect("make_compute_pipeline_state");
    let command_buffer = subject.make_command_buffer().expect("make_command_buffer");
    subject
        .encode_compute_commands(
            &command_buffer,
            &[
                ComputeCommand::SetComputePipelineState(state),
                ComputeCommand::SetBuffer { buffer: buffer.clone(), offset: 0, length: WORDS_BYTES, index: 1 },
                ComputeCommand::DispatchThreadgroups(Size { width: 2, height: 1, depth: 1 }),
            ],
        )
        .expect("encode_compute_commands");
    finish(&mut subject, &command_buffer);
    let mut read = vec![0; 32];
    subject.read_contents(&buffer, 0, &mut read).expect("read_contents");
    let doubled: Vec<u8> = words.iter().flat_map(|w| (w * 2).to_le_bytes()).collect();
    assert_eq!(read, doubled);
}

/// Runs each suite function above as its own test on a value made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn has_a_name() {
            $crate::has_a_name($value);
        }

        #[test]
        fn writes_and_reads_buffer_contents() {
            $crate::writes_and_reads_buffer_contents($value);
        }

        #[test]
        fn refuses_contents_of_uncommitted_work_and_a_second_commit() {
            $crate::refuses_contents_of_uncommitted_work_and_a_second_commit($value);
        }

        #[test]
        fn copies_and_fills_move_exact_bytes() {
            $crate::copies_and_fills_move_exact_bytes($value);
        }

        #[test]
        fn draws_in_a_uniform_color_and_refuses_an_empty_slot() {
            $crate::draws_in_a_uniform_color_and_refuses_an_empty_slot($value);
        }

        #[test]
        fn samples_a_texture_view() {
            $crate::samples_a_texture_view($value);
        }

        #[test]
        fn dispatches_a_compute_function() {
            $crate::dispatches_a_compute_function($value);
        }
    };
}
