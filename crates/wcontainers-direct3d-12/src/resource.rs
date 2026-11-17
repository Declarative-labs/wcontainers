//! Resource descriptions and how their bytes lie in buffers (`GetCopyableFootprints`).

use wcontainers_directx_graphics_infrastructure::dxgi_format;
use wcontainers_gpu::Format;

use crate::Error;

/// `D3D12_RESOURCE_DIMENSION`.
pub const DIMENSION_BUFFER: u32 = 1;
pub const DIMENSION_TEXTURE1D: u32 = 2;
pub const DIMENSION_TEXTURE2D: u32 = 3;
pub const DIMENSION_TEXTURE3D: u32 = 4;

/// `D3D12_RESOURCE_FLAGS` bits.
pub const FLAG_RENDER_TARGET: u32 = 1;
pub const FLAG_DEPTH_STENCIL: u32 = 2;
pub const FLAG_UNORDERED_ACCESS: u32 = 4;
pub const FLAG_DENY_SHADER_RESOURCE: u32 = 8;

/// Rows of a texture in a buffer start at multiples of this (`D3D12_TEXTURE_DATA_PITCH_ALIGNMENT`).
pub const PITCH_ALIGNMENT: u64 = 256;
/// Subresources in a buffer start at multiples of this (`D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT`).
pub const PLACEMENT_ALIGNMENT: u64 = 512;
/// `DXGI_FORMAT_R32_TYPELESS` and `DXGI_FORMAT_R8_TYPELESS`: the depth and stencil planes' formats.
const R32_TYPELESS: u32 = 39;
const R8_TYPELESS: u32 = 60;

/// `D3D12_RESOURCE_DESC`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceDesc {
    pub dimension: u32,
    pub alignment: u64,
    pub width: u64,
    pub height: u32,
    pub depth_or_array_size: u16,
    pub mip_levels: u16,
    pub format: u32,
    pub sample_count: u32,
    pub sample_quality: u32,
    pub layout: u32,
    pub flags: u32,
}

/// `D3D12_HEAP_TYPE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeapType {
    Default,
    /// Memory the program writes and the GPU reads.
    Upload,
    /// Memory the GPU writes and the program reads.
    Readback,
}

/// `D3D12_HEAP_FLAGS`: every defined bit, the deny flags, and the ones this front honours as they
/// are (resources are always resident, and memory is zeroed anyway).
const HEAP_FLAGS_DEFINED: u32 =
    0x1 | 0x4 | 0x8 | 0x20 | 0x40 | 0x80 | 0x100 | 0x200 | 0x400 | 0x800 | 0x1000 | 0x2000;
const HEAP_FLAGS_DENY: u32 = 0x4 | 0x40 | 0x80;
const HEAP_FLAGS_TAKEN: u32 = 0x800 | 0x1000;

/// Checks heap flags: refuses undefined bits and, for committed resources (`committed`), the deny
/// flags, as Direct3D 12 does; reports as not done those asking for an ability the front lacks
/// (sharing, display, protected memory, write watching, cross-adapter atomics, manual write
/// tracking).
pub fn check_heap_flags(flags: u32, committed: bool) -> Result<(), Error> {
    if flags & !HEAP_FLAGS_DEFINED != 0 || (committed && flags & HEAP_FLAGS_DENY != 0) {
        return Err(Error::Invalid("heap flags"));
    }
    if flags & !(HEAP_FLAGS_TAKEN | HEAP_FLAGS_DENY) != 0 {
        return Err(Error::NotImplemented(
            "heap flags for sharing, display, protection, write watching or atomics",
        ));
    }
    Ok(())
}

impl HeapType {
    pub fn from_d3d12(value: u32) -> Result<HeapType, Error> {
        match value {
            1 => Ok(HeapType::Default),
            2 => Ok(HeapType::Upload),
            3 => Ok(HeapType::Readback),
            4 => Err(Error::NotImplemented("custom heaps")),
            _ => Err(Error::Invalid("unknown heap type")),
        }
    }
}

/// `D3D12_SUBRESOURCE_FOOTPRINT` placed at an offset: where one subresource's texels lie in a buffer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Footprint {
    pub offset: u64,
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub row_pitch: u32,
}

/// One subresource's footprint with its row count and the bytes of one row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FootprintInfo {
    pub footprint: Footprint,
    pub rows: u32,
    pub row_size: u64,
}

impl ResourceDesc {
    /// Mip levels, with zero meaning the full chain.
    pub fn mips(&self) -> u32 {
        if self.mip_levels != 0 || self.dimension == DIMENSION_BUFFER {
            return u32::from(self.mip_levels).max(1);
        }
        let depth = if self.dimension == DIMENSION_TEXTURE3D {
            u64::from(self.depth_or_array_size)
        } else {
            1
        };
        let largest = self.width.max(u64::from(self.height)).max(depth).max(1);
        64 - largest.leading_zeros()
    }

    /// Array layers (one for 3D textures and buffers).
    /// Planes: two for formats with depth and stencil, whose subresources are each plane's in turn.
    pub fn planes(&self) -> Result<u32, Error> {
        let (format, _) = self.texture_format()?;
        Ok(match format.kind() {
            wcontainers_gpu::FormatKind::DepthStencil => 2,
            _ => 1,
        })
    }

    pub fn subresources(&self) -> Result<u32, Error> {
        Ok(self.mips() * self.layers() * self.planes()?)
    }

