//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! Exactly sized bytes across both glues, meant to run under AddressSanitizer (C in exact.c, compiled by build.rs
//! under the `c-caller` feature). Every length comes from the reference (lengths_reference.rs), which shares no code
//! with the glue's rules:
//!
//! - the serving glue, called by C with initial data and uploads in heap blocks of exactly the reference's length, and
//!   lending Map's memory that C touches to exactly the reference's length: a rule that reads more than an upload, or
//!   lends less than C may touch, is an AddressSanitizer report;
//! - the calling glue, calling a stand-in system in C that reads exactly the reference's length of every initial
//!   subresource and upload and lends from Map a heap block of exactly the reference's length: a rule that lets fewer
//!   bytes through than the system reads, or views more than it lends, is an AddressSanitizer report; a rule that
//!   refuses the exact bytes, or lets one byte fewer through, fails an assertion.
//!
//! Run with one test thread: the stand-in's expectations are the process's.

use std::cell::Cell;

#[path = "../../lengths_reference.rs"]
mod reference;

use reference::{
    Resource, boxes, reference_initial, reference_lent, reference_mapped, reference_upload,
    row_and_rows,
};
use wcontainers_component_object_model::structures::{Guid, Hmodule};
use wcontainers_component_object_model_direct::{IID_IUNKNOWN, Raw};
use wcontainers_direct3d_11 as d3d11;
use wcontainers_direct3d_11::constants as ids;
use wcontainers_direct3d_11::enumerations::{CreateDeviceFlag, Map};
use wcontainers_direct3d_11::structures::SubresourceData;
use wcontainers_direct3d_11::{Device as _, DeviceContext as _, Direct3D11 as _, ResourceRef};
use wcontainers_direct3d_11_direct::{
    Library, SystemDevice, SystemDeviceContext, SystemDirect3D11, exports,
};
use wcontainers_direct3d_11_recorder::Recorder;
use wcontainers_direct3d_common::enumerations::{DriverType, FeatureLevel};
use wcontainers_volatile_bytes::VolatileBytes;

#[expect(
    unsafe_code,
    reason = "exact.c, which build.rs compiles under the c-caller feature"
)]
unsafe extern "C" {
    fn exact_create_device(create: usize, device: *mut usize, context: *mut usize) -> i32;
    fn exact_create(
        device: usize,
        kind: u32,
        desc: *const u8,
        count: u32,
        lengths: *const u64,
        pitches: *const u32,
        slice_pitches: *const u32,
        out: *mut usize,
    ) -> i32;
    fn exact_update(
        context: usize,
        resource: usize,
        subresource: u32,
        region: *const u32,
        length: u64,
        row_pitch: u32,
        depth_pitch: u32,
    );
    fn exact_map(
        context: usize,
        resource: usize,
        subresource: u32,
        map_type: i32,
        touch: extern "C" fn(u32, u32) -> u64,
        touched: *mut u64,
    ) -> i32;
    fn exact_release(object: usize) -> u32;
    fn exact_set_exception_mode(device: usize, flags: u32) -> i32;
    fn exact_open_shared(device: usize, handle: usize, iid: *const u8, out: *mut usize) -> i32;
    fn exact_set_break(queue: usize, slot: u32, value: i32, enable: i32) -> i32;
    fn exact_create_device_with_module(
        create: usize,
        module: usize,
        device: *mut usize,
        context: *mut usize,
    ) -> i32;
    fn exact_query(object: usize, iid: *const u8, out: *mut usize) -> i32;
    fn exact_offer(device: usize, resource: usize, priority: i32) -> i32;
    fn exact_reclaim(device: usize, resource: usize, discarded: *mut i32) -> i32;
    fn exact_offer1(device: usize, resource: usize, priority: i32, flags: u32) -> i32;
    fn exact_reclaim1(device: usize, resource: usize, results: *mut i32) -> i32;
    fn exact_software_adapter(factory: usize, module: usize, adapter: *mut usize) -> i32;
    fn exact_system_answer(kind: u32, iids: *const u8, count: usize);
    fn exact_system_expect(lengths: *const u64, count: usize);
    fn exact_system_map_with(row_pitch: u32, depth_pitch: u32, length: u64);
    fn exact_system_seen(out: *mut u64);
}

#[expect(
    unsafe_code,
    reason = "exact.c's stand-in d3d11.dll exports, which take what d3d11.h declares"
)]
unsafe extern "system" {
    fn exact_system_create_device(
        adapter: Raw,
        driver_type: i32,
        software: usize,
        flags: u32,
        levels: *const u8,
        level_count: u32,
        sdk_version: u32,
        device: *mut Raw,
        level: *mut u8,
        context: *mut Raw,
    ) -> i32;
    fn exact_system_create_device_and_swap_chain(
        adapter: Raw,
        driver_type: i32,
        software: usize,
        flags: u32,
        levels: *const u8,
        level_count: u32,
        sdk_version: u32,
        desc: *const u8,
        swap_chain: *mut Raw,
        device: *mut Raw,
        level: *mut u8,
        context: *mut Raw,
    ) -> i32;
}

/// D3D11_MAP_READ_WRITE.
const READ_WRITE: i32 = 3;
/// D3D11_CPU_ACCESS_READ | D3D11_CPU_ACCESS_WRITE.
const CPU_READ_WRITE: u32 = 0x30000;
/// D3D11_USAGE_STAGING.
const STAGING: u32 = 3;
/// The level asked for.
const LEVELS: [FeatureLevel; 1] = [FeatureLevel(0xb000)];
/// d3d11.h's D3D11_SDK_VERSION.
const SDK_VERSION: u32 = 7;

