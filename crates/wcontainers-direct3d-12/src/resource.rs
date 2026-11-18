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
    /// Memory of a custom heap the CPU sees, which both write and read. Its buffers are written to
    /// the GPU from the program's memory as upload buffers are, and what the GPU writes into them
    /// is read back as into readback buffers; its textures are stored as default ones.
    Shared,
}

/// `D3D12_HEAP_FLAGS`: every bit this desktop's Windows 10 runtime takes, and the deny flags.
const HEAP_FLAGS_DEFINED: u32 =
    0x1 | 0x4 | 0x8 | 0x20 | 0x40 | 0x80 | 0x100 | 0x200 | 0x800 | 0x1000 | 0x2000;
const HEAP_FLAGS_DENY: u32 = 0x4 | 0x40 | 0x80;
/// `ALLOW_SHADER_ATOMICS`: that runtime takes it for committed resources allowing unordered
/// access and refuses it otherwise (measured by `check_d3d12 answers`). Atomics on one adapter
/// need nothing of the memory.
const HEAP_FLAG_SHADER_ATOMICS: u32 = 0x400;

/// The heap flags asking for an ability the front lacks, each with its reason. The others are
/// honoured as they are: resources are always resident and memory is zeroed anyway.
const HEAP_FLAGS_REFUSED: [(u32, &str); 6] = [
    (0x1, "shared heaps"),
    (0x8, "heaps allowing display"),
    (0x20, "heaps shared across adapters"),
    (0x100, "hardware-protected heaps"),
    (0x200, "heaps allowing write watching"),
    (0x2000, "heaps with manual write tracking"),
];

/// Checks heap flags: refuses undefined bits and, for committed resources (`committed`, whether
/// the resource allows unordered access), the deny flags, as Direct3D 12 does; reports as not
/// done those asking for an ability the front lacks.
pub fn check_heap_flags(flags: u32, committed: Option<bool>) -> Result<(), Error> {
    let defined = match committed {
        Some(true) => HEAP_FLAGS_DEFINED | HEAP_FLAG_SHADER_ATOMICS,
        _ => HEAP_FLAGS_DEFINED,
    };
    if flags & !defined != 0 || (committed.is_some() && flags & HEAP_FLAGS_DENY != 0) {
        return Err(Error::Invalid("heap flags"));
    }
    match HEAP_FLAGS_REFUSED.iter().find(|(bit, _)| flags & bit != 0) {
        Some(&(_, reason)) => Err(Error::NotImplemented(reason)),
        None => Ok(()),
    }
}

/// `CreateHeap`'s checks of what the description asks beyond its type: an alignment of 0, 64 KiB
/// or 4 MiB, and no display flag (only committed resources are displayed).
pub fn check_heap(alignment: u64, flags: u32) -> Result<(), Error> {
    if ![0, 64 << 10, 4 << 20].contains(&alignment) {
        return Err(Error::Invalid("a heap's alignment"));
    }
    if flags & 0x8 != 0 {
        return Err(Error::Invalid("a heap allowing display"));
    }
    check_heap_flags(flags, None)
}

/// `D3D12_CPU_PAGE_PROPERTY` values.
const PAGE_NOT_AVAILABLE: u32 = 1;
const PAGE_WRITE_COMBINE: u32 = 2;
const PAGE_WRITE_BACK: u32 = 3;
/// `D3D12_MEMORY_POOL_L0` and `_L1`: system memory and the adapter's own.
const POOL_L0: u32 = 1;
const POOL_L1: u32 = 2;

/// The heap a custom heap's properties name on a discrete adapter (the one `ARCHITECTURE`
/// reports). Memory the CPU does not see is a default heap; the adapter's own memory is never
/// CPU-visible (no GPU upload heaps).
pub fn custom_heap(page: u32, pool: u32) -> Result<HeapType, Error> {
    match (page, pool) {
        (PAGE_NOT_AVAILABLE, POOL_L0 | POOL_L1) => Ok(HeapType::Default),
        (PAGE_WRITE_COMBINE | PAGE_WRITE_BACK, POOL_L0) => Ok(HeapType::Shared),
        _ => Err(Error::Invalid("custom heap properties")),
    }
}

