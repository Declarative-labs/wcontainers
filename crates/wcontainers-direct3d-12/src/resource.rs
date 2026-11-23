//! Resource descriptions and how their bytes lie in buffers (`GetCopyableFootprints`).

use wcontainers_directx_graphics_infrastructure::d3d::{
    DEFAULT_MSAA_RESOURCE_PLACEMENT_ALIGNMENT, DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT,
    SMALL_RESOURCE_PLACEMENT_ALIGNMENT, TEXTURE_DATA_PLACEMENT_ALIGNMENT,
};
use wcontainers_directx_graphics_infrastructure::dxgi::{
    NV12, OPAQUE_420, R8_TYPELESS, R8G8_TYPELESS, R10G10B10_XR_BIAS_A2_UNORM, R32_TYPELESS, YUY2,
};
use wcontainers_directx_graphics_infrastructure::dxgi_format;
use wcontainers_gpu::Format;

use crate::Error;

/// Whether DXGI format `format` is held as a luma texture and a chroma texture of half the width
/// and height (plan 4.12): `NV12`, and `420_OPAQUE`, which is stored as `NV12`.
pub fn has_chroma_plane(format: u32) -> bool {
    matches!(format, NV12 | OPAQUE_420)
}

/// The pixels one stored texel of DXGI format `format` holds: two for `YUY2`, whose pixel pairs
/// are stored as `rgba8unorm` texels (plan 4.12), one otherwise.
pub fn pixels_per_texel(format: u32) -> u32 {
    if format == YUY2 { 2 } else { 1 }
}

/// One subresource's shape: its block (width, height, bytes), the DXGI format its footprint
/// names, the alignment of its rows in a buffer, and its size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubresourceShape {
    pub block: (u32, u32, u32),
    pub format: u32,
    pub pitch_alignment: u64,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

/// `D3D12_RESOURCE_DIMENSION`.
pub const DIMENSION_BUFFER: u32 = 1;
pub const DIMENSION_TEXTURE1D: u32 = 2;
pub const DIMENSION_TEXTURE2D: u32 = 3;
pub const DIMENSION_TEXTURE3D: u32 = 4;

/// `D3D12_TEXTURE_LAYOUT_ROW_MAJOR`, the one layout of buffers.
pub const LAYOUT_ROW_MAJOR: u32 = 1;

/// `D3D12_RESOURCE_FLAGS` bits.
pub const FLAG_RENDER_TARGET: u32 = 1;
pub const FLAG_DEPTH_STENCIL: u32 = 2;
pub const FLAG_UNORDERED_ACCESS: u32 = 4;
pub const FLAG_DENY_SHADER_RESOURCE: u32 = 8;

/// Rows of a texture in a buffer start at multiples of this (`D3D12_TEXTURE_DATA_PITCH_ALIGNMENT`).
pub const PITCH_ALIGNMENT: u64 = 256;
/// Constant buffer views start at, and span, multiples of this
/// (`D3D12_CONSTANT_BUFFER_DATA_PLACEMENT_ALIGNMENT`).
pub const CONSTANT_BUFFER_ALIGNMENT: u64 = 256;

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

/// `SHARED`, `ALLOW_DISPLAY`, `SHARED_CROSS_ADAPTER`, `HARDWARE_PROTECTED`, `ALLOW_WRITE_WATCH` and
/// `TOOLS_USE_MANUAL_WRITE_TRACKING`.
pub const HEAP_FLAG_SHARED: u32 = 0x1;
pub const HEAP_FLAG_ALLOW_DISPLAY: u32 = 0x8;
pub const HEAP_FLAG_SHARED_CROSS_ADAPTER: u32 = 0x20;
pub const HEAP_FLAG_HARDWARE_PROTECTED: u32 = 0x100;
pub const HEAP_FLAG_ALLOW_WRITE_WATCH: u32 = 0x200;
pub const HEAP_FLAG_MANUAL_WRITE_TRACKING: u32 = 0x2000;