/// The resources of every test: each class of format (plain, 96-bit texels, packed pairs, block-compressed of 8 and of
/// 16 bytes a block, depth, refused planar) in each dimension it is allowed in, with arrays and mip chains.
fn resources() -> Vec<Resource> {
    vec![
        Resource::Buffer(256),
        Resource::Buffer(1000),
        Resource::Texture1D {
            width: 300,
            mips: 0,
            array: 2,
            format: 28,
        },
        Resource::Texture1D {
            width: 17,
            mips: 3,
            array: 1,
            format: 2,
        },
        Resource::Texture2D {
            width: 37,
            height: 23,
            mips: 0,
            array: 2,
            format: 28,
        },
        Resource::Texture2D {
            width: 64,
            height: 64,
            mips: 0,
            array: 1,
            format: 71,
        },
        Resource::Texture2D {
            width: 13,
            height: 9,
            mips: 1,
            array: 1,
            format: 98,
        },
        Resource::Texture2D {
            width: 10,
            height: 4,
            mips: 1,
            array: 1,
            format: 45,
        },
        Resource::Texture2D {
            width: 9,
            height: 5,
            mips: 2,
            array: 2,
            format: 20,
        },
        Resource::Texture2D {
            width: 6,
            height: 3,
            mips: 1,
            array: 1,
            format: 68,
        },
        Resource::Texture2D {
            width: 5,
            height: 5,
            mips: 0,
            array: 1,
            format: 55,
        },
        Resource::Texture3D {
            width: 19,
            height: 13,
            depth: 7,
            mips: 0,
            format: 28,
        },
        Resource::Texture3D {
            width: 8,
            height: 8,
            depth: 4,
            mips: 0,
            format: 71,
        },
        Resource::Texture3D {
            width: 2,
            height: 3,
            depth: 4,
            mips: 1,
            format: 2,
        },
        Resource::Texture2D {
            width: 64,
            height: 64,
            mips: 1,
            array: 1,
            format: 103,
        },
        // R32G32B32_FLOAT, _UINT, _SINT and _TYPELESS: 12-byte texels.
        Resource::Texture1D {
            width: 33,
            mips: 0,
            array: 2,
            format: 6,
        },
        Resource::Texture2D {
            width: 7,
            height: 5,
            mips: 0,
            array: 1,
            format: 7,
        },
        Resource::Texture3D {
            width: 5,
            height: 3,
            depth: 2,
            mips: 0,
            format: 8,
        },
        Resource::Texture2D {
            width: 3,
            height: 3,
            mips: 1,
            array: 1,
            format: 5,
        },
        // BC2_UNORM and BC3_UNORM: 16 bytes a 4 x 4 block, off the block grid and with a full chain.
        Resource::Texture2D {
            width: 37,
            height: 23,
            mips: 0,
            array: 2,
            format: 74,
        },
        Resource::Texture3D {
            width: 13,
            height: 9,
            depth: 3,
            mips: 0,
            format: 77,
        },
        Resource::Texture2D {
            width: 64,
            height: 64,
            mips: 0,
            array: 1,
            format: 77,
        },
    ]
}

