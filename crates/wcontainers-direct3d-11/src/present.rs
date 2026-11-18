//! Swap chains: the shared infrastructure's, with each back buffer handed to the program as a
//! texture of this front.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wcontainers_directx_graphics_infrastructure::{SwapChain, SwapChainDesc, SwapChainError};
use wcontainers_gpu::{Backend, GpuError};

use crate::desc::{TexDim, TextureDesc, bind};
use crate::object::{Kept, Texture};
use crate::{Device, Error, Result};

/// A swap chain and the textures standing for its back buffers.
pub struct Chain<B: Backend> {
    chain: SwapChain<B>,
    buffers: Vec<Rc<Texture<B>>>,
    /// Bind flags of the back buffers (render target, and shader resource or unordered access as
    /// the swap chain's usage asks).
    bind: u32,
}

impl<B: Backend> Chain<B> {
    pub fn desc(&self) -> &SwapChainDesc {
        self.chain.desc()
    }

    /// Back buffer `index` (`GetBuffer`): any of a flip model's, buffer 0 of a blit model's.
    pub fn buffer(&self, index: u32) -> Option<&Rc<Texture<B>>> {
        self.buffers.get(index as usize)
    }

    /// How many presents were made (`GetLastPresentCount`).
    pub fn present_count(&self) -> u64 {
        self.chain.present_count()
    }
}

fn chain_error(e: SwapChainError) -> Error {
    match e {
        SwapChainError::InvalidCall(why) => Error::InvalidCall(why),
        SwapChainError::Gpu(GpuError::OutOfMemory) => Error::OutOfMemory,
        SwapChainError::Gpu(_) => Error::DeviceRemoved,
    }
}

impl<B: Backend> Device<B> {
    /// A swap chain on host window `window`, its back buffers bound as `bind` says.
    pub fn create_swap_chain(
        &mut self,
        window: u32,
        desc: &SwapChainDesc,
        bind: u32,
    ) -> Result<Chain<B>> {
        self.live()?;
        self.bury();
        let chain =
            SwapChain::new(&mut self.backend, window, desc).map_err(|e| self.chain_lost(e))?;
        let mut chain = Chain {
            chain,
            buffers: Vec::new(),
            bind: bind | bind::RENDER_TARGET,
        };
        self.adopt_buffers(&mut chain);
        Ok(chain)
    }

    fn chain_lost(&mut self, e: SwapChainError) -> Error {
        let e = chain_error(e);
        if e == Error::DeviceRemoved {
            self.lose(e);
        }
        e
    }

    fn adopt_buffers(&mut self, c: &mut Chain<B>) {
        let gpu_desc = c.chain.buffer_desc();
        let d = *c.chain.desc();
        let mut index = 0;
        while let Some(texture) = c.chain.buffer(index) {
            let id = self.id();
            c.buffers.push(Rc::new(Texture {
                desc: TextureDesc {
                    dimension: TexDim::D2,
                    width: d.width,
                    height: d.height,
                    depth: 1,
                    mip_levels: 1,
                    array_size: 1,
                    format: d.format,
                    sample_count: 1,
                    sample_quality: 0,
                    usage: 0,
                    bind_flags: c.bind,
                    cpu_access_flags: 0,
                    misc_flags: 0,
                },
                format: gpu_desc.format,
                gpu_desc,
                gpu: Kept::borrowed(texture.clone(), id, &self.graveyard),
                shadow: RefCell::new(Vec::new()),
                staged: RefCell::new(Vec::new()),
                chain: true,
                min_lod: Cell::new(0.0),
            }));
            index += 1;
        }
    }

    /// `Present`: the work recorded so far, then the frame shown.
    pub fn present(&mut self, c: &mut Chain<B>, sync_interval: u32) -> Result<()> {
        self.live()?;
        self.settle();
        self.report_ahead();
        c.chain
            .present(&mut self.backend, sync_interval)
            .map_err(|e| self.chain_lost(e))
    }

    /// `ResizeBuffers`: refused while the program or a binding holds a back buffer, as DXGI does.
    pub fn resize_buffers(
        &mut self,
        c: &mut Chain<B>,
        count: u32,
        width: u32,
        height: u32,
        format: u32,
    ) -> Result<()> {
        self.live()?;
        if c.buffers.iter().any(|b| Rc::strong_count(b) > 1) {
            return Err(Error::InvalidCall("a back buffer is still referenced"));
        }
        self.settle();
        c.buffers.clear();
        self.bury();
        c.chain
            .resize(&mut self.backend, count, width, height, format)
            .map_err(|e| self.chain_lost(e))?;
        self.adopt_buffers(c);
        Ok(())
    }

    /// Destroys the swap chain once the program released it.
    pub fn destroy_chain(&mut self, c: Chain<B>) {
        self.settle();
        drop(c.buffers);
        self.bury();
        c.chain.destroy(&mut self.backend);
    }
}
