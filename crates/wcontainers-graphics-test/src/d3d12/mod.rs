//! The Direct3D 12 scenes: a device with one direct queue, list and fence, the helpers the scenes
//! share, and the list of scenes. Each scene records into the list, submits and waits, and reads
//! its targets back.

mod draws;
mod primitives;
mod raster;
pub mod scenes;
mod states;
mod textures;
mod views;

use std::cell::{Cell, RefCell};
use std::mem::{ManuallyDrop, transmute_copy};

use anyhow::{Context, Result};
use image::DynamicImage;
use windows::Win32::Foundation::{CloseHandle, RECT};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};
use windows::core::{Interface, PCSTR};

use crate::{Driver, debug, texels};

pub struct Gpu {
    pub device: ID3D12Device,
    pub queue: ID3D12CommandQueue,
    pub allocator: ID3D12CommandAllocator,
    pub list: ID3D12GraphicsCommandList,
    pub fence: ID3D12Fence,
    pub driver: Driver,
    pub adapter: String,
    debug: bool,
    value: Cell<u64>,
    /// Resources the recorded commands use, kept until they have run.
    keep: RefCell<Vec<ID3D12Resource>>,
}

pub fn create(driver: Driver, debug_layer: bool) -> Result<Gpu> {
    unsafe {
        if debug_layer {
            let mut debug_interface: Option<ID3D12Debug> = None;
            D3D12GetDebugInterface(&mut debug_interface)
                .context("the debug layer needs the Graphics Tools feature")?;
            debug_interface
                .context("no debug interface")?
                .EnableDebugLayer();
        }
        let factory: IDXGIFactory4 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
        let adapter: IDXGIAdapter1 = match driver {
            Driver::Hardware => factory.EnumAdapters1(0)?,
            Driver::Warp => factory.EnumWarpAdapter()?,
        };
        let adapter_name = crate::adapter_name(driver, &adapter.GetDesc1()?)?;
        let mut device: Option<ID3D12Device> = None;
        D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device)
            .context("D3D12CreateDevice")?;
        let device = device.context("no device")?;
        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            ..Default::default()
        })?;
        let allocator: ID3D12CommandAllocator =
            device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let list: ID3D12GraphicsCommandList =
            device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)?;
        let fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
        Ok(Gpu {
            device,
            queue,
            allocator,
            list,
            fence,
            driver,
            adapter: adapter_name,
            debug: debug_layer,
            value: Cell::new(0),
            keep: RefCell::new(Vec::new()),
        })
    }
}

/// Compiled DXIL as a pipeline's shader.
pub fn bytecode(code: &[u8]) -> D3D12_SHADER_BYTECODE {
    D3D12_SHADER_BYTECODE {
        pShaderBytecode: code.as_ptr().cast(),
        BytecodeLength: code.len(),
    }
}

pub fn buffer_desc(size: u64) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Width: size,
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        ..Default::default()
    }
}

pub fn tex2d_desc(
    width: u32,
    height: u32,
    format: DXGI_FORMAT,
    flags: D3D12_RESOURCE_FLAGS,
) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        Width: width as u64,
        Height: height,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Flags: flags,
        ..Default::default()
    }
}

pub fn transition(
    resource: &ID3D12Resource,
    before: D3D12_RESOURCE_STATES,
    after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: unsafe { transmute_copy(resource) },
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                StateBefore: before,
                StateAfter: after,
            }),
        },
    }
}

/// An input element of slot 0, per vertex.
pub fn element(name: PCSTR, format: DXGI_FORMAT, offset: u32) -> D3D12_INPUT_ELEMENT_DESC {
    D3D12_INPUT_ELEMENT_DESC {
        SemanticName: name,
        SemanticIndex: 0,
        Format: format,
        InputSlot: 0,
        AlignedByteOffset: offset,
        InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    }
}

/// A viewport and scissor rectangle of `width` by `height` at `x`, `y`.
pub fn area(list: &ID3D12GraphicsCommandList, x: u32, y: u32, width: u32, height: u32) {
    unsafe {
        list.RSSetViewports(&[D3D12_VIEWPORT {
            TopLeftX: x as f32,
            TopLeftY: y as f32,
            Width: width as f32,
            Height: height as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        }]);
        list.RSSetScissorRects(&[RECT {
            left: x as i32,
            top: y as i32,
            right: (x + width) as i32,
            bottom: (y + height) as i32,
        }]);
    }
}