fn words(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// The resource's kind for exact.c (0 buffer, 1 to 3 the texture's dimensions) and its description's bytes as d3d11.h
/// lays them out, staging, mappable for reading and writing.
fn described(resource: Resource) -> (u32, Vec<u8>) {
    match resource {
        Resource::Buffer(width) => (0, words(&[width, STAGING, 0, CPU_READ_WRITE, 0, 0])),
        Resource::Texture1D {
            width,
            mips,
            array,
            format,
        } => (
            1,
            words(&[
                width,
                mips,
                array,
                format as u32,
                STAGING,
                0,
                CPU_READ_WRITE,
                0,
            ]),
        ),
        Resource::Texture2D {
            width,
            height,
            mips,
            array,
            format,
        } => (
            2,
            words(&[
                width,
                height,
                mips,
                array,
                format as u32,
                1,
                0,
                STAGING,
                0,
                CPU_READ_WRITE,
                0,
            ]),
        ),
        Resource::Texture3D {
            width,
            height,
            depth,
            mips,
            format,
        } => (
            3,
            words(&[
                width,
                height,
                depth,
                mips,
                format as u32,
                STAGING,
                0,
                CPU_READ_WRITE,
                0,
            ]),
        ),
    }
}

/// A subresource's packed row and rows of elements, if its format is covered.
fn packed(resource: Resource, index: u32) -> Option<(u64, u64)> {
    let (sides, element) = resource.subresource(index)?;
    row_and_rows(element, (0, sides.0), (0, sides.1))
}

/// The pitches each subresource's initial data is laid out at: the packed row padded by 13 bytes, the slices by 7.
fn initial_pitches(resource: Resource, index: u32) -> (u32, u32) {
    let (row, rows) = packed(resource, index).unwrap_or((1, 1));
    let row_pitch = u32::try_from(row).unwrap_or(0) + 13;
    (row_pitch, row_pitch * u32::try_from(rows).unwrap_or(0) + 7)
}

/// Each subresource's initial data: its pitches and the reference's length; none when the reference refuses one.
fn initial_layout(resource: Resource) -> Option<Vec<(u32, u32, u64)>> {
    (0..resource.subresources())
        .map(|index| {
            let (row_pitch, slice_pitch) = initial_pitches(resource, index);
            let length = reference_initial(resource, index, row_pitch, slice_pitch)?;
            Some((row_pitch, slice_pitch, u64::try_from(length).ok()?))
        })
        .collect()
}

fn box_words(region: &d3d11::structures::Box) -> [u32; 6] {
    [
        region.left,
        region.top,
        region.front,
        region.right,
        region.bottom,
        region.back,
    ]
}

/// How many uploads reached the recorder behind the serving glue so far.
fn updates_reaching_the_implementation() -> usize {
    wcontainers_direct3d_11_recorder::calls("ID3D11DeviceContext::UpdateSubresource").len()
}

thread_local! {
    /// The resource and subresource whose mapped bytes C touches now.
    static TOUCHING: Cell<(Resource, u32)> = const { Cell::new((Resource::Buffer(0), 0)) };
}

/// How many bytes C touches of the memory the serving glue lent: what the reference says the glue lends.
extern "C" fn lent_by_reference(row_pitch: u32, depth_pitch: u32) -> u64 {
    let (resource, index) = TOUCHING.with(Cell::get);
    reference_lent(resource, index, row_pitch, depth_pitch)
        .map_or(0, |length| u64::try_from(length).unwrap_or(0))
}

/// C calls the serving glue (over the recorder) with exactly sized initial data and uploads, and touches exactly what
/// the reference says it lends.
#[test]
fn the_serving_glue_reads_and_lends_exactly_the_documented_bytes() {
    let library = exports(Recorder);
    let (mut device, mut context) = (0_usize, 0_usize);
    #[expect(
        unsafe_code,
        reason = "exact.c's caller, given the serving glue's own D3D11CreateDevice"
    )]
    // SAFETY: `create` is the serving glue's export, which takes what d3d11.h declares; the two outs are ours.
    let status = unsafe {
        exact_create_device(
            library.d_3d11_create_device as usize,
            &mut device,
            &mut context,
        )
    };
    assert!(
        status >= 0 && device != 0 && context != 0,
        "a device through the serving glue: {status:#x}"
    );
    let (mut created, mut uploads, mut maps, mut touched_bytes, mut refused_boxes) =
        (0, 0, 0, 0_u64, 0);
    for resource in resources() {
        let (kind, desc) = described(resource);
        let Some(layout) = initial_layout(resource) else {
            // A format the glue refuses: no initial bytes, and the serving glue refuses the description.
            let mut made = 0_usize;
            #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
            // SAFETY: `device` is the serving glue's device; `desc` holds the description's bytes; no initial data.
            let status = unsafe {
                exact_create(
                    device,
                    kind,
                    desc.as_ptr(),
                    0,
                    core::ptr::null(),
                    core::ptr::null(),
                    core::ptr::null(),
                    &mut made,
                )
            };
            assert!(
                status < 0,
                "{resource:?}: a format the reference refuses is refused"
            );
            continue;
        };
        let lengths: Vec<u64> = layout.iter().map(|(_, _, length)| *length).collect();
        let pitches: Vec<u32> = layout.iter().map(|(pitch, _, _)| *pitch).collect();
        let slice_pitches: Vec<u32> = layout.iter().map(|(_, slice, _)| *slice).collect();
        let mut made = 0_usize;
        #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
        // SAFETY: `device` is the serving glue's device; `desc` holds the description's bytes; the three arrays hold one
        // entry per subresource, `count` of them.
        let status = unsafe {
            exact_create(
                device,
                kind,
                desc.as_ptr(),
                u32::try_from(lengths.len()).unwrap_or(0),
                lengths.as_ptr(),
                pitches.as_ptr(),
                slice_pitches.as_ptr(),
                &mut made,
            )
        };
        assert!(
            status >= 0 && made != 0,
            "{resource:?}: created from exactly the reference's initial bytes: {status:#x}"
        );
        created += 1;
        for index in 0..resource.subresources() {
            let sides = resource
                .subresource(index)
                .map_or((1, 1, 1), |(sides, _)| sides);
            let (row, rows) = packed(resource, index).unwrap_or((1, 1));
            let row_pitch = u32::try_from(row).unwrap_or(0) + 5;
            let depth_pitch = row_pitch * u32::try_from(rows).unwrap_or(0) + 3;
            for region in boxes(sides) {
                // A box the reference refuses (past an edge) is handed one byte: the glue must refuse it, never read on.
                let reference =
                    reference_upload(resource, index, region.as_ref(), row_pitch, depth_pitch);
                let length = reference.unwrap_or(1);
                let region_words = region.as_ref().map(box_words);
                let region_pointer = region_words
                    .as_ref()
                    .map_or(core::ptr::null(), |words| words.as_ptr());
                let before = updates_reaching_the_implementation();
                #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
                // SAFETY: `context` and `made` are the serving glue's; `region_pointer` is six words or null.
                unsafe {
                    exact_update(
                        context,
                        made,
                        index,
                        region_pointer,
                        u64::try_from(length).unwrap_or(0),
                        row_pitch,
                        depth_pitch,
                    )
                };
                if reference.is_some() {
                    uploads += 1;
                } else {
                    assert_eq!(
                        updates_reaching_the_implementation(),
                        before,
                        "{resource:?} {index} {region:?}: a box the reference refuses never reaches the implementation"
                    );
                    refused_boxes += 1;
                }
            }
            TOUCHING.with(|touching| touching.set((resource, index)));
            let mut touched = 0_u64;
            #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
            // SAFETY: `context` and `made` are the serving glue's; `lent_by_reference` takes two words and returns one.
            let status = unsafe {
                exact_map(
                    context,
                    made,
                    index,
                    READ_WRITE,
                    lent_by_reference,
                    &mut touched,
                )
            };
            if status >= 0 {
                maps += 1;
                touched_bytes += touched;
            }
        }
        #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
        // SAFETY: `made` is a reference the serving glue handed out, released once.
        let _ = unsafe { exact_release(made) };
    }
    println!(
        "EXACT SERVING: {created} resources created, {uploads} uploads, {refused_boxes} boxes past an edge refused, {maps} maps touching {touched_bytes} bytes"
    );
    assert!(created >= 21 && uploads > 0 && maps > 0 && refused_boxes > 0);
    #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
    // SAFETY: references the serving glue handed out, each released once.
    let _ = unsafe { (exact_release(context), exact_release(device)) };
}

/// The interface ids as 16 bytes each, as GUID lays them out.
fn id_bytes(ids: &[Guid]) -> Vec<u8> {
    ids.iter()
        .flat_map(|id| {
            id.data1
                .to_le_bytes()
                .into_iter()
                .chain(id.data2.to_le_bytes())
                .chain(id.data3.to_le_bytes())
                .chain(id.data4)
        })
        .collect()
}

