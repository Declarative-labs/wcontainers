//! The per-draw cost of the WebGL 2 back end against the WebGPU back end, both built for WebAssembly
//! and run in one Chrome by browser/cost.mjs (cost.html): one scene, many small quads in one pass, each
//! draw naming its own uniform binding set or all naming one, drawn as strips or as indexed lists
//! that do not restart. The page times each frame's recording and submission.
#![cfg(target_arch = "wasm32")]

#[path = "common/kit.rs"]
#[allow(
    dead_code,
    unused_imports,
    reason = "the scene draws with part of the kit, which imports the whole interface"
)]
mod kit;

use kit::Kit;
use wasm_bindgen::prelude::*;
use wcontainers_gpu::{draw::*, format::*, resource::*, shader::*, state::*, *};
use wcontainers_gpu_webgl::GlConfig;
use wcontainers_opengl_es_webgl::WebGl;

/// The back end on the OpenGL ES crate's implementation over a canvas's WebGL 2 context.
type Gl = wcontainers_gpu_webgl::Gl<WebGl>;
use wcontainers_gpu_webgpu::{WebGpu, block_on};

/// A quad at the uniform's place, in its colour.
const QUAD: &str = "
struct Draw { at: vec4<f32>, colour: vec4<f32> }
@group(0) @binding(0) var<uniform> d: Draw;
@vertex fn vs(@builtin(vertex_index) v: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32(v & 1u), f32(v >> 1u)) * 0.0625;
    return vec4<f32>(d.at.xy + corner, 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return d.colour;
}
";

/// Distinct uniform ranges, one binding set each.
const SETS: u32 = 1024;
/// The side of the square target every draw covers, in texels.
const TARGET_SIDE: u32 = 256;

/// What a frame draws with, made once.
struct Scene<B: Gpu> {
    kit: Kit<B>,
    program: B::Program,
    target: B::TextureView,
    sets: Vec<B::BindingSet>,
    /// A quad's two triangles as 16-bit indices.
    indices: B::Buffer,
    viewport: [Viewport; 1],
}

impl<B: Gpu> Scene<B> {
    fn new(b: &mut B) -> Scene<B> {
        let kit = Kit::new(b);
        let program = kit::program(b, QUAD, "vs", "fs");
        let f = Format::Rgba8Unorm;
        let usage = TextureUsage::RENDER | TextureUsage::COPY_SRC;
        let t = kit::texture(
            b,
            TextureDimension::D2,
            f,
            [TARGET_SIDE, TARGET_SIDE, 1],
            1,
            usage,
            false,
        );
        let target = kit::target(b, &t, f, 0, 0);
        let stride = b.caps().limits.min_uniform_offset_alignment.max(32);
        let buffer = b
            .create_buffer(&BufferDesc {
                size: u64::from(SETS * stride),
                usage: BufferUsage::UNIFORM | BufferUsage::COPY_DST,
                intent: MapIntent::Rarely,
            })
            .expect("create_buffer");
        let mut bytes = vec![0u8; (SETS * stride) as usize];
        let mut sets = Vec::new();
        for i in 0..SETS {
            let (x, y) = ((i % 32) as f32 / 16.0 - 1.0, (i / 32) as f32 / 16.0 - 1.0);
            let c = (i % 7) as f32 / 7.0;
            let values = [x, y, 0.0, 0.0, c, 1.0 - c, 0.5, 1.0];
            for (k, v) in values.iter().enumerate() {
                let at = (i * stride) as usize + k * 4;
                bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
            }
            sets.push(
                b.create_binding_set(&[BindingEntry {
                    binding: 0,
                    stages: StageSet::VERTEX | StageSet::FRAGMENT,
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &buffer,
                        offset: u64::from(i * stride),
                        size: 32,
                    }),
                }])
                .expect("create_binding_set"),
            );
        }
        b.write_buffer(&buffer, 0, &bytes, WriteMode::Discard);
        let indices = b
            .create_buffer(&BufferDesc {
                size: 12,
                usage: BufferUsage::INDEX | BufferUsage::COPY_DST,
                intent: MapIntent::Rarely,
            })
            .expect("create_buffer");
        let quad: Vec<u8> = [0u16, 1, 2, 2, 1, 3]
            .iter()
            .flat_map(|i| i.to_le_bytes())
            .collect();
        b.write_buffer(&indices, 0, &quad, WriteMode::Ordered);
        Scene {
            kit,
            program,
            target,
            sets,
            indices,
            viewport: [kit::viewport(
                0.0,
                0.0,
                TARGET_SIDE as f32,
                TARGET_SIDE as f32,
            )],
        }
    }

    /// One pass of `draws` quads, each with the next binding set when `vary`, else all with the first,
    /// as strips or, when `indexed`, as indexed lists without restart; then a submit.
    fn frame(&self, b: &mut B, draws: u32, vary: bool, indexed: bool) {
        b.begin_pass(&kit::pass(&self.target, [0; 4], None));
        for i in 0..draws {
            let set = &self.sets[if vary { (i % SETS) as usize } else { 0 }];
            let strip = kit::call(
                &self.kit,
                &self.program,
                Some(set),
                &self.viewport,
                Topology::TriangleStrip,
                4,
            );
            if !indexed {
                b.draw(&strip);
                continue;
            }
            b.draw(&DrawCall {
                index: Some(IndexBinding {
                    buffer: &self.indices,
                    offset: 0,
                    format: IndexFormat::U16,
                }),
                topology: Topology::TriangleList,
                range: DrawRange::Indexed {
                    first: 0,
                    count: 6,
                    base_vertex: 0,
                    first_instance: 0,
                    instances: 1,
                },
                ..strip
            });
        }
        b.end_pass();
        b.submit();
    }
}

