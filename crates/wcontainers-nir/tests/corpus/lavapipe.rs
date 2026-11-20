//! Runs one program on lavapipe, Mesa's Vulkan driver on the CPU, loaded from the pinned build without
//! a Vulkan loader: a compute dispatch, a vertex program's outputs by transform feedback, or a
//! fragment program fed one point a pixel by a vertex program made for it.

use ash::vk;
use std::ffi::CString;
use wcontainers_nir::mesa::LAVAPIPE;

/// The driver's own entry point (a Vulkan driver exports this, the loader `vkGetInstanceProcAddr`).
const ICD_GET_INSTANCE_PROC_ADDR: &[u8] = b"vk_icdGetInstanceProcAddr\0";
/// The Vulkan the references use.
const API_VERSION: u32 = vk::API_VERSION_1_3;
/// The mip level and array layer everything here starts at.
const FIRST: u32 = 0;
/// How long a submission may take before the run counts as hung.
const FENCE_TIME_LIMIT_NS: u64 = 60_000_000_000;

pub type Error = String;

fn vk_error(step: &'static str) -> impl Fn(vk::Result) -> Error {
    move |e| format!("lavapipe: {step}: {e}")
}

/// A stage of the programs run here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Vertex,
    Fragment,
    Compute,
}

/// An image's contents and shape.
#[derive(Clone, Debug)]
pub struct Image {
    pub view: vk::ImageViewType,
    pub format: vk::Format,
    pub extent: vk::Extent3D,
    pub layers: u32,
    pub depth: bool,
    pub bytes: Vec<u8>,
}

/// One array element of a descriptor.
#[derive(Clone, Debug)]
pub enum Element {
    Buffer(Vec<u8>),
    Image(Image),
    Sampler { comparison: bool },
    Combined(Image, bool),
}

/// One binding of a descriptor set.
#[derive(Clone, Debug)]
pub struct Descriptor {
    pub set: u32,
    pub binding: u32,
    pub kind: vk::DescriptorType,
    pub elements: Vec<Element>,
}

/// One vertex input: its location, format and the bytes of every vertex.
#[derive(Clone, Debug)]
pub struct Attribute {
    pub location: u32,
    pub format: vk::Format,
    pub stride: u32,
    pub bytes: Vec<u8>,
}

/// One program run.
pub struct Job<'a> {
    pub stage: Stage,
    pub words: &'a [u32],
    pub entry: &'a str,
    pub descriptors: &'a [Descriptor],
    pub immediates: &'a [u8],
    /// Compute: the workgroups dispatched.
    pub groups: [u32; 3],
    /// Vertex and fragment: the points drawn (for a fragment program, one a pixel of a row).
    pub vertices: u32,
    pub attributes: &'a [Attribute],
    /// Fragment: the vertex program feeding it, and its entry point.
    pub feeder: Option<(&'a [u32], &'a str)>,
    /// Vertex: the bytes transform feedback captures a vertex.
    pub capture_stride: u32,
    /// Fragment: a colour target's format at each location (None where there is none), and
    /// whether the program writes depth.
    pub targets: &'a [Option<vk::Format>],
    pub depth: bool,
    /// The bytes every colour target texel starts as.
    pub clear: [u32; 4],
}

/// What a run left: captured vertex outputs, each colour target's bytes, the depth target's,
/// and every storage element's contents, in the order of the descriptors.
#[derive(Debug, PartialEq)]
pub struct Outputs {
    pub captured: Vec<u8>,
    pub targets: Vec<Vec<u8>>,
    pub depth: Vec<u8>,
    pub storage: Vec<Vec<u8>>,
}

pub struct Lavapipe {
    _library: libloading::Library,
    instance: ash::Instance,
    physical: vk::PhysicalDevice,
    device: ash::Device,
    queue: vk::Queue,
    family: u32,
    xfb: ash::ext::transform_feedback::Device,
}

/// What a run made, destroyed when it ends.
struct Made<'a> {
    device: &'a ash::Device,
    memory: Vec<vk::DeviceMemory>,
    buffers: Vec<vk::Buffer>,
    images: Vec<vk::Image>,
    views: Vec<vk::ImageView>,
    samplers: Vec<vk::Sampler>,
    modules: Vec<vk::ShaderModule>,
    set_layouts: Vec<vk::DescriptorSetLayout>,
    layouts: Vec<vk::PipelineLayout>,
    pipelines: Vec<vk::Pipeline>,
    pools: Vec<vk::DescriptorPool>,
    command_pools: Vec<vk::CommandPool>,
    fences: Vec<vk::Fence>,
}