/// The stand-in's answers, kind by kind as exact.c numbers them, and the calling glue over it.
fn stand_in() -> (
    SystemDevice<SystemDirect3D11>,
    SystemDeviceContext<SystemDirect3D11>,
) {
    let child = [
        IID_IUNKNOWN,
        ids::IID_ID3D11_DEVICE_CHILD,
        ids::IID_ID3D11_RESOURCE,
    ];
    let answers: [Vec<Guid>; 7] = [
        vec![
            IID_IUNKNOWN,
            ids::IID_ID3D11_DEVICE,
            ids::IID_ID3D11_DEVICE1,
            ids::IID_ID3D11_DEVICE2,
            ids::IID_ID3D11_DEVICE3,
            ids::IID_ID3D11_DEVICE4,
            ids::IID_ID3D11_DEVICE5,
        ],
        vec![
            IID_IUNKNOWN,
            ids::IID_ID3D11_DEVICE_CHILD,
            ids::IID_ID3D11_DEVICE_CONTEXT,
            ids::IID_ID3D11_DEVICE_CONTEXT1,
            ids::IID_ID3D11_DEVICE_CONTEXT2,
            ids::IID_ID3D11_DEVICE_CONTEXT3,
            ids::IID_ID3D11_DEVICE_CONTEXT4,
        ],
        vec![ids::IID_ID3D11_MULTITHREAD],
        [child.as_slice(), &[ids::IID_ID3D11_BUFFER]].concat(),
        [child.as_slice(), &[ids::IID_ID3D11_TEXTURE_1D]].concat(),
        [
            child.as_slice(),
            &[ids::IID_ID3D11_TEXTURE_2D, ids::IID_ID3D11_TEXTURE_2D1],
        ]
        .concat(),
        [
            child.as_slice(),
            &[ids::IID_ID3D11_TEXTURE_3D, ids::IID_ID3D11_TEXTURE_3D1],
        ]
        .concat(),
    ];
    for (kind, answered) in answers.iter().enumerate() {
        let bytes = id_bytes(answered);
        #[expect(
            unsafe_code,
            reason = "exact.c's stand-in, told what each kind answers"
        )]
        // SAFETY: `bytes` holds `answered.len()` ids of 16 bytes.
        unsafe {
            exact_system_answer(
                u32::try_from(kind).unwrap_or(u32::MAX),
                bytes.as_ptr(),
                answered.len(),
            )
        };
    }
    let library = Library {
        d_3d11_create_device: exact_system_create_device,
        d_3d11_create_device_and_swap_chain: exact_system_create_device_and_swap_chain,
    };
    #[expect(
        unsafe_code,
        reason = "the calling glue over exact.c's stand-in d3d11.dll"
    )]
    // SAFETY: each entry is the stand-in's export of that name, taking what d3d11.h declares.
    let system = unsafe { SystemDirect3D11::new(library) };
    let (device, _, context) = system
        .create_device(
            None,
            DriverType(1),
            Hmodule(0),
            CreateDeviceFlag(0),
            &LEVELS,
            SDK_VERSION,
        )
        .expect("the stand-in's device");
    (device, context)
}

fn expect_lengths(lengths: &[u64]) {
    #[expect(unsafe_code, reason = "exact.c's stand-in, told what to read next")]
    // SAFETY: `lengths` holds its length of words.
    unsafe {
        exact_system_expect(lengths.as_ptr(), lengths.len())
    };
}

/// What the stand-in saw: creations, uploads, maps, unmaps, calls of a slot it does not serve, unmaps of another
/// resource or subresource than the one mapped, devices made.
fn seen() -> [u64; 7] {
    let mut seen = [0_u64; 7];
    #[expect(unsafe_code, reason = "exact.c's stand-in's counts")]
    // SAFETY: seven words.
    unsafe {
        exact_system_seen(seen.as_mut_ptr())
    };
    seen
}

/// A resource made through the calling glue, of any dimension.
enum Made {
    Buffer(<SystemDirect3D11 as d3d11::Direct3D11>::Buffer),
    Texture1D(<SystemDirect3D11 as d3d11::Direct3D11>::Texture1D),
    Texture2D(<SystemDirect3D11 as d3d11::Direct3D11>::Texture2D),
    Texture3D(<SystemDirect3D11 as d3d11::Direct3D11>::Texture3D),
}

impl Made {
    fn as_ref(&self) -> ResourceRef<'_, SystemDirect3D11> {
        match self {
            Made::Buffer(made) => ResourceRef::Buffer(made),
            Made::Texture1D(made) => ResourceRef::Texture1D(made),
            Made::Texture2D(made) => ResourceRef::Texture2D(made),
            Made::Texture3D(made) => ResourceRef::Texture3D(made),
        }
    }
}

fn make(
    device: &SystemDevice<SystemDirect3D11>,
    resource: Resource,
    initial: &[SubresourceData],
) -> Option<Made> {
    match resource {
        Resource::Buffer(width) => device
            .create_buffer(&reference::buffer_desc(width), initial.first())
            .ok()
            .map(Made::Buffer),
        Resource::Texture1D {
            width,
            mips,
            array,
            format,
        } => device
            .create_texture_1d(
                &reference::texture_1d_desc(width, mips, array, format),
                initial,
            )
            .ok()
            .map(Made::Texture1D),
        Resource::Texture2D {
            width,
            height,
            mips,
            array,
            format,
        } => device
            .create_texture_2d(
                &reference::texture_2d_desc(width, height, mips, array, format),
                initial,
            )
            .ok()
            .map(Made::Texture2D),
        Resource::Texture3D {
            width,
            height,
            depth,
            mips,
            format,
        } => device
            .create_texture_3d(
                &reference::texture_3d_desc(width, height, depth, mips, format),
                initial,
            )
            .ok()
            .map(Made::Texture3D),
    }
}