/// The heap flags asking for an ability the front lacks, each with its reason. The others are
/// honoured as they are: resources are always resident and memory is zeroed anyway.
const HEAP_FLAGS_REFUSED: [(u32, &str); 6] = [
    (HEAP_FLAG_SHARED, "shared heaps"),
    (HEAP_FLAG_ALLOW_DISPLAY, "heaps allowing display"),
    (
        HEAP_FLAG_SHARED_CROSS_ADAPTER,
        "heaps shared across adapters",
    ),
    (HEAP_FLAG_HARDWARE_PROTECTED, "hardware-protected heaps"),
    (HEAP_FLAG_ALLOW_WRITE_WATCH, "heaps allowing write watching"),
    (
        HEAP_FLAG_MANUAL_WRITE_TRACKING,
        "heaps with manual write tracking",
    ),
];

/// Checks heap flags for a heap of type `heap`: refuses undefined bits and, for a committed
/// resource described by `committed`, the deny flags, as Direct3D 12 does; refuses as invalid the
/// flags Direct3D 12 refuses where they are (`check_d3d12 answers`): cross-adapter sharing without
/// sharing, hardware protection without a protected session (the front offers none), write
/// tracking on a default heap and display on a buffer; reports as not done those asking for an
/// ability the front lacks.
pub fn check_heap_flags(
    flags: u32,
    heap: HeapType,
    committed: Option<&ResourceDesc>,
) -> Result<(), Error> {
    let defined = match committed {
        Some(d) if d.flags & FLAG_UNORDERED_ACCESS != 0 => {
            HEAP_FLAGS_DEFINED | HEAP_FLAG_SHADER_ATOMICS
        }
        _ => HEAP_FLAGS_DEFINED,
    };
    if flags & !defined != 0 || (committed.is_some() && flags & HEAP_FLAGS_DENY != 0) {
        return Err(Error::Invalid("heap flags"));
    }
    let has = |bit: u32| flags & bit != 0;
    let tracked = HEAP_FLAG_ALLOW_WRITE_WATCH | HEAP_FLAG_MANUAL_WRITE_TRACKING;
    if has(HEAP_FLAG_SHARED_CROSS_ADAPTER) && !has(HEAP_FLAG_SHARED)
        || has(HEAP_FLAG_HARDWARE_PROTECTED)
        || has(tracked) && heap == HeapType::Default
        || has(HEAP_FLAG_ALLOW_DISPLAY)
            && committed.is_some_and(|d| d.dimension == DIMENSION_BUFFER)
    {
        return Err(Error::Invalid("heap flags refused where they are"));
    }
    match HEAP_FLAGS_REFUSED.iter().find(|(bit, _)| has(*bit)) {
        Some(&(_, reason)) => Err(Error::NotImplemented(reason)),
        None => Ok(()),
    }
}