    pub fn layers(&self) -> u32 {
        match self.dimension {
            DIMENSION_TEXTURE1D | DIMENSION_TEXTURE2D => u32::from(self.depth_or_array_size).max(1),
            _ => 1,
        }
    }

    /// The interface format of a texture and whether its DXGI format is typeless.
    pub fn texture_format(&self) -> Result<(Format, bool), Error> {
        dxgi_format(self.format).ok_or(Error::Invalid("a format the front cannot store"))
    }

    /// `GetResourceAllocationInfo` for this resource: its size and alignment in a heap. Buffers and
    /// textures take 64 KiB pages (4 MiB for multisampled textures); a texture's bytes are its
    /// subresources' copyable footprints for each sample, the layout this front keeps them in.
    pub fn allocation(&self) -> Result<(u64, u64), Error> {
        const PAGE: u64 = 64 << 10;
        const MSAA_PAGE: u64 = 4 << 20;
        let align = if self.sample_count > 1 {
            MSAA_PAGE
        } else {
            PAGE
        };
        let bytes = if self.dimension == DIMENSION_BUFFER {
            self.width
        } else {
            self.texture_format()?;
            self.footprints(0, self.subresources()?, 0)?.1 * u64::from(self.sample_count.max(1))
        };
        Ok((bytes.next_multiple_of(align), align))
    }

    /// The footprints of subresources `first..first + count`, laid out from `base_offset`, and the
    /// bytes they cover.
    pub fn footprints(
        &self,
        first: u32,
        count: u32,
        base_offset: u64,
    ) -> Result<(Vec<FootprintInfo>, u64), Error> {
        if ![0, 4 << 10, 64 << 10, 4 << 20].contains(&self.alignment) {
            return Err(Error::Invalid("a resource's alignment"));
        }
        if self.dimension == DIMENSION_BUFFER {
            if first != 0 || count > 1 {
                return Err(Error::Invalid("a buffer has one subresource"));
            }
            if self.height != 1 || self.depth_or_array_size != 1 || self.mip_levels != 1 {
                return Err(Error::Invalid("a buffer is one row of one level"));
            }
            let info = FootprintInfo {
                footprint: Footprint {
                    offset: base_offset,
                    format: 0,
                    width: self.width as u32,
                    height: 1,
                    depth: 1,
                    row_pitch: self.width.next_multiple_of(PITCH_ALIGNMENT) as u32,
                },
                rows: 1,
                row_size: self.width,
            };
            return Ok((vec![info; count as usize], self.width * u64::from(count)));
        }
        let (format, _) = self.texture_format()?;
        if self.dimension == DIMENSION_TEXTURE1D && self.height != 1 {
            return Err(Error::Invalid("a one-dimensional texture is one row"));
        }
        // Block-compressed textures are whole blocks at level 0 (unaligned blocks are not reported).
        let (bw, bh, _) = format.block();
        if !self.width.is_multiple_of(u64::from(bw)) || !self.height.is_multiple_of(bh) {
            return Err(Error::Invalid(
                "a block-compressed texture of part of a block",
            ));
        }
        let planar = self.planes()? == 2;
        let mips = self.mips();
        if first
            .checked_add(count)
            .is_none_or(|end| end > self.subresources().unwrap_or(0))
        {
            return Err(Error::Invalid("subresource out of range"));
        }
        // Offsets from the base, which the placement alignment applies to.
        let mut offset = 0u64;
        let mut out = Vec::with_capacity(count as usize);
        let mut total = 0;
        for index in first..first + count {
            let mip = index % mips;
            // Planar depth-stencil formats are copied as a 32-bit depth plane and an 8-bit stencil
            // plane, with rows aligned twice as far, as Direct3D 12 lays them out on the desktop.
            let (bw, bh, bytes, plane_format, pitch_alignment) =
                match (planar, index / (mips * self.layers())) {
                    (false, _) => {
                        let (bw, bh, bytes) = format.block();
                        (bw, bh, bytes, self.format, PITCH_ALIGNMENT)
                    }
                    (true, 0) => (1, 1, 4, R32_TYPELESS, 2 * PITCH_ALIGNMENT),
                    (true, _) => (1, 1, 1, R8_TYPELESS, 2 * PITCH_ALIGNMENT),
                };
            let size = |n: u64| (n >> mip).max(1);
            let width = size(self.width) as u32;
            let height = size(u64::from(self.height)) as u32;
            let depth = if self.dimension == DIMENSION_TEXTURE3D {
                size(u64::from(self.depth_or_array_size)) as u32
            } else {
                1
            };
            let rows = height.div_ceil(bh);
            let row_size = u64::from(width.div_ceil(bw)) * u64::from(bytes);
            let row_pitch = row_size.next_multiple_of(pitch_alignment);
            offset = offset.next_multiple_of(PLACEMENT_ALIGNMENT);
            // Offsets wrap as the API's unsigned arithmetic does.
            let at = base_offset.wrapping_add(offset);
            out.push(FootprintInfo {
                footprint: Footprint {
                    offset: at,
                    format: plane_format,
                    width: width.next_multiple_of(bw),
                    height: height.next_multiple_of(bh),
                    depth,
                    row_pitch: row_pitch as u32,
                },
                rows,
                row_size,
            });
            let size = row_pitch * u64::from(rows) * u64::from(depth - 1)
                + row_pitch * u64::from(rows - 1)
                + row_size;
            total = offset + size;
            offset += row_pitch * u64::from(rows) * u64::from(depth);
        }
        Ok((out, total))
    }
}