/// The calling glue over a stand-in system that reads exactly the reference's lengths and lends exactly the reference's
/// view: exact bytes pass, one byte fewer is refused, and the view is exactly the reference's.
#[test]
fn the_calling_glue_passes_and_views_exactly_the_documented_bytes() {
    let (device, context) = stand_in();
    let (mut short_refused, mut uploads, mut maps, mut viewed_bytes, mut refused_boxes) =
        (0, 0, 0, 0_u64, 0);
    for resource in resources() {
        let Some(layout) = initial_layout(resource) else {
            let before = seen()[0];
            assert!(
                make(&device, resource, &[]).is_none(),
                "{resource:?}: a format the reference refuses is refused"
            );
            assert_eq!(
                seen()[0],
                before,
                "{resource:?}: the refused description never reached the system"
            );
            continue;
        };
        let initial = |shorten: Option<usize>| -> Vec<SubresourceData> {
            layout
                .iter()
                .enumerate()
                .map(|(at, (pitch, slice, length))| {
                    let length =
                        usize::try_from(*length).unwrap_or(0) - usize::from(shorten == Some(at));
                    SubresourceData {
                        sys_mem: vec![0x3c; length],
                        sys_mem_pitch: *pitch,
                        sys_mem_slice_pitch: *slice,
                    }
                })
                .collect()
        };
        let lengths: Vec<u64> = layout.iter().map(|(_, _, length)| *length).collect();
        expect_lengths(&lengths);
        // One byte fewer in any one subresource is refused before the system reads it.
        for shorten in [0, layout.len() - 1] {
            let before = seen()[0];
            assert!(
                make(&device, resource, &initial(Some(shorten))).is_none(),
                "{resource:?}: subresource {shorten} one byte short is refused"
            );
            assert_eq!(seen()[0], before);
            short_refused += 1;
        }
        let made = make(&device, resource, &initial(None))
            .unwrap_or_else(|| panic!("{resource:?}: exactly the reference's initial bytes pass"));
        for index in 0..resource.subresources() {
            let sides = resource
                .subresource(index)
                .map_or((1, 1, 1), |(sides, _)| sides);
            let (row, rows) = packed(resource, index).unwrap_or((1, 1));
            let row_pitch = u32::try_from(row).unwrap_or(0) + 9;
            let depth_pitch = row_pitch * u32::try_from(rows).unwrap_or(0) + 11;
            for region in boxes(sides) {
                let Some(length) =
                    reference_upload(resource, index, region.as_ref(), row_pitch, depth_pitch)
                else {
                    // A box the reference refuses (past an edge) is refused before the system, however many bytes come.
                    let whole = reference_upload(resource, index, None, row_pitch, depth_pitch)
                        .unwrap_or(0);
                    let before = seen()[1];
                    context.update_subresource(
                        made.as_ref(),
                        index,
                        region.as_ref(),
                        &vec![3; whole + (1 << 16)],
                        row_pitch,
                        depth_pitch,
                    );
                    assert_eq!(
                        seen()[1],
                        before,
                        "{resource:?} {index} {region:?}: a box the reference refuses is refused"
                    );
                    refused_boxes += 1;
                    continue;
                };
                expect_lengths(&[u64::try_from(length).unwrap_or(0)]);
                if length > 0 {
                    let before = seen()[1];
                    context.update_subresource(
                        made.as_ref(),
                        index,
                        region.as_ref(),
                        &vec![1; length - 1],
                        row_pitch,
                        depth_pitch,
                    );
                    assert_eq!(
                        seen()[1],
                        before,
                        "{resource:?} {index} {region:?}: an upload one byte short is refused"
                    );
                    short_refused += 1;
                }
                let before = seen()[1];
                context.update_subresource(
                    made.as_ref(),
                    index,
                    region.as_ref(),
                    &vec![2; length],
                    row_pitch,
                    depth_pitch,
                );
                assert_eq!(
                    seen()[1],
                    before + 1,
                    "{resource:?} {index} {region:?}: exactly the reference's upload passes"
                );
                uploads += 1;
            }
            // Map at packed pitches, padded ones, a depth pitch far past the rows, and pitches past the largest resource.
            let row = u32::try_from(row).unwrap_or(0);
            let rows = u32::try_from(rows).unwrap_or(0);
            let padded = row.next_multiple_of(256);
            for (row_pitch, depth_pitch) in [
                (row, row * rows),
                (padded, padded * rows + 4096),
                (padded, u32::MAX / 2),
                (1 << 27, 1 << 27),
            ] {
                let view = reference_mapped(resource, index, row_pitch, depth_pitch);
                #[expect(unsafe_code, reason = "exact.c's stand-in, told what Map lends next")]
                // SAFETY: plain numbers.
                unsafe {
                    exact_system_map_with(
                        row_pitch,
                        depth_pitch,
                        view.map_or(0, |view| u64::try_from(view).unwrap_or(0)),
                    )
                };
                let mapped = context.map(made.as_ref(), index, Map(READ_WRITE), 0);
                match (view, mapped) {
                    (Some(view), Ok(mapped)) => {
                        // Every byte the glue views is read and written first, so that a view past the stand-in's block is
                        // an AddressSanitizer report; then its length is the reference's.
                        let viewed = usize::try_from(mapped.data.len()).unwrap_or(0);
                        let mut bytes = vec![0_u8; viewed];
                        assert_eq!(mapped.data.read(&mut bytes), viewed);
                        assert_eq!(mapped.data.write(&bytes), viewed);
                        assert_eq!(
                            viewed, view,
                            "{resource:?} {index} pitches {row_pitch} {depth_pitch}: the view is the reference's"
                        );
                        maps += 1;
                        viewed_bytes += u64::try_from(view).unwrap_or(0);
                        drop(mapped);
                        context.unmap(made.as_ref(), index);
                    }
                    (None, Err(_)) => {}
                    (view, mapped) => panic!(
                        "{resource:?} {index} pitches {row_pitch} {depth_pitch}: reference {view:?}, glue {:?}",
                        mapped.map(|mapped| mapped.data.len())
                    ),
                }
            }
        }
    }
    let seen = seen();
    println!(
        "EXACT CALLING: {short_refused} short bytes refused, {uploads} exact uploads, {refused_boxes} boxes past an edge refused, {maps} maps viewing {viewed_bytes} bytes; the stand-in saw {seen:?}"
    );
    assert_eq!(seen[2], seen[3], "every map unmapped");
    assert_eq!(
        seen[5], 0,
        "every Unmap, the glue's own after a mapping it refuses too, names the resource and subresource mapped"
    );
    assert_eq!(
        seen[4], 0,
        "the calling glue called no slot the stand-in does not serve"
    );
    assert!(uploads > 0 && maps > 0 && refused_boxes > 0);
}