/// `CreateHeap`'s checks of what the description asks beyond its type: an alignment of 0, 64 KiB
/// or 4 MiB, and no display flag (only committed resources are displayed).
pub fn check_heap(alignment: u64, heap: HeapType, flags: u32) -> Result<(), Error> {
    if ![
        0,
        DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT,
        DEFAULT_MSAA_RESOURCE_PLACEMENT_ALIGNMENT,
    ]
    .contains(&alignment)
    {
        return Err(Error::Invalid("a heap's alignment"));
    }
    if flags & HEAP_FLAG_ALLOW_DISPLAY != 0 {
        return Err(Error::Invalid("a heap allowing display"));
    }
    check_heap_flags(flags, heap, None)
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
const WRITE_STATES: u32 = STATE_RENDER_TARGET
    | STATE_UNORDERED_ACCESS
    | STATE_DEPTH_WRITE
    | STATE_STREAM_OUT
    | STATE_COPY_DEST
    | STATE_RESOLVE_DEST;
const STATE_RENDER_TARGET: u32 = 0x4;
const STATE_UNORDERED_ACCESS: u32 = 0x8;
const STATE_DEPTH_WRITE: u32 = 0x10;
const STATE_STREAM_OUT: u32 = 0x100;
const STATE_COPY_DEST: u32 = 0x400;
const STATE_RESOLVE_DEST: u32 = 0x1000;
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
        if desc.mip_levels != 1 || desc.layout != LAYOUT_ROW_MAJOR || desc.format != 0 {
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
    // The video formats are 2D textures of one level and sample, of whole pixel pairs (plan 4.12).
    if has_chroma_plane(desc.format) || desc.format == YUY2 {
        let pairs =
            desc.width.is_multiple_of(2) && (desc.format == YUY2 || desc.height.is_multiple_of(2));
        if desc.dimension != DIMENSION_TEXTURE2D
            || desc.mips() != 1
            || desc.sample_count != 1
            || !pairs
        {
            return Err(Error::Invalid(
                "a video format's texture of more than one level or sample, of another dimension \
                 than 2D, or of part of a pixel pair",
            ));
        }
    }
    // Blocks of one texel's height (the two-pixel formats) also make 1D textures.
    let (bw, bh, _) = format.block();
    if bw > 1
        && (desc.dimension == DIMENSION_TEXTURE1D && bh > 1
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

    /// Planes: two for formats with depth and stencil and for `NV12`, whose subresources are each
    /// plane's in turn (`420_OPAQUE` is one plane to the program).
    pub fn planes(&self) -> Result<u32, Error> {
        let (format, _) = self.texture_format()?;
        Ok(match format.kind() {
            wcontainers_gpu::FormatKind::DepthStencil => 2,
            _ if self.format == NV12 => 2,
            _ => 1,
        })
    }

    /// The description of the texture holding the chroma plane of a format with one
    /// (`has_chroma_plane`): `R8G8` at half the width and height.
    pub fn chroma(&self) -> Option<ResourceDesc> {
        has_chroma_plane(self.format).then_some(ResourceDesc {
            format: R8G8_TYPELESS,
            width: self.width / 2,
            height: self.height / 2,
            ..*self
        })
    }

    /// The texels the stored texture is wide: the width in pixels over `pixels_per_texel`.
    pub fn stored_width(&self) -> u64 {
        self.width / u64::from(pixels_per_texel(self.format))
    }

    /// The shape of subresource `index`. Planar formats' rows are aligned twice as far, as
    /// Direct3D 12 lays them out on the desktop: depth-stencil formats are copied as a 32-bit depth
    /// plane and an 8-bit stencil plane, `NV12` as an 8-bit luma plane and a 16-bit chroma plane of
    /// half the width and height, `420_OPAQUE` as its luma plane alone; `YUY2` in blocks of two
    /// pixels.
    pub fn subresource_shape(&self, index: u32) -> Result<SubresourceShape, Error> {
        let (format, _) = self.texture_format()?;
        let (mips, layers) = (self.mips(), self.layers());
        let planar = self.planes()? == 2;
        let plane = index / (mips * layers);
        let chroma = has_chroma_plane(self.format);
        let (block, plane_format, pitch_alignment) = match (planar, plane) {
            _ if chroma && plane == 0 => {
                (Format::R8Unorm.block(), R8_TYPELESS, 2 * PITCH_ALIGNMENT)
            }
            _ if chroma => (Format::Rg8Unorm.block(), R8G8_TYPELESS, 2 * PITCH_ALIGNMENT),
            (false, _) if self.format == YUY2 => {
                let (_, bh, bytes) = format.block();
                ((2, bh, bytes), self.format, PITCH_ALIGNMENT)
            }
            (false, _) => (format.block(), self.format, PITCH_ALIGNMENT),
            (true, 0) => (Format::R32Float.block(), R32_TYPELESS, 2 * PITCH_ALIGNMENT),
            (true, _) => (Format::R8Uint.block(), R8_TYPELESS, 2 * PITCH_ALIGNMENT),
        };
        let subsampled = u32::from(chroma && plane == 1);
        let mip = index % mips;
        let size = |n: u64| ((n >> subsampled) >> mip).max(1) as u32;
        Ok(SubresourceShape {
            block,
            format: plane_format,
            pitch_alignment,
            width: size(self.width),
            height: size(u64::from(self.height)),
            depth: if self.dimension == DIMENSION_TEXTURE3D {
                (u32::from(self.depth_or_array_size) >> mip).max(1)
            } else {
                1
            },
        })
    }

    pub fn subresources(&self) -> Result<u32, Error> {
        Ok(self.mips() * self.layers() * self.planes()?)
    }

    /// Array layers (one for 3D textures and buffers).
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
        // The video formats' stored textures (plan 4.12); their views name a plane's format.
        match self.format {
            NV12 | OPAQUE_420 => return Ok((Format::R8Unorm, true)),
            YUY2 => return Ok((Format::Rgba8Unorm, true)),
            // 2D textures held in its bits, copied and mapped with the R10G10B10A2 formats;
            // Microsoft's 11_0 table gives it no view, target or storage use.
            R10G10B10_XR_BIAS_A2_UNORM => {
                if self.dimension != DIMENSION_TEXTURE2D
                    || self.flags
                        & (FLAG_RENDER_TARGET | FLAG_DEPTH_STENCIL | FLAG_UNORDERED_ACCESS)
                        != 0
                {
                    return Err(Error::Invalid(
                        "an R10G10B10_XR_BIAS_A2_UNORM texture other than a 2D one without views",
                    ));
                }
                return Ok((Format::Rgb10a2Unorm, false));
            }
            _ => {}
        }
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
        /// `D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET` and `..._ALLOW_DEPTH_STENCIL`.
        const TARGETS: u32 = 0x3;
        let samples = u64::from(self.sample_count.max(1));
        let natural = if samples > 1 {
            DEFAULT_MSAA_RESOURCE_PLACEMENT_ALIGNMENT
        } else {
            DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT
        };
        let bytes = if self.dimension == DIMENSION_BUFFER {
            if ![0, DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT].contains(&self.alignment) {
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
                    SMALL_RESOURCE_PLACEMENT_ALIGNMENT => {
                        samples == 1
                            && self.flags & TARGETS == 0
                            && top <= DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT
                    }
                    DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT => {
                        samples > 1 && top * samples <= DEFAULT_MSAA_RESOURCE_PLACEMENT_ALIGNMENT
                    }
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
        let alignments = [
            0,
            SMALL_RESOURCE_PLACEMENT_ALIGNMENT,
            DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT,
            DEFAULT_MSAA_RESOURCE_PLACEMENT_ALIGNMENT,
        ];
        if !alignments.contains(&self.alignment) {
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
        if self.dimension == DIMENSION_TEXTURE1D && self.height != 1 {
            return Err(Error::Invalid("a one-dimensional texture is one row"));
        }
        // Block-compressed textures are whole blocks at level 0 (unaligned blocks are not reported).
        let (bw, bh, _) = self.subresource_shape(0)?.block;
        if !self.width.is_multiple_of(u64::from(bw)) || !self.height.is_multiple_of(bh) {
            return Err(Error::Invalid(
                "a block-compressed texture of part of a block",
            ));
        }
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
            let SubresourceShape {
                block: (bw, bh, bytes),
                format: plane_format,
                pitch_alignment,
                width,
                height,
                depth,
            } = self.subresource_shape(index)?;
            let rows = height.div_ceil(bh);
            let row_size = u64::from(width.div_ceil(bw)) * u64::from(bytes);
            let row_pitch = row_size.next_multiple_of(pitch_alignment);
            offset = offset.next_multiple_of(TEXTURE_DATA_PLACEMENT_ALIGNMENT);
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