/// `D3D12_RESOURCE_STATES` that write; none combines with another state.
const WRITE_STATES: u32 = 0x4 | 0x8 | 0x10 | 0x100 | 0x400 | 0x1000;
const STATE_RENDER_TARGET: u32 = 0x4;
const STATE_DEPTH_WRITE: u32 = 0x10;
const STATE_COPY_DEST: u32 = 0x400;
/// `D3D12_RESOURCE_STATE_GENERIC_READ`.
const STATE_GENERIC_READ: u32 = 0xac3;
/// `D3D12_RESOURCE_FLAG_ALLOW_SIMULTANEOUS_ACCESS`.
const FLAG_SIMULTANEOUS_ACCESS: u32 = 0x20;

/// A resource state as Direct3D 12 takes one: exactly one write state, or read states only.
pub fn check_state(state: u32) -> Result<(), Error> {
    if state & WRITE_STATES != 0 && !state.is_power_of_two() {
        return Err(Error::Invalid("a write state combined with another"));
    }
    Ok(())
}

/// The checks of `CreateCommittedResource` and `CreatePlacedResource` that Direct3D 12 makes of a
/// description, its heap, its initial state and whether a clear value is given.
pub fn check_creation(
    heap: HeapType,
    desc: &ResourceDesc,
    state: u32,
    clear: bool,
) -> Result<(), Error> {
    check_state(state)?;
    if desc.dimension == DIMENSION_BUFFER {
        if clear {
            return Err(Error::Invalid("a clear value for a buffer"));
        }
        if desc.mip_levels != 1 || desc.layout != 1 || desc.format != 0 {
            return Err(Error::Invalid("a buffer's description"));
        }
        if desc.flags & (FLAG_RENDER_TARGET | FLAG_DEPTH_STENCIL | FLAG_SIMULTANEOUS_ACCESS) != 0
            || (heap.cpu_only() && desc.flags & FLAG_UNORDERED_ACCESS != 0)
        {
            return Err(Error::Invalid("a buffer's flags"));
        }
        let state_ok = match heap {
            HeapType::Upload => state == 0 || state == STATE_GENERIC_READ,
            HeapType::Readback => state == 0 || state == STATE_COPY_DEST,
            HeapType::Default | HeapType::Shared => true,
        };
        if !state_ok {
            return Err(Error::Invalid(
                "the initial state of an upload or readback buffer",
            ));
        }
        if heap == HeapType::Shared && desc.flags & FLAG_UNORDERED_ACCESS != 0 {
            // Its storage writes would have to be read back after every draw and dispatch.
            return Err(Error::NotImplemented(
                "unordered access buffers in a custom heap the CPU sees",
            ));
        }
        return Ok(());
    }
    if heap.cpu_only() {
        return Err(Error::Invalid("textures live in default or custom heaps"));
    }
    if desc.sample_count == 0 || desc.format == 0 {
        return Err(Error::Invalid("a texture's sample count or format"));
    }
    if desc.sample_count > 1 && desc.flags & (FLAG_RENDER_TARGET | FLAG_DEPTH_STENCIL) == 0 {
        return Err(Error::Invalid("a multisampled texture that is no target"));
    }
    if (state == STATE_RENDER_TARGET && desc.flags & FLAG_RENDER_TARGET == 0)
        || (state == STATE_DEPTH_WRITE && desc.flags & FLAG_DEPTH_STENCIL == 0)
    {
        return Err(Error::Invalid("a target state without the target flag"));
    }
    let (format, _) = desc.texture_format()?;
    let (bw, bh, _) = format.block();
    if bw > 1
        && (desc.dimension != DIMENSION_TEXTURE2D && desc.dimension != DIMENSION_TEXTURE3D
            || !desc.width.is_multiple_of(u64::from(bw))
            || !desc.height.is_multiple_of(bh))
    {
        return Err(Error::Invalid(
            "a block-compressed texture of part of a block",
        ));
    }
    Ok(())
}

