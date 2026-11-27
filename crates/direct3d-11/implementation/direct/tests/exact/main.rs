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

/// The resources of every test: each class of format (plain, packed pairs, block-compressed, depth, refused planar)
/// in each dimension, with arrays and mip chains.
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
    let (mut created, mut uploads, mut maps, mut touched_bytes) = (0, 0, 0, 0_u64);
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
                let Some(length) =
                    reference_upload(resource, index, region.as_ref(), row_pitch, depth_pitch)
                else {
                    continue;
                };
                let region_words = region.as_ref().map(box_words);
                let region_pointer = region_words
                    .as_ref()
                    .map_or(core::ptr::null(), |words| words.as_ptr());
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
                uploads += 1;
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
        "EXACT SERVING: {created} resources created, {uploads} uploads, {maps} maps touching {touched_bytes} bytes"
    );
    assert!(created >= 14 && uploads > 0 && maps > 0);
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

/// What the stand-in saw: creations, uploads, maps, unmaps, calls of a slot it does not serve.
fn seen() -> [u64; 5] {
    let mut seen = [0_u64; 5];
    #[expect(unsafe_code, reason = "exact.c's stand-in's counts")]
    // SAFETY: five words.
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
    let (mut short_refused, mut uploads, mut maps, mut viewed_bytes) = (0, 0, 0, 0_u64);
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
                        assert_eq!(
                            mapped.data.len(),
                            u64::try_from(view).unwrap_or(0),
                            "{resource:?} {index} pitches {row_pitch} {depth_pitch}: the view is the reference's"
                        );
                        let mut bytes = vec![0_u8; view];
                        assert_eq!(mapped.data.read(&mut bytes), view);
                        assert_eq!(mapped.data.write(&bytes), view);
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
        "EXACT CALLING: {short_refused} short bytes refused, {uploads} exact uploads, {maps} maps viewing {viewed_bytes} bytes; the stand-in saw {seen:?}"
    );
    assert_eq!(seen[2], seen[3], "every map unmapped");
    assert_eq!(
        seen[4], 0,
        "the calling glue called no slot the stand-in does not serve"
    );
    assert!(uploads > 0 && maps > 0);
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
