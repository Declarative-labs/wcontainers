//! The triangle scene drawn natively through wgpu, the WebGPU implementation the back ends build on.

use anyhow::{Context, Result};

use crate::Picture;
use crate::scene::{self, HEIGHT, WIDTH};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Bytes per row of the readback buffer, already the multiple of 256 copies need.
const PITCH: u32 = WIDTH * 4;
const _: () = assert!(PITCH.is_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT));

/// Draws the triangle on the first adapter of `backends`, which must be a GPU.
pub fn render_triangle(backends: wgpu::Backends) -> Result<Picture> {
    let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
    instance_desc.backends = backends;
    let instance = wgpu::Instance::new(instance_desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .context("no adapter")?;
    let info = adapter.get_info();
    anyhow::ensure!(
        info.device_type != wgpu::DeviceType::Cpu,
        "the adapter is the software {}, not a GPU",
        info.name
    );
    let adapter_name = format!(
        "{} ({:?}, {} {})",
        info.name, info.backend, info.driver, info.driver_info
    );
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(scene::TRIANGLE_WGSL.into()),
    });
    let attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3];
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: 20,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &attributes,
            })],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("ps"),
            compilation_options: Default::default(),
            targets: &[Some(FORMAT.into())],
        }),
        multiview_mask: None,
        cache: None,
    });

    let vertex_bytes: &[u8] = bytemuck::cast_slice(&scene::TRIANGLE);
    let vertices = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: vertex_bytes.len() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&vertices, 0, vertex_bytes);
    let size = wgpu::Extent3d {
        width: WIDTH,
        height: HEIGHT,
        depth_or_array_layers: 1,
    };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (PITCH * HEIGHT) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let [r, g, b, a] = scene::CLEAR.map(f64::from);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(PITCH),
                rows_per_image: None,
            },
        },
        size,
    );
    queue.submit([encoder.finish()]);

    let (sender, receiver) = std::sync::mpsc::channel();
    readback.map_async(wgpu::MapMode::Read, .., move |result| {
        let _ = sender.send(result);
    });
    device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    })?;
    receiver.recv()??;
    let rows = readback.get_mapped_range(..)?;
    let image = crate::picture_from_rows(WIDTH, HEIGHT, &rows, PITCH as usize);
    Ok(Picture {
        adapter: adapter_name,
        image,
    })
}