impl HeapType {
    /// Upload and readback heaps, which hold buffers only.
    pub fn cpu_only(self) -> bool {
        matches!(self, HeapType::Upload | HeapType::Readback)
    }

    /// Buffers whose bytes the program writes, written to the GPU before work reading them.
    pub fn uploads(self) -> bool {
        matches!(self, HeapType::Upload | HeapType::Shared)
    }

    /// Buffers whose bytes the GPU writes, read back into the program's memory.
    pub fn reads_back(self) -> bool {
        matches!(self, HeapType::Readback | HeapType::Shared)
    }

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
        self.full_chain()
    }

    /// The levels down to one texel.
    pub fn full_chain(&self) -> u32 {
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

    /// The interface format of a texture and whether its DXGI format is typeless. A typeless
    /// texture allowing depth-stencil views whose family has a depth format is stored in it (as
    /// `R24G8` and `R32G8X24` are), and views of the family's colour formats read its depth.
    pub fn texture_format(&self) -> Result<(Format, bool), Error> {
        let (format, typeless) =
            dxgi_format(self.format).ok_or(Error::Invalid("a format the front cannot store"))?;
        if typeless && self.flags & FLAG_DEPTH_STENCIL != 0 {
            match format {
                Format::R32Float => return Ok((Format::Depth32Float, false)),
                Format::R16Float => return Ok((Format::Depth16Unorm, false)),
                _ => {}
            }
        }
        Ok((format, typeless))
    }

    /// The depth format whose depth a view of colour format `view` reads.
    pub fn depth_read_as(view: Format) -> Option<Format> {
        match view {
            Format::R32Float => Some(Format::Depth32Float),
            Format::R16Unorm => Some(Format::Depth16Unorm),
            _ => None,
        }
    }

    /// `GetResourceAllocationInfo` for this resource: its size and alignment in a heap. Buffers and
    /// textures take 64 KiB pages (4 MiB for multisampled textures); a texture's bytes are its
    /// subresources' copyable footprints for each sample, the layout this front keeps them in. The
    /// smaller alignment Direct3D 12 allows is taken when asked for and the most detailed level is
    /// small enough: 4 KiB for textures that are no target with at most 64 KiB, 64 KiB for
    /// multisampled ones with at most 4 MiB; any other alignment asked for is refused.
    pub fn allocation(&self) -> Result<(u64, u64), Error> {
        const SMALL: u64 = 4 << 10;
        const PAGE: u64 = 64 << 10;
        const MSAA_PAGE: u64 = 4 << 20;
        /// `D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET` and `..._ALLOW_DEPTH_STENCIL`.
        const TARGETS: u32 = 0x3;
        let samples = u64::from(self.sample_count.max(1));
        let natural = if samples > 1 { MSAA_PAGE } else { PAGE };
        let bytes = if self.dimension == DIMENSION_BUFFER {
            if ![0, PAGE].contains(&self.alignment) {
                return Err(Error::Invalid("a buffer's alignment"));
            }
            self.width
        } else {
            self.texture_format()?;
            self.footprints(0, self.subresources()?, 0)?.1 * samples
        };
        let align = match self.alignment {
            0 => natural,
            a if a == natural => natural,
            a if self.dimension != DIMENSION_BUFFER => {
                let top = self.footprints(0, 1, 0)?.0[0];
                let top = top.row_size * u64::from(top.rows) * u64::from(top.footprint.depth);
                let small = match a {
                    SMALL => samples == 1 && self.flags & TARGETS == 0 && top <= PAGE,
                    PAGE => samples > 1 && top * samples <= MSAA_PAGE,
                    _ => false,
                };
                if !small {
                    return Err(Error::Invalid("a texture's alignment"));
                }
                a
            }
            _ => return Err(Error::Invalid("a buffer's alignment")),
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
