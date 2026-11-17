//! The adapter description DXGI's `GetDesc`, `GetDesc1`, `GetDesc2` and `GetDesc3` answer with.

use wcontainers_gpu::Caps;

/// The locally unique identifier of the one adapter, the same for every factory and device of the
/// process (`DXGI_ADAPTER_DESC::AdapterLuid`, `ID3D12Device::GetAdapterLuid`).
pub const ADAPTER_LUID: u64 = 0x5743_0001;

/// What `DXGI_ADAPTER_DESC3` holds, in plain data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterDesc {
    /// The adapter's name, at most 127 UTF-16 units (DXGI's field holds 128 with the terminator).
    pub description: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub sub_sys_id: u32,
    pub revision: u32,
    /// Bytes of memory the adapter has for itself. No browser API reports memory sizes, so this is
    /// zero, as DXGI reports for adapters without memory of their own.
    pub dedicated_video_memory: u64,
    pub dedicated_system_memory: u64,
    pub shared_system_memory: u64,
    pub luid: u64,
    /// `DXGI_ADAPTER_FLAG_SOFTWARE`.
    pub software: bool,
}

impl AdapterDesc {
    pub fn from_caps(caps: &Caps) -> Self {
        let mut description = String::new();
        let mut units = 0;
        for c in caps.adapter.name.chars() {
            units += c.len_utf16();
            if units > 127 {
                break;
            }
            description.push(c);
        }
        AdapterDesc {
            description,
            vendor_id: caps.adapter.vendor,
            device_id: caps.adapter.device,
            sub_sys_id: 0,
            revision: 0,
            dedicated_video_memory: 0,
            dedicated_system_memory: 0,
            shared_system_memory: 0,
            luid: ADAPTER_LUID,
            software: caps.adapter.software,
        }
    }

    /// The description as DXGI's fixed array of UTF-16 units, terminated.
    pub fn description_utf16(&self) -> [u16; 128] {
        let mut out = [0u16; 128];
        for (o, u) in out.iter_mut().zip(self.description.encode_utf16()) {
            *o = u;
        }
        out
    }
}