impl Drop for Made<'_> {
    fn drop(&mut self) {
        // SAFETY: everything here was made on this device, and the device is idle (each run waits).
        unsafe {
            let d = self.device;
            let _ = d.device_wait_idle();
            self.fences.iter().for_each(|&x| d.destroy_fence(x, None));
            self.command_pools
                .iter()
                .for_each(|&x| d.destroy_command_pool(x, None));
            self.pools
                .iter()
                .for_each(|&x| d.destroy_descriptor_pool(x, None));
            self.pipelines
                .iter()
                .for_each(|&x| d.destroy_pipeline(x, None));
            self.layouts
                .iter()
                .for_each(|&x| d.destroy_pipeline_layout(x, None));
            self.set_layouts
                .iter()
                .for_each(|&x| d.destroy_descriptor_set_layout(x, None));
            self.modules
                .iter()
                .for_each(|&x| d.destroy_shader_module(x, None));
            self.samplers
                .iter()
                .for_each(|&x| d.destroy_sampler(x, None));
            self.views
                .iter()
                .for_each(|&x| d.destroy_image_view(x, None));
            self.images.iter().for_each(|&x| d.destroy_image(x, None));
            self.buffers.iter().for_each(|&x| d.destroy_buffer(x, None));
            self.memory.iter().for_each(|&x| d.free_memory(x, None));
        }
    }
}

/// A buffer whose memory the host reads and writes.
#[derive(Clone, Copy)]
struct Buffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    size: u64,
}