/// The scene on the WebGL 2 back end, on a canvas's context.
#[wasm_bindgen]
pub struct GlCost(Gl, Scene<Gl>);

#[wasm_bindgen]
impl GlCost {
    #[wasm_bindgen(constructor)]
    pub fn new(context: web_sys::WebGl2RenderingContext) -> GlCost {
        let config = GlConfig {
            gl: WebGl::new(context, &[]),
            off: Vec::new(),
        };
        let Ok(mut b) = <Gl as StandaloneGpu>::new(config);
        let s = Scene::new(&mut b);
        GlCost(b, s)
    }

    pub fn frame(&mut self, draws: u32, vary: bool, indexed: bool) {
        self.1.frame(&mut self.0, draws, vary, indexed);
    }

    /// The device's state and the renderer, after the frames.
    pub fn report(&mut self) -> String {
        format!("{:?}, {}", self.0.status(), self.0.renderer())
    }
}

/// The scene on the WebGPU back end, on the browser's WebGPU.
#[wasm_bindgen]
pub struct GpuCost(WebGpu, Scene<WebGpu>, String);

// JSPI exports, which wasm-bindgen still marks experimental.
#[allow(deprecated)]
mod gpu {
    use super::*;

    /// The WebGPU back end waits for its probe by suspending, so this is a JSPI export.
    #[wasm_bindgen(jspi)]
    pub fn gpu_cost() -> Result<GpuCost, JsError> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::BROWSER_WEBGPU;
        let instance = wgpu::Instance::new(desc);
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| JsError::new(&format!("no WebGPU adapter: {e}")))?;
        let info = adapter.get_info();
        let (device, queue) =
            block_on(adapter.request_device(&WebGpu::device_descriptor(&adapter)))
                .map_err(|e| JsError::new(&format!("no WebGPU device: {e}")))?;
        let mut b = WebGpu::new(&adapter, device, queue);
        let s = Scene::new(&mut b);
        Ok(GpuCost(
            b,
            s,
            format!("{} ({:?})", info.name, info.device_type),
        ))
    }
}

#[wasm_bindgen]
impl GpuCost {
    pub fn frame(&mut self, draws: u32, vary: bool, indexed: bool) {
        self.1.frame(&mut self.0, draws, vary, indexed);
    }

    pub fn report(&mut self) -> String {
        format!("{:?}, {}", self.0.status(), self.2)
    }
}