impl Gpu {
    /// Closes the list, runs it, waits for it and opens it again.
    pub fn submit(&self) -> Result<()> {
        unsafe {
            self.list.Close()?;
            self.queue.ExecuteCommandLists(&[Some(self.list.cast()?)]);
            self.wait(&self.queue)?;
            self.allocator.Reset()?;
            self.list.Reset(&self.allocator, None)?;
        }
        self.keep.borrow_mut().clear();
        Ok(())
    }

    /// Waits until everything `queue` was given has run.
    pub fn wait(&self, queue: &ID3D12CommandQueue) -> Result<()> {
        let value = self.value.get() + 1;
        self.value.set(value);
        unsafe {
            queue.Signal(&self.fence, value)?;
            let event = CreateEventW(None, false, false, None)?;
            self.fence.SetEventOnCompletion(value, event)?;
            WaitForSingleObject(event, INFINITE);
            CloseHandle(event)?;
        }
        Ok(())
    }

    /// Keeps a resource alive until the next submission has run.
    pub fn keep(&self, resource: &ID3D12Resource) {
        self.keep.borrow_mut().push(resource.clone());
    }

    pub fn committed(
        &self,
        heap: D3D12_HEAP_TYPE,
        desc: &D3D12_RESOURCE_DESC,
        state: D3D12_RESOURCE_STATES,
        clear: Option<&D3D12_CLEAR_VALUE>,
    ) -> Result<ID3D12Resource> {
        let properties = D3D12_HEAP_PROPERTIES {
            Type: heap,
            ..Default::default()
        };
        let mut resource = None;
        unsafe {
            self.device.CreateCommittedResource(
                &properties,
                D3D12_HEAP_FLAG_NONE,
                desc,
                state,
                clear.map(|c| c as *const _),
                &mut resource,
            )?
        };
        resource.context("no resource")
    }