impl Lavapipe {
    /// Loads lavapipe and makes a device with every feature it reports of Vulkan 1.0 to 1.3,
    /// transform feedback, and robust access (accesses outside a binding read zero, write nothing).
    pub fn open() -> Result<Lavapipe, Error> {
        // SAFETY: lavapipe is a Vulkan driver built from the pinned Mesa; loading it runs no
        // initialiser beyond its own.
        let library = unsafe { libloading::Library::new(LAVAPIPE) }
            .map_err(|e| format!("lavapipe: {LAVAPIPE}: {e}"))?;
        // SAFETY: the symbol is the driver's vkGetInstanceProcAddr, of that type.
        let get: vk::PFN_vkGetInstanceProcAddr = unsafe {
            *library
                .get(ICD_GET_INSTANCE_PROC_ADDR)
                .map_err(|e| format!("lavapipe: {e}"))?
        };
        // SAFETY: the pointer stays valid while `library` is held, as long as the Lavapipe lives.
        let entry = unsafe {
            ash::Entry::from_static_fn(ash::StaticFn {
                get_instance_proc_addr: get,
            })
        };
        let app = vk::ApplicationInfo::default().api_version(API_VERSION);
        // SAFETY: plain Vulkan calls on objects made here, each checked.
        unsafe {
            let instance = entry
                .create_instance(
                    &vk::InstanceCreateInfo::default().application_info(&app),
                    None,
                )
                .map_err(vk_error("vkCreateInstance"))?;
            let physical = *instance
                .enumerate_physical_devices()
                .map_err(vk_error("vkEnumeratePhysicalDevices"))?
                .first()
                .ok_or("lavapipe: no device")?;
            let family = instance
                .get_physical_device_queue_family_properties(physical)
                .iter()
                .position(|f| {
                    f.queue_flags
                        .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
                })
                .ok_or("lavapipe: no graphics and compute queue")?;
            let family = u32::try_from(family).expect("a queue family index");
            let mut v11 = vk::PhysicalDeviceVulkan11Features::default();
            let mut v12 = vk::PhysicalDeviceVulkan12Features::default();
            let mut v13 = vk::PhysicalDeviceVulkan13Features::default();
            let mut xfb = vk::PhysicalDeviceTransformFeedbackFeaturesEXT::default();
            let mut robust = vk::PhysicalDeviceRobustness2FeaturesEXT::default();
            let mut features = vk::PhysicalDeviceFeatures2::default()
                .push_next(&mut v11)
                .push_next(&mut v12)
                .push_next(&mut v13)
                .push_next(&mut xfb)
                .push_next(&mut robust);
            instance.get_physical_device_features2(physical, &mut features);
            let priorities = [1.0];
            let queues = [vk::DeviceQueueCreateInfo::default()
                .queue_family_index(family)
                .queue_priorities(&priorities)];
            let extensions = [
                ash::ext::transform_feedback::NAME.as_ptr(),
                ash::ext::robustness2::NAME.as_ptr(),
            ];
            let info = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queues)
                .enabled_extension_names(&extensions)
                .push_next(&mut features);
            let device = instance
                .create_device(physical, &info, None)
                .map_err(vk_error("vkCreateDevice"))?;
            let queue = device.get_device_queue(family, FIRST);
            let xfb = ash::ext::transform_feedback::Device::new(&instance, &device);
            Ok(Lavapipe {
                _library: library,
                instance,
                physical,
                device,
                queue,
                family,
                xfb,
            })
        }
    }

    fn memory_type(&self, bits: u32, flags: vk::MemoryPropertyFlags) -> Result<u32, Error> {
        // SAFETY: a query on a live physical device.
        let props = unsafe {
            self.instance
                .get_physical_device_memory_properties(self.physical)
        };
        (0..props.memory_type_count)
            .find(|&i| {
                bits & (1 << i) != 0
                    && props.memory_types[i as usize]
                        .property_flags
                        .contains(flags)
            })
            .ok_or_else(|| format!("lavapipe: no memory type {flags:?}"))
    }

    fn buffer(
        &self,
        made: &mut Made,
        size: u64,
        usage: vk::BufferUsageFlags,
        bytes: &[u8],
    ) -> Result<Buffer, Error> {
        let d = &self.device;
        // SAFETY: plain Vulkan calls; the mapping covers `size` bytes and `bytes` fits in it.
        unsafe {
            let buffer = d
                .create_buffer(
                    &vk::BufferCreateInfo::default()
                        .size(size.max(1))
                        .usage(usage),
                    None,
                )
                .map_err(vk_error("vkCreateBuffer"))?;
            made.buffers.push(buffer);
            let needs = d.get_buffer_memory_requirements(buffer);
            let kind = self.memory_type(
                needs.memory_type_bits,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )?;
            let memory = d
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(needs.size)
                        .memory_type_index(kind),
                    None,
                )
                .map_err(vk_error("vkAllocateMemory"))?;
            made.memory.push(memory);
            d.bind_buffer_memory(buffer, memory, 0)
                .map_err(vk_error("vkBindBufferMemory"))?;
            let at = d
                .map_memory(memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
                .map_err(vk_error("vkMapMemory"))?
                .cast::<u8>();
            std::ptr::write_bytes(at, 0, usize::try_from(needs.size).expect("a size"));
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), at, bytes.len().min(size as usize));
            d.unmap_memory(memory);
            Ok(Buffer {
                buffer,
                memory,
                size,
            })
        }
    }

    fn read(&self, buffer: Buffer) -> Result<Vec<u8>, Error> {
        let d = &self.device;
        // SAFETY: the memory is host visible and holds `size` bytes; the device is idle.
        unsafe {
            let at = d
                .map_memory(
                    buffer.memory,
                    0,
                    vk::WHOLE_SIZE,
                    vk::MemoryMapFlags::empty(),
                )
                .map_err(vk_error("vkMapMemory"))?
                .cast::<u8>();
            let bytes =
                std::slice::from_raw_parts(at, usize::try_from(buffer.size).expect("a size"))
                    .to_vec();
            d.unmap_memory(buffer.memory);
            Ok(bytes)
        }
    }

    /// An image in the general layout once `cmd` has run, filled from `image.bytes`.
    fn image(
        &self,
        made: &mut Made,
        cmd: vk::CommandBuffer,
        image: &Image,
        usage: vk::ImageUsageFlags,
    ) -> Result<(vk::Image, vk::ImageView), Error> {
        let d = &self.device;
        let aspect = aspect(image.depth);
        let cube = matches!(
            image.view,
            vk::ImageViewType::CUBE | vk::ImageViewType::CUBE_ARRAY
        );
        let kind = match image.view {
            vk::ImageViewType::TYPE_1D | vk::ImageViewType::TYPE_1D_ARRAY => vk::ImageType::TYPE_1D,
            vk::ImageViewType::TYPE_3D => vk::ImageType::TYPE_3D,
            _ => vk::ImageType::TYPE_2D,
        };
        // SAFETY: plain Vulkan calls on objects made here; `cmd` is recording.
        unsafe {
            let handle = d
                .create_image(
                    &vk::ImageCreateInfo::default()
                        .flags(if cube {
                            vk::ImageCreateFlags::CUBE_COMPATIBLE
                        } else {
                            vk::ImageCreateFlags::empty()
                        })
                        .image_type(kind)
                        .format(image.format)
                        .extent(image.extent)
                        .mip_levels(1)
                        .array_layers(image.layers)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(
                            usage
                                | vk::ImageUsageFlags::TRANSFER_DST
                                | vk::ImageUsageFlags::TRANSFER_SRC,
                        ),
                    None,
                )
                .map_err(vk_error("vkCreateImage"))?;
            made.images.push(handle);
            let needs = d.get_image_memory_requirements(handle);
            let memory = d
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(needs.size)
                        .memory_type_index(self.memory_type(
                            needs.memory_type_bits,
                            vk::MemoryPropertyFlags::empty(),
                        )?),
                    None,
                )
                .map_err(vk_error("vkAllocateMemory"))?;
            made.memory.push(memory);
            d.bind_image_memory(handle, memory, 0)
                .map_err(vk_error("vkBindImageMemory"))?;
            let range = vk::ImageSubresourceRange::default()
                .aspect_mask(aspect)
                .level_count(1)
                .layer_count(image.layers);
            let view = d
                .create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(handle)
                        .view_type(image.view)
                        .format(image.format)
                        .subresource_range(range),
                    None,
                )
                .map_err(vk_error("vkCreateImageView"))?;
            made.views.push(view);
            barrier(d, cmd, handle, range, vk::ImageLayout::UNDEFINED);
            if !image.bytes.is_empty() {
                let staging = self.buffer(
                    made,
                    image.bytes.len() as u64,
                    vk::BufferUsageFlags::TRANSFER_SRC,
                    &image.bytes,
                )?;
                d.cmd_copy_buffer_to_image(
                    cmd,
                    staging.buffer,
                    handle,
                    vk::ImageLayout::GENERAL,
                    &[copy_region(image)],
                );
                barrier(d, cmd, handle, range, vk::ImageLayout::GENERAL);
            }
            Ok((handle, view))
        }
    }

    fn sampler(&self, made: &mut Made, comparison: bool) -> Result<vk::Sampler, Error> {
        let info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::NEAREST)
            .min_filter(vk::Filter::NEAREST)
            .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .compare_enable(comparison)
            .compare_op(vk::CompareOp::LESS_OR_EQUAL);
        // SAFETY: a plain Vulkan call.
        let sampler = unsafe { self.device.create_sampler(&info, None) }
            .map_err(vk_error("vkCreateSampler"))?;
        made.samplers.push(sampler);
        Ok(sampler)
    }

    fn module(&self, made: &mut Made, words: &[u32]) -> Result<vk::ShaderModule, Error> {
        // SAFETY: a plain Vulkan call; the driver checks nothing it cannot read.
        let module = unsafe {
            self.device
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(words), None)
        }
        .map_err(vk_error("vkCreateShaderModule"))?;
        made.modules.push(module);
        Ok(module)
    }

    /// Runs the job once on fresh objects made from its contents.
    #[allow(
        clippy::too_many_lines,
        reason = "one run, in the order Vulkan takes it"
    )]
    pub fn run(&self, job: &Job) -> Result<Outputs, Error> {
        let d = &self.device;
        let mut made = Made {
            device: d,
            memory: Vec::new(),
            buffers: Vec::new(),
            images: Vec::new(),
            views: Vec::new(),
            samplers: Vec::new(),
            modules: Vec::new(),
            set_layouts: Vec::new(),
            layouts: Vec::new(),
            pipelines: Vec::new(),
            pools: Vec::new(),
            command_pools: Vec::new(),
            fences: Vec::new(),
        };
        let made = &mut made;
        let entry = CString::new(job.entry).map_err(|e| e.to_string())?;
        // SAFETY: plain Vulkan calls on objects made here, recorded and submitted in order; the
        // run waits for its fence before reading anything back.
        unsafe {
            let command_pool = d
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default().queue_family_index(self.family),
                    None,
                )
                .map_err(vk_error("vkCreateCommandPool"))?;
            made.command_pools.push(command_pool);
            let cmd = d
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(command_pool)
                        .command_buffer_count(1),
                )
                .map_err(vk_error("vkAllocateCommandBuffers"))?[0];
            d.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())
                .map_err(vk_error("vkBeginCommandBuffer"))?;

            // The descriptor sets, in set order, with the storage each holds.
            let sets = job.descriptors.iter().map(|x| x.set + 1).max().unwrap_or(0);
            let mut storage: Vec<(Buffer, Option<(vk::Image, &Image)>)> = Vec::new();
            let mut layouts = Vec::new();
            for set in 0..sets {
                let bindings: Vec<vk::DescriptorSetLayoutBinding> = job
                    .descriptors
                    .iter()
                    .filter(|x| x.set == set)
                    .map(|x| {
                        vk::DescriptorSetLayoutBinding::default()
                            .binding(x.binding)
                            .descriptor_type(x.kind)
                            .descriptor_count(len(&x.elements))
                            .stage_flags(vk::ShaderStageFlags::ALL)
                    })
                    .collect();
                let layout = d
                    .create_descriptor_set_layout(
                        &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                        None,
                    )
                    .map_err(vk_error("vkCreateDescriptorSetLayout"))?;
                made.set_layouts.push(layout);
                layouts.push(layout);
            }
            let ranges = [vk::PushConstantRange::default()
                .stage_flags(vk::ShaderStageFlags::ALL)
                .size(len(job.immediates))];
            let layout = d
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&layouts)
                        .push_constant_ranges(if job.immediates.is_empty() {
                            &[]
                        } else {
                            &ranges
                        }),
                    None,
                )
                .map_err(vk_error("vkCreatePipelineLayout"))?;
            made.layouts.push(layout);
            let mut sets_made = Vec::new();
            if sets > 0 {
                let kinds = [
                    vk::DescriptorType::UNIFORM_BUFFER,
                    vk::DescriptorType::STORAGE_BUFFER,
                    vk::DescriptorType::SAMPLED_IMAGE,
                    vk::DescriptorType::STORAGE_IMAGE,
                    vk::DescriptorType::SAMPLER,
                    vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                ];
                let sizes: Vec<vk::DescriptorPoolSize> = kinds
                    .iter()
                    .map(|&k| {
                        vk::DescriptorPoolSize::default().ty(k).descriptor_count(
                            job.descriptors
                                .iter()
                                .filter(|x| x.kind == k)
                                .map(|x| len(&x.elements))
                                .sum::<u32>()
                                .max(1),
                        )
                    })
                    .collect();
                let pool = d
                    .create_descriptor_pool(
                        &vk::DescriptorPoolCreateInfo::default()
                            .max_sets(sets)
                            .pool_sizes(&sizes),
                        None,
                    )
                    .map_err(vk_error("vkCreateDescriptorPool"))?;
                made.pools.push(pool);
                sets_made = d
                    .allocate_descriptor_sets(
                        &vk::DescriptorSetAllocateInfo::default()
                            .descriptor_pool(pool)
                            .set_layouts(&layouts),
                    )
                    .map_err(vk_error("vkAllocateDescriptorSets"))?;
            }
            for x in job.descriptors {
                let mut buffers = Vec::new();
                let mut images = Vec::new();
                for element in &x.elements {
                    match element {
                        Element::Buffer(bytes) => {
                            let usage = if x.kind == vk::DescriptorType::UNIFORM_BUFFER {
                                vk::BufferUsageFlags::UNIFORM_BUFFER
                            } else {
                                vk::BufferUsageFlags::STORAGE_BUFFER
                            };
                            let b = self.buffer(made, bytes.len() as u64, usage, bytes)?;
                            if x.kind == vk::DescriptorType::STORAGE_BUFFER {
                                storage.push((b, None));
                            }
                            buffers.push(
                                vk::DescriptorBufferInfo::default()
                                    .buffer(b.buffer)
                                    .range(vk::WHOLE_SIZE),
                            );
                        }
                        Element::Image(image) => {
                            let usage = if x.kind == vk::DescriptorType::STORAGE_IMAGE {
                                vk::ImageUsageFlags::STORAGE
                            } else {
                                vk::ImageUsageFlags::SAMPLED
                            };
                            let (handle, view) = self.image(made, cmd, image, usage)?;
                            if x.kind == vk::DescriptorType::STORAGE_IMAGE {
                                let b = self.buffer(
                                    made,
                                    image.bytes.len() as u64,
                                    vk::BufferUsageFlags::TRANSFER_DST,
                                    &[],
                                )?;
                                storage.push((b, Some((handle, image))));
                            }
                            images.push(
                                vk::DescriptorImageInfo::default()
                                    .image_view(view)
                                    .image_layout(vk::ImageLayout::GENERAL),
                            );
                        }
                        Element::Sampler { comparison } => images.push(
                            vk::DescriptorImageInfo::default()
                                .sampler(self.sampler(made, *comparison)?),
                        ),
                        Element::Combined(image, comparison) => {
                            let (_, view) =
                                self.image(made, cmd, image, vk::ImageUsageFlags::SAMPLED)?;
                            images.push(
                                vk::DescriptorImageInfo::default()
                                    .image_view(view)
                                    .image_layout(vk::ImageLayout::GENERAL)
                                    .sampler(self.sampler(made, *comparison)?),
                            );
                        }
                    }
                }
                let write = vk::WriteDescriptorSet::default()
                    .dst_set(sets_made[x.set as usize])
                    .dst_binding(x.binding)
                    .descriptor_type(x.kind);
                let write = if buffers.is_empty() {
                    write.image_info(&images)
                } else {
                    write.buffer_info(&buffers)
                };
                d.update_descriptor_sets(&[write], &[]);
            }

            let module = self.module(made, job.words)?;
            let stage_flag = match job.stage {
                Stage::Vertex => vk::ShaderStageFlags::VERTEX,
                Stage::Fragment => vk::ShaderStageFlags::FRAGMENT,
                Stage::Compute => vk::ShaderStageFlags::COMPUTE,
            };
            let mut stages = vec![
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(stage_flag)
                    .module(module)
                    .name(&entry),
            ];
            let feeder_entry =
                CString::new(job.feeder.map_or("", |f| f.1)).map_err(|e| e.to_string())?;
            if let Some((feeder, _)) = job.feeder {
                let feeder = self.module(made, feeder)?;
                stages.insert(
                    0,
                    vk::PipelineShaderStageCreateInfo::default()
                        .stage(vk::ShaderStageFlags::VERTEX)
                        .module(feeder)
                        .name(&feeder_entry),
                );
            }
            let bind_point = if job.stage == Stage::Compute {
                vk::PipelineBindPoint::COMPUTE
            } else {
                vk::PipelineBindPoint::GRAPHICS
            };

            let mut captured = None;
            let mut targets = Vec::new();
            let mut depth = None;
            let width = job.vertices.max(1);
            if job.stage == Stage::Compute {
                let pipeline = d
                    .create_compute_pipelines(
                        vk::PipelineCache::null(),
                        &[vk::ComputePipelineCreateInfo::default()
                            .stage(stages[0])
                            .layout(layout)],
                        None,
                    )
                    .map_err(|(_, e)| vk_error("vkCreateComputePipelines")(e))?[0];
                made.pipelines.push(pipeline);
                d.cmd_bind_pipeline(cmd, bind_point, pipeline);
                bind(d, cmd, bind_point, layout, &sets_made, job.immediates);
                let [x, y, z] = job.groups;
                d.cmd_dispatch(cmd, x, y, z);
            } else {
                let bindings: Vec<_> = (0..)
                    .zip(job.attributes)
                    .map(|(i, a)| {
                        vk::VertexInputBindingDescription::default()
                            .binding(i)
                            .stride(a.stride)
                            .input_rate(vk::VertexInputRate::VERTEX)
                    })
                    .collect();
                let attributes: Vec<_> = (0..)
                    .zip(job.attributes)
                    .map(|(i, a)| {
                        vk::VertexInputAttributeDescription::default()
                            .location(a.location)
                            .binding(i)
                            .format(a.format)
                    })
                    .collect();
                let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
                    .vertex_binding_descriptions(&bindings)
                    .vertex_attribute_descriptions(&attributes);
                let assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
                    .topology(vk::PrimitiveTopology::POINT_LIST);
                let viewports = [vk::Viewport::default()
                    .width(width as f32)
                    .height(1.0)
                    .max_depth(1.0)];
                let extent = vk::Extent2D { width, height: 1 };
                let scissors = [vk::Rect2D::default().extent(extent)];
                let viewport = vk::PipelineViewportStateCreateInfo::default()
                    .viewports(&viewports)
                    .scissors(&scissors);
                let raster = vk::PipelineRasterizationStateCreateInfo::default()
                    .rasterizer_discard_enable(job.stage == Stage::Vertex)
                    .polygon_mode(vk::PolygonMode::FILL)
                    .cull_mode(vk::CullModeFlags::NONE)
                    .line_width(1.0);
                let multisample = vk::PipelineMultisampleStateCreateInfo::default()
                    .rasterization_samples(vk::SampleCountFlags::TYPE_1);
                let depth_state = vk::PipelineDepthStencilStateCreateInfo::default()
                    .depth_test_enable(job.depth)
                    .depth_write_enable(job.depth)
                    .depth_compare_op(vk::CompareOp::ALWAYS);
                let formats: Vec<vk::Format> = job
                    .targets
                    .iter()
                    .map(|t| t.unwrap_or(vk::Format::UNDEFINED))
                    .collect();
                let blends: Vec<_> = formats
                    .iter()
                    .map(|_| {
                        vk::PipelineColorBlendAttachmentState::default()
                            .color_write_mask(vk::ColorComponentFlags::RGBA)
                    })
                    .collect();
                let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blends);
                let mut rendering = vk::PipelineRenderingCreateInfo::default()
                    .color_attachment_formats(&formats)
                    .depth_attachment_format(if job.depth {
                        vk::Format::D32_SFLOAT
                    } else {
                        vk::Format::UNDEFINED
                    });
                let info = vk::GraphicsPipelineCreateInfo::default()
                    .stages(&stages)
                    .vertex_input_state(&vertex_input)
                    .input_assembly_state(&assembly)
                    .viewport_state(&viewport)
                    .rasterization_state(&raster)
                    .multisample_state(&multisample)
                    .depth_stencil_state(&depth_state)
                    .color_blend_state(&blend)
                    .layout(layout)
                    .push_next(&mut rendering);
                let pipeline = d
                    .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
                    .map_err(|(_, e)| vk_error("vkCreateGraphicsPipelines")(e))?[0];
                made.pipelines.push(pipeline);

                let mut vertex_buffers = Vec::new();
                for a in job.attributes {
                    let b = self.buffer(
                        made,
                        a.bytes.len() as u64,
                        vk::BufferUsageFlags::VERTEX_BUFFER,
                        &a.bytes,
                    )?;
                    vertex_buffers.push(b.buffer);
                }
                let target_image = |made: &mut Made, format, depth: bool| {
                    let image = Image {
                        view: vk::ImageViewType::TYPE_2D,
                        format,
                        extent: vk::Extent3D {
                            width,
                            height: 1,
                            depth: 1,
                        },
                        layers: 1,
                        depth,
                        bytes: Vec::new(),
                    };
                    let usage = if depth {
                        vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                    } else {
                        vk::ImageUsageFlags::COLOR_ATTACHMENT
                    };
                    self.image(made, cmd, &image, usage)
                        .map(|(h, v)| (h, v, image))
                };
                let mut colour = Vec::new();
                for t in job.targets {
                    let info = match t {
                        Some(format) => {
                            let (h, v, image) = target_image(made, *format, false)?;
                            targets.push((h, image));
                            vk::RenderingAttachmentInfo::default()
                                .image_view(v)
                                .image_layout(vk::ImageLayout::GENERAL)
                                .load_op(vk::AttachmentLoadOp::CLEAR)
                                .store_op(vk::AttachmentStoreOp::STORE)
                                .clear_value(vk::ClearValue {
                                    color: vk::ClearColorValue { uint32: job.clear },
                                })
                        }
                        None => vk::RenderingAttachmentInfo::default(),
                    };
                    colour.push(info);
                }
                let mut depth_info = vk::RenderingAttachmentInfo::default();
                if job.depth {
                    let (h, v, image) = target_image(made, vk::Format::D32_SFLOAT, true)?;
                    depth = Some((h, image));
                    depth_info = depth_info
                        .image_view(v)
                        .image_layout(vk::ImageLayout::GENERAL)
                        .load_op(vk::AttachmentLoadOp::CLEAR)
                        .store_op(vk::AttachmentStoreOp::STORE);
                }
                let xfb_buffer = if job.stage == Stage::Vertex {
                    let b = self.buffer(
                        made,
                        u64::from(job.capture_stride) * u64::from(job.vertices),
                        vk::BufferUsageFlags::TRANSFORM_FEEDBACK_BUFFER_EXT,
                        &[],
                    )?;
                    captured = Some(b);
                    Some(b)
                } else {
                    None
                };
                let mut rendering = vk::RenderingInfo::default()
                    .render_area(vk::Rect2D::default().extent(extent))
                    .layer_count(1)
                    .color_attachments(&colour);
                if job.depth {
                    rendering = rendering.depth_attachment(&depth_info);
                }
                all_barrier(d, cmd);
                d.cmd_begin_rendering(cmd, &rendering);
                d.cmd_bind_pipeline(cmd, bind_point, pipeline);
                bind(d, cmd, bind_point, layout, &sets_made, job.immediates);
                let offsets = vec![0; vertex_buffers.len()];
                if !vertex_buffers.is_empty() {
                    d.cmd_bind_vertex_buffers(cmd, 0, &vertex_buffers, &offsets);
                }
                if let Some(b) = xfb_buffer {
                    (self.xfb.fp().cmd_bind_transform_feedback_buffers_ext)(
                        cmd, 0, 1, &b.buffer, &0, &b.size,
                    );
                    (self.xfb.fp().cmd_begin_transform_feedback_ext)(
                        cmd,
                        0,
                        0,
                        std::ptr::null(),
                        std::ptr::null(),
                    );
                }
                d.cmd_draw(cmd, job.vertices, 1, 0, 0);
                if xfb_buffer.is_some() {
                    (self.xfb.fp().cmd_end_transform_feedback_ext)(
                        cmd,
                        0,
                        0,
                        std::ptr::null(),
                        std::ptr::null(),
                    );
                }
                d.cmd_end_rendering(cmd);
            }
            all_barrier(d, cmd);

            // Images are read back through buffers.
            let mut readbacks = Vec::new();
            for (handle, image) in targets.iter().chain(depth.iter()) {
                let b = self.buffer(
                    made,
                    u64::from(width) * u64::from(texel_bytes(image.format)),
                    vk::BufferUsageFlags::TRANSFER_DST,
                    &[],
                )?;
                d.cmd_copy_image_to_buffer(
                    cmd,
                    *handle,
                    vk::ImageLayout::GENERAL,
                    b.buffer,
                    &[copy_region(image)],
                );
                readbacks.push(b);
            }
            for (b, image) in &storage {
                if let Some((handle, image)) = image {
                    d.cmd_copy_image_to_buffer(
                        cmd,
                        *handle,
                        vk::ImageLayout::GENERAL,
                        b.buffer,
                        &[copy_region(image)],
                    );
                }
            }
            all_barrier(d, cmd);
            d.end_command_buffer(cmd)
                .map_err(vk_error("vkEndCommandBuffer"))?;
            let fence = d
                .create_fence(&vk::FenceCreateInfo::default(), None)
                .map_err(vk_error("vkCreateFence"))?;
            made.fences.push(fence);
            let cmds = [cmd];
            d.queue_submit(
                self.queue,
                &[vk::SubmitInfo::default().command_buffers(&cmds)],
                fence,
            )
            .map_err(vk_error("vkQueueSubmit"))?;
            d.wait_for_fences(&[fence], true, FENCE_TIME_LIMIT_NS)
                .map_err(vk_error("vkWaitForFences"))?;

            let mut out = Outputs {
                captured: match captured {
                    Some(b) => self.read(b)?,
                    None => Vec::new(),
                },
                targets: Vec::new(),
                depth: Vec::new(),
                storage: Vec::new(),
            };
            let mut readbacks = readbacks.into_iter();
            for _ in &targets {
                let b = readbacks.next().expect("one readback a target");
                out.targets.push(self.read(b)?);
            }
            if depth.is_some() {
                let b = readbacks.next().expect("one readback for depth");
                out.depth = self.read(b)?;
            }
            for (b, _) in &storage {
                out.storage.push(self.read(*b)?);
            }
            Ok(out)
        }
    }
}