fn answer(kind: u32, answered: &[Guid]) {
    let bytes = id_bytes(answered);
    #[expect(unsafe_code, reason = "exact.c's stand-in, told what a kind answers")]
    // SAFETY: `bytes` holds `answered.len()` ids of 16 bytes.
    unsafe {
        exact_system_answer(kind, bytes.as_ptr(), answered.len())
    };
}

/// exact.c's kinds of query object.
const KIND_QUERY: u32 = 7;
const KIND_PREDICATE: u32 = 8;

/// CreateQuery's object crosses as the kind it answers: a predicate (which, like the runtime's, also answers the newest
/// query interface) as a predicate, any other query as a query. The calling glue asks for the predicate first; asking
/// for the query kind first would hand a predicate out as a query. A query answering only ID3D11Query (the 11.0
/// runtime's) is refused: the query kind is held by its newest interface, ID3D11Query1, whose table it does not have.
#[test]
fn create_query_hands_out_the_kind_the_object_answers() {
    let (device, _context) = stand_in();
    let query = [
        IID_IUNKNOWN,
        ids::IID_ID3D11_DEVICE_CHILD,
        ids::IID_ID3D11_ASYNCHRONOUS,
        ids::IID_ID3D11_QUERY,
        ids::IID_ID3D11_QUERY1,
    ];
    answer(KIND_QUERY, &query);
    answer(
        KIND_PREDICATE,
        &[query.as_slice(), &[ids::IID_ID3D11_PREDICATE]].concat(),
    );
    let made = |kind: i32| {
        device.create_query(&d3d11::structures::QueryDesc {
            query: d3d11::enumerations::QueryKind(kind),
            misc_flags: 0,
        })
    };
    // D3D11_QUERY_OCCLUSION_PREDICATE, D3D11_QUERY_SO_OVERFLOW_PREDICATE and its four streams.
    for kind in [5, 7, 12, 13, 14, 15] {
        assert!(
            matches!(made(kind), Ok(d3d11::QueryObject::Predicate(_))),
            "a predicate's object ({kind}) comes back as a predicate"
        );
    }
    for kind in [0, 1, 2, 3, 4, 6, 8, 9, 10, 11] {
        assert!(
            matches!(made(kind), Ok(d3d11::QueryObject::Query(_))),
            "a query's object ({kind}) comes back as a query"
        );
    }
    answer(KIND_QUERY, &query[..4]);
    assert!(
        made(1).is_err(),
        "a query of the 11.0 runtime, answering no ID3D11Query1, is refused"
    );
    println!(
        "CREATE QUERY: 6 predicates as predicates, 10 queries as queries, a query without ID3D11Query1 refused"
    );
}

/// One byte more than the reference says the serving glue lends: C touches it, and AddressSanitizer must stop it.
extern "C" fn one_past_the_lent_memory(row_pitch: u32, depth_pitch: u32) -> u64 {
    lent_by_reference(row_pitch, depth_pitch) + 1
}

/// The shapes `touch_one_past_the_lent_memory` is run for, each as `EXACT_SHAPE` names it, with a resource of each whose
/// lent memory at the recorder's pitches (rows of 4096 bytes, slices of 65536) is within what the serving glue lends.
const SHAPES: [(&str, Resource); 4] = [
    ("buffer", Resource::Buffer(256)),
    (
        "texture_1d",
        Resource::Texture1D {
            width: 300,
            mips: 0,
            array: 2,
            format: 28,
        },
    ),
    (
        "texture_2d",
        Resource::Texture2D {
            width: 37,
            height: 23,
            mips: 0,
            array: 2,
            format: 28,
        },
    ),
    (
        "texture_3d",
        Resource::Texture3D {
            width: 19,
            height: 16,
            depth: 7,
            mips: 0,
            format: 28,
        },
    ),
];

/// Under AddressSanitizer (`EXACT_ASAN` set, as the plant runner's exact-under-asan.sh sets it): the memory the serving glue lends
/// for a mapped subresource ends exactly where the reference says, for a buffer and each texture shape: C touching one
/// byte past it is stopped with a heap-buffer-overflow, each shape in a process of its own (this test binary again).
/// Without AddressSanitizer nothing would stop the touch, so the test checks nothing there.
#[test]
fn one_byte_past_the_lent_memory_is_stopped() {
    if std::env::var_os("EXACT_ASAN").is_none() {
        return;
    }
    let program = std::env::current_exe().expect("this test binary");
    for (shape, _) in SHAPES {
        let run = std::process::Command::new(&program)
            .args([
                "--exact",
                "touch_one_past_the_lent_memory",
                "--ignored",
                "--test-threads=1",
            ])
            .env("EXACT_SHAPE", shape)
            .output()
            .expect("this test binary, run again");
        let errors = String::from_utf8_lossy(&run.stderr);
        assert!(
            !run.status.success() && errors.contains("heap-buffer-overflow"),
            "{shape}: C touched one byte past the memory lent and was not stopped"
        );
    }
}