    /// An upload buffer holding `bytes`.
    pub fn upload(&self, bytes: &[u8]) -> Result<ID3D12Resource> {
        let buffer = self.committed(
            D3D12_HEAP_TYPE_UPLOAD,
            &buffer_desc(bytes.len().max(256) as u64),
            D3D12_RESOURCE_STATE_GENERIC_READ,
            None,
        )?;
        unsafe {
            let mut mapped = std::ptr::null_mut();
            buffer.Map(0, Some(&D3D12_RANGE::default()), Some(&mut mapped))?;
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), mapped.cast(), bytes.len());
            buffer.Unmap(0, None);
        }
        Ok(buffer)
    }

    /// Records a copy of `bytes` (rows of `pitch` bytes) into subresource 0 of a 2D texture in the
    /// copy destination state.
    pub fn fill_texture(&self, texture: &ID3D12Resource, bytes: &[u8], pitch: u32) -> Result<()> {
        let desc = unsafe { texture.GetDesc() };
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        unsafe {
            self.device.GetCopyableFootprints(
                &desc,
                0,
                1,
                0,
                Some(&mut footprint),
                None,
                None,
                None,
            )
        };
        let row = footprint.Footprint.RowPitch as usize;
        let rows = bytes.len() / pitch as usize;
        let mut staged = vec![0u8; row * rows];
        for y in 0..rows {
            staged[y * row..y * row + pitch as usize]
                .copy_from_slice(&bytes[y * pitch as usize..(y + 1) * pitch as usize]);
        }
        let upload = self.upload(&staged)?;
        unsafe {
            let to = texture_location(texture, 0);
            let from = footprint_location(&upload, footprint);
            self.list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
        }
        self.keep(&upload);
        Ok(())
    }

    /// Reads back subresource `index` of a 2D texture now in `state`, leaving it in that state.
    pub fn read(
        &self,
        texture: &ID3D12Resource,
        index: u32,
        state: D3D12_RESOURCE_STATES,
    ) -> Result<DynamicImage> {
        let (f, rows) = self.read_rows(texture, index, state)?;
        texels::picture(f.Format, f.Width, f.Height, &rows, f.RowPitch as usize)
    }

    /// Reads back both planes of a depth and stencil texture now in `state` as one picture, laid
    /// out as Direct3D 11 reads the whole texture: depth in red, stencil in green.
    pub fn read_depth_stencil(
        &self,
        texture: &ID3D12Resource,
        state: D3D12_RESOURCE_STATES,
    ) -> Result<DynamicImage> {
        let format = unsafe { texture.GetDesc() }.Format;
        let (d, depth) = self.read_rows(texture, 0, state)?;
        let (s, stencil) = self.read_rows(texture, 1, state)?;
        let (width, height) = (d.Width as usize, d.Height as usize);
        let size = if format == DXGI_FORMAT_D24_UNORM_S8_UINT {
            4
        } else {
            8
        };
        let mut texels = vec![0u8; width * height * size];
        for y in 0..height {
            for x in 0..width {
                let at = y * d.RowPitch as usize + x * 4;
                let mut value: [u8; 4] = depth[at..at + 4].try_into().unwrap();
                let stencil = stencil[y * s.RowPitch as usize + x];
                let to = (y * width + x) * size;
                if size == 4 {
                    value[3] = stencil;
                } else {
                    texels[to + 4] = stencil;
                }
                texels[to..to + 4].copy_from_slice(&value);
            }
        }
        texels::picture(format, d.Width, d.Height, &texels, width * size)
    }

    /// The footprint and bytes of subresource `index` of a 2D texture now in `state`.
    fn read_rows(
        &self,
        texture: &ID3D12Resource,
        index: u32,
        state: D3D12_RESOURCE_STATES,
    ) -> Result<(D3D12_SUBRESOURCE_FOOTPRINT, Vec<u8>)> {
        let desc = unsafe { texture.GetDesc() };
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut total = 0;
        unsafe {
            self.device.GetCopyableFootprints(
                &desc,
                index,
                1,
                0,
                Some(&mut footprint),
                None,
                None,
                Some(&mut total),
            )
        };
        let readback = self.committed(
            D3D12_HEAP_TYPE_READBACK,
            &buffer_desc(total),
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        )?;
        unsafe {
            if state != D3D12_RESOURCE_STATE_COPY_SOURCE {
                self.list.ResourceBarrier(&[transition(
                    texture,
                    state,
                    D3D12_RESOURCE_STATE_COPY_SOURCE,
                )]);
            }
            self.list.CopyTextureRegion(
                &footprint_location(&readback, footprint),
                0,
                0,
                0,
                &texture_location(texture, index),
                None,
            );
            if state != D3D12_RESOURCE_STATE_COPY_SOURCE {
                self.list.ResourceBarrier(&[transition(
                    texture,
                    D3D12_RESOURCE_STATE_COPY_SOURCE,
                    state,
                )]);
            }
        }
        self.submit()?;
        let f = footprint.Footprint;
        unsafe {
            let mut mapped = std::ptr::null_mut();
            readback.Map(
                0,
                Some(&D3D12_RANGE {
                    Begin: 0,
                    End: total as usize,
                }),
                Some(&mut mapped),
            )?;
            let rows = std::slice::from_raw_parts(mapped.cast::<u8>(), total as usize).to_vec();
            readback.Unmap(0, Some(&D3D12_RANGE::default()));
            Ok((f, rows))
        }
    }

    /// Reads back the first `size` bytes of a buffer now in `state`.
    pub fn read_buffer(
        &self,
        buffer: &ID3D12Resource,
        size: u64,
        state: D3D12_RESOURCE_STATES,
    ) -> Result<Vec<u8>> {
        let readback = self.committed(
            D3D12_HEAP_TYPE_READBACK,
            &buffer_desc(size),
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        )?;
        unsafe {
            self.list.ResourceBarrier(&[transition(
                buffer,
                state,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            )]);
            self.list.CopyBufferRegion(&readback, 0, buffer, 0, size);
            self.list.ResourceBarrier(&[transition(
                buffer,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
                state,
            )]);
        }
        self.submit()?;
        unsafe {
            let mut mapped = std::ptr::null_mut();
            readback.Map(
                0,
                Some(&D3D12_RANGE {
                    Begin: 0,
                    End: size as usize,
                }),
                Some(&mut mapped),
            )?;
            let bytes = std::slice::from_raw_parts(mapped.cast::<u8>(), size as usize).to_vec();
            readback.Unmap(0, Some(&D3D12_RANGE::default()));
            Ok(bytes)
        }
    }

    pub fn root_signature(
        &self,
        desc: &D3D12_VERSIONED_ROOT_SIGNATURE_DESC,
    ) -> Result<ID3D12RootSignature> {
        let mut blob = None;
        let mut errors = None;
        unsafe { D3D12SerializeVersionedRootSignature(desc, &mut blob, Some(&mut errors)) }
            .with_context(|| {
                errors
                    .map(|e| String::from_utf8_lossy(blob_bytes(&e)).into_owned())
                    .unwrap_or_default()
            })?;
        Ok(unsafe {
            self.device
                .CreateRootSignature(0, blob_bytes(&blob.context("no blob")?))?
        })
    }

    pub fn descriptor_heap(
        &self,
        kind: D3D12_DESCRIPTOR_HEAP_TYPE,
        count: u32,
        visible: bool,
    ) -> Result<ID3D12DescriptorHeap> {
        let flags = if visible {
            D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE
        } else {
            D3D12_DESCRIPTOR_HEAP_FLAG_NONE
        };
        Ok(unsafe {
            self.device
                .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                    Type: kind,
                    NumDescriptors: count,
                    Flags: flags,
                    NodeMask: 0,
                })?
        })
    }

    /// The CPU handle of descriptor `index` of a heap.
    pub fn cpu(&self, heap: &ID3D12DescriptorHeap, index: u32) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        let desc = unsafe { heap.GetDesc() };
        let step = unsafe { self.device.GetDescriptorHandleIncrementSize(desc.Type) };
        let start = unsafe { heap.GetCPUDescriptorHandleForHeapStart() };
        D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: start.ptr + (index * step) as usize,
        }
    }

    /// The GPU handle of descriptor `index` of a shader-visible heap.
    pub fn gpu(&self, heap: &ID3D12DescriptorHeap, index: u32) -> D3D12_GPU_DESCRIPTOR_HANDLE {
        let desc = unsafe { heap.GetDesc() };
        let step = unsafe { self.device.GetDescriptorHandleIncrementSize(desc.Type) };
        let start = unsafe { heap.GetGPUDescriptorHandleForHeapStart() };
        D3D12_GPU_DESCRIPTOR_HANDLE {
            ptr: start.ptr + (index * step) as u64,
        }
    }

    /// A render target of `format` in the render target state, cleared to `CLEAR` when bound by
    /// `begin`, and an RTV heap holding its view.
    pub fn target(
        &self,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
    ) -> Result<(ID3D12Resource, ID3D12DescriptorHeap)> {
        let clear = D3D12_CLEAR_VALUE {
            Format: format,
            Anonymous: D3D12_CLEAR_VALUE_0 {
                Color: crate::scene::CLEAR,
            },
        };
        let texture = self.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &tex2d_desc(
                width,
                height,
                format,
                D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
            ),
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            Some(&clear),
        )?;
        let heap = self.descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 1, false)?;
        unsafe {
            self.device
                .CreateRenderTargetView(&texture, None, self.cpu(&heap, 0))
        };
        Ok((texture, heap))
    }

    /// Clears and binds a target with a full viewport.
    pub fn begin(&self, heap: &ID3D12DescriptorHeap, width: u32, height: u32) {
        let rtv = self.cpu(heap, 0);
        unsafe {
            self.list
                .ClearRenderTargetView(rtv, &crate::scene::CLEAR, None);
            self.list.OMSetRenderTargets(1, Some(&rtv), false, None);
        }
        area(&self.list, 0, 0, width, height);
    }

    /// Fails on any error the debug layer reported, printing all its messages.
    pub fn check_debug(&self) -> Result<()> {
        if !self.debug {
            return Ok(());
        }
        let queue: ID3D12InfoQueue = self.device.cast()?;
        let mut messages = Vec::new();
        unsafe {
            for i in 0..queue.GetNumStoredMessages() {
                let mut length = 0;
                queue.GetMessage(i, None, &mut length)?;
                let mut storage = vec![0u64; length.div_ceil(8)];
                let message = storage.as_mut_ptr().cast::<D3D12_MESSAGE>();
                queue.GetMessage(i, Some(message), &mut length)?;
                let message = &*message;
                messages.push((
                    message.Severity.0,
                    debug::text(message.pDescription, message.DescriptionByteLength),
                ));
            }
            queue.ClearStoredMessages();
        }
        debug::check(messages)
    }
}

pub fn texture_location(resource: &ID3D12Resource, index: u32) -> D3D12_TEXTURE_COPY_LOCATION {
    D3D12_TEXTURE_COPY_LOCATION {
        pResource: unsafe { transmute_copy(resource) },
        Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
            SubresourceIndex: index,
        },
    }
}

pub fn footprint_location(
    resource: &ID3D12Resource,
    footprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
) -> D3D12_TEXTURE_COPY_LOCATION {
    D3D12_TEXTURE_COPY_LOCATION {
        pResource: unsafe { transmute_copy(resource) },
        Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
            PlacedFootprint: footprint,
        },
    }
}

pub fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    unsafe { std::slice::from_raw_parts(blob.GetBufferPointer().cast(), blob.GetBufferSize()) }
}