impl Drop for Lavapipe {
    fn drop(&mut self) {
        // SAFETY: nothing made on the device outlives a run.
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

fn len<T>(items: &[T]) -> u32 {
    u32::try_from(items.len()).expect("a count Vulkan takes")
}

fn aspect(depth: bool) -> vk::ImageAspectFlags {
    if depth {
        vk::ImageAspectFlags::DEPTH
    } else {
        vk::ImageAspectFlags::COLOR
    }
}

/// The bytes of one texel of the formats the targets use.
pub fn texel_bytes(format: vk::Format) -> u32 {
    match format {
        vk::Format::D32_SFLOAT => size_of::<f32>() as u32,
        _ => size_of::<[u32; 4]>() as u32,
    }
}

fn copy_region(image: &Image) -> vk::BufferImageCopy {
    vk::BufferImageCopy::default()
        .image_subresource(
            vk::ImageSubresourceLayers::default()
                .aspect_mask(aspect(image.depth))
                .layer_count(image.layers),
        )
        .image_extent(image.extent)
}

/// Every write before it visible to every access after it.
fn all_barrier(d: &ash::Device, cmd: vk::CommandBuffer) {
    let memory = [vk::MemoryBarrier::default()
        .src_access_mask(vk::AccessFlags::MEMORY_WRITE)
        .dst_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE)];
    // SAFETY: `cmd` is recording.
    unsafe {
        d.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::DependencyFlags::empty(),
            &memory,
            &[],
            &[],
        );
    }
}