/// Maps the first subresource of the `EXACT_SHAPE` resource through the serving glue and touches one byte more than
/// the reference says it lends (run by `one_byte_past_the_lent_memory_is_stopped`, where AddressSanitizer stops it).
#[test]
#[ignore = "run by one_byte_past_the_lent_memory_is_stopped in a process of its own"]
fn touch_one_past_the_lent_memory() {
    let wanted = std::env::var("EXACT_SHAPE").unwrap_or_default();
    let (_, resource) = SHAPES
        .into_iter()
        .find(|(shape, _)| *shape == wanted)
        .expect("a shape EXACT_SHAPE names");
    let library = exports(Recorder);
    let (mut device, mut context) = (0_usize, 0_usize);
    #[expect(
        unsafe_code,
        reason = "exact.c's caller, given the serving glue's own D3D11CreateDevice"
    )]
    // SAFETY: `create` is the serving glue's export, which takes what d3d11.h declares; the two outs are ours.
    let status = unsafe {
        exact_create_device(
            library.d_3d11_create_device as usize,
            &mut device,
            &mut context,
        )
    };
    assert!(
        status >= 0,
        "a device through the serving glue: {status:#x}"
    );
    let (kind, desc) = described(resource);
    let layout = initial_layout(resource).expect("a format the reference covers");
    let lengths: Vec<u64> = layout.iter().map(|(_, _, length)| *length).collect();
    let pitches: Vec<u32> = layout.iter().map(|(pitch, _, _)| *pitch).collect();
    let slice_pitches: Vec<u32> = layout.iter().map(|(_, slice, _)| *slice).collect();
    let mut made = 0_usize;
    #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
    // SAFETY: `device` is the serving glue's device; `desc` holds the description's bytes; the three arrays hold one
    // entry per subresource, `count` of them.
    let status = unsafe {
        exact_create(
            device,
            kind,
            desc.as_ptr(),
            u32::try_from(lengths.len()).unwrap_or(0),
            lengths.as_ptr(),
            pitches.as_ptr(),
            slice_pitches.as_ptr(),
            &mut made,
        )
    };
    assert!(
        status >= 0 && made != 0,
        "{resource:?}: created: {status:#x}"
    );
    TOUCHING.with(|touching| touching.set((resource, 0)));
    let mut touched = 0_u64;
    #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
    // SAFETY: `context` and `made` are the serving glue's; `one_past_the_lent_memory` takes two words and returns one.
    let status = unsafe {
        exact_map(
            context,
            made,
            0,
            READ_WRITE,
            one_past_the_lent_memory,
            &mut touched,
        )
    };
    assert!(status >= 0, "{resource:?}: mapped: {status:#x}");
}

/// The last call of `place` the recorder behind the serving glue received.
fn last_call(place: &str) -> String {
    wcontainers_direct3d_11_recorder::calls(place)
        .pop()
        .unwrap_or_default()
}

/// The calling glue's pins hold for the system only: the serving glue passes on what a C caller gives, so that our own
/// implementation sees the program's values. D3D11_RAISE_FLAG_DRIVER_INTERNAL_ERROR (1), a break turned on (TRUE) for
/// a category, a severity and a message, and a software module (0x1000) each reach the recorder as given.
#[test]
fn the_serving_glue_passes_what_the_calling_glue_pins() {
    let library = exports(Recorder);
    let (mut device, mut context) = (0_usize, 0_usize);
    #[expect(
        unsafe_code,
        reason = "exact.c's caller, given the serving glue's own D3D11CreateDevice"
    )]
    // SAFETY: `create` is the serving glue's export, which takes what d3d11.h declares; the two outs are ours.
    let status = unsafe {
        exact_create_device(
            library.d_3d11_create_device as usize,
            &mut device,
            &mut context,
        )
    };
    assert!(
        status >= 0,
        "a device through the serving glue: {status:#x}"
    );
    #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
    // SAFETY: `device` is the serving glue's device.
    let status = unsafe { exact_set_exception_mode(device, 1) };
    assert!(status >= 0, "SetExceptionMode: {status:#x}");
    assert_eq!(
        last_call("ID3D11Device::SetExceptionMode"),
        "ID3D11Device::SetExceptionMode(raise_flags=1)"
    );
    let iid = id_bytes(&[ids::IID_ID3D11_INFO_QUEUE]);
    let mut queue = 0_usize;
    #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
    // SAFETY: `device` is the serving glue's device; the handle is a number it passes on; `iid` holds 16 bytes; the out
    // is ours.
    let status = unsafe { exact_open_shared(device, 68, iid.as_ptr(), &mut queue) };
    assert!(
        status >= 0 && queue != 0,
        "the device's info queue: {status:#x}"
    );
    for (slot, place, value, shown) in [
        (
            30,
            "ID3D11InfoQueue::SetBreakOnCategory",
            3,
            "category=MessageCategory(3), b_enable=true",
        ),
        (
            31,
            "ID3D11InfoQueue::SetBreakOnSeverity",
            1,
            "severity=MessageSeverity(1), b_enable=true",
        ),
        (
            32,
            "ID3D11InfoQueue::SetBreakOnID",
            7,
            "id=MessageId(7), b_enable=true",
        ),
    ] {
        #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
        // SAFETY: `queue` is the serving glue's info queue; the slot is that method's in d3d11sdklayers.h's table.
        let status = unsafe { exact_set_break(queue, slot, value, 1) };
        assert!(status >= 0, "{place}: {status:#x}");
        assert_eq!(last_call(place), format!("{place}({shown})"));
    }
    let (mut soft_device, mut soft_context) = (0_usize, 0_usize);
    #[expect(
        unsafe_code,
        reason = "exact.c's caller, given the serving glue's own D3D11CreateDevice"
    )]
    // SAFETY: as above; the module is a number the serving glue passes on, never loads.
    let status = unsafe {
        exact_create_device_with_module(
            library.d_3d11_create_device as usize,
            0x1000,
            &mut soft_device,
            &mut soft_context,
        )
    };
    assert!(
        status >= 0,
        "a device of a software module through the serving glue: {status:#x}"
    );
    assert!(
        last_call("D3D11CreateDevice").contains("software=Hmodule(4096)"),
        "{}",
        last_call("D3D11CreateDevice")
    );
    #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
    // SAFETY: references the serving glue handed out, each released once.
    let _ = unsafe {
        (
            exact_release(queue),
            exact_release(soft_context),
            exact_release(soft_device),
            exact_release(context),
            exact_release(device),
        )
    };
}