/// The image moved from `from` into the general layout, after every earlier write.
fn barrier(
    d: &ash::Device,
    cmd: vk::CommandBuffer,
    image: vk::Image,
    range: vk::ImageSubresourceRange,
    from: vk::ImageLayout,
) {
    let barriers = [vk::ImageMemoryBarrier::default()
        .src_access_mask(vk::AccessFlags::MEMORY_WRITE)
        .dst_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE)
        .old_layout(from)
        .new_layout(vk::ImageLayout::GENERAL)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(range)];
    // SAFETY: `cmd` is recording.
    unsafe {
        d.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &barriers,
        );
    }
}

fn bind(
    d: &ash::Device,
    cmd: vk::CommandBuffer,
    point: vk::PipelineBindPoint,
    layout: vk::PipelineLayout,
    sets: &[vk::DescriptorSet],
    immediates: &[u8],
) {
    // SAFETY: `cmd` is recording, the sets match the layout and the immediates fit its range.
    unsafe {
        if !sets.is_empty() {
            d.cmd_bind_descriptor_sets(cmd, point, layout, 0, sets, &[]);
        }
        if !immediates.is_empty() {
            d.cmd_push_constants(cmd, layout, vk::ShaderStageFlags::ALL, 0, immediates);
        }
    }
}