/// The calling glue refuses a list of more feature levels than d3dcommon.h declares (src/api.rs: at most 16) before the
/// system reads it, and passes a list of 16.
#[test]
fn seventeen_feature_levels_never_reach_the_system() {
    let _ = stand_in();
    let library = Library {
        d_3d11_create_device: exact_system_create_device,
        d_3d11_create_device_and_swap_chain: exact_system_create_device_and_swap_chain,
    };
    #[expect(
        unsafe_code,
        reason = "the calling glue over exact.c's stand-in d3d11.dll"
    )]
    // SAFETY: each entry is the stand-in's export of that name, taking what d3d11.h declares.
    let system = unsafe { SystemDirect3D11::new(library) };
    let made = seen()[6];
    let refused = system.create_device(
        None,
        DriverType(1),
        Hmodule(0),
        CreateDeviceFlag(0),
        &[LEVELS[0]; 17],
        SDK_VERSION,
    );
    assert_eq!(
        refused.err(),
        Some(wcontainers_component_object_model::constants::E_INVALIDARG)
    );
    assert_eq!(seen()[6], made, "seventeen levels never reach the system");
    let passed = system.create_device(
        None,
        DriverType(1),
        Hmodule(0),
        CreateDeviceFlag(0),
        &[LEVELS[0]; 16],
        SDK_VERSION,
    );
    assert!(passed.is_ok(), "sixteen levels pass");
    assert_eq!(seen()[6], made + 1, "and reach the system");
}

/// The interface `iid` of `object`, asked through C.
fn queried(object: usize, iid: &Guid) -> usize {
    let bytes = id_bytes(&[*iid]);
    let mut out = 0_usize;
    #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
    // SAFETY: `object` is a reference the serving glue handed out; `bytes` holds 16 bytes; the out is ours.
    let status = unsafe { exact_query(object, bytes.as_ptr(), &mut out) };
    assert!(status >= 0 && out != 0, "QueryInterface: {status:#x}");
    out
}

/// What the recorder received in the calls of `place` after the first `from` arguments it logged.
fn received_since(from: usize, place: &str) -> Vec<String> {
    wcontainers_direct3d_11_recorder::arguments_since(from)
        .into_iter()
        .filter(|call| call.starts_with(place))
        .collect()
}

/// DXGI's calls the calling glue never makes of the system (OfferResources and ReclaimResources, never called; a software
/// adapter's module, pinned), made by C on our implementation through the serving glue: each reaches the recorder with
/// its list of one resource (null: an element left empty, which the header lets a caller pass and the trait passes as
/// none), the priority (DXGI_OFFER_RESOURCE_PRIORITY_NORMAL, 2), the flags (DXGI_OFFER_RESOURCE_FLAG_ALLOW_DECOMMIT, 1)
/// and the module (0x2000) as given.
#[test]
fn the_serving_glue_passes_dxgis_calls_the_calling_glue_never_makes() {
    let library = exports(Recorder);
    let (mut device, mut context) = (0_usize, 0_usize);
    #[expect(
        unsafe_code,
        reason = "exact.c's caller, given the serving glue's own D3D11CreateDevice"
    )]
    // SAFETY: `create` is the serving glue's export, which takes what d3d11.h declares; the two outs are ours.
    let status = unsafe {
        exact_create_device(
            library.d_3d11_create_device as usize,
            &mut device,
            &mut context,
        )
    };
    assert!(
        status >= 0,
        "a device through the serving glue: {status:#x}"
    );
    let device2 = queried(
        device,
        &wcontainers_directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
    );
    let device4 = queried(
        device,
        &wcontainers_directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
    );
    let from = wcontainers_direct3d_11_recorder::arguments_logged();
    let mut answered = [0_i32; 1];
    #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
    // SAFETY: the two devices are references the serving glue handed out; each list holds one null element; `answered`
    // holds one word.
    let statuses = unsafe {
        [
            exact_offer(device2, 0, 2),
            exact_reclaim(device2, 0, answered.as_mut_ptr()),
            exact_offer1(device4, 0, 2, 1),
            exact_reclaim1(device4, 0, answered.as_mut_ptr()),
        ]
    };
    assert!(statuses.iter().all(|status| *status >= 0), "{statuses:x?}");
    assert_eq!(
        received_since(from, "IDXGIDevice2::OfferResources"),
        ["IDXGIDevice2::OfferResources(resources=[false], priority=OfferResourcePriority(2))"]
    );
    assert_eq!(
        received_since(from, "IDXGIDevice2::ReclaimResources"),
        ["IDXGIDevice2::ReclaimResources(resources=[false])"]
    );
    assert_eq!(
        received_since(from, "IDXGIDevice4::OfferResources1"),
        [
            "IDXGIDevice4::OfferResources1(resources=[false], priority=OfferResourcePriority(2), flags=1)"
        ]
    );
    assert_eq!(
        received_since(from, "IDXGIDevice4::ReclaimResources1"),
        ["IDXGIDevice4::ReclaimResources1(resources=[false])"]
    );
    let factories = wcontainers_directx_graphics_infrastructure_direct::exports(Recorder);
    let iid =
        id_bytes(&[wcontainers_directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY]);
    let mut factory: Raw = core::ptr::null_mut();
    #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
    // SAFETY: the serving glue's export, which takes what dxgi.h declares: 16 bytes of an id and an out of ours.
    let status = unsafe { (factories.create_dxgi_factory)(iid.as_ptr(), &mut factory) };
    assert!(status >= 0 && !factory.is_null(), "a factory: {status:#x}");
    let mut adapter = 0_usize;
    #[expect(unsafe_code, reason = "exact.c's caller over the serving glue")]
    // SAFETY: `factory` is the serving glue's factory; the module is a number it passes on, never loads; the out is ours.
    let status = unsafe { exact_software_adapter(factory.addr(), 0x2000, &mut adapter) };
    let calls = wcontainers_directx_graphics_infrastructure_recorder::calls(
        "IDXGIFactory::CreateSoftwareAdapter",
    );
    assert!(
        calls
            .last()
            .is_some_and(|call| call.contains("module=Hmodule(8192)")),
        "{status:#x} {calls:?}"
    );
}
