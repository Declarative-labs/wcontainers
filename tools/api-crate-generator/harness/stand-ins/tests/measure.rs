//! Cost per call in nanoseconds, as the spike measured it: the median of repeated runs, each a loop of one operation.
//! Run alone, pinned: `taskset -c N cargo test --release --test measure -- --ignored --nocapture`. "Direct" is the C
//! game on the fake; "added by the serving glue" is the C game on the front through the serving glue minus the front
//! called from Rust; "pass-through" is the C game through both glues to the fake.

use std::hint::black_box;
use std::time::Instant;

use wcontainers_component_object_model::structures::Hmodule;
use wcontainers_direct3d_11::enumerations::{CreateDeviceFlag, Map, Usage};
use wcontainers_direct3d_11::structures::{BufferDesc, SubresourceData};
use wcontainers_direct3d_11::{DeviceChild as _, Device as _, DeviceContext as _, Direct3D11, ResourceRef};
use wcontainers_direct3d_11_direct_harness::front::Front;
use wcontainers_direct3d_11_direct_harness::{Entry, Operation};
use wcontainers_direct3d_common::enumerations::{DriverType, FeatureLevel};

const REPETITIONS: usize = 31;
const DYNAMIC: BufferDesc = BufferDesc { byte_width: 64, usage: Usage(2), bind_flags: 1, cpu_access_flags: 0x10000, misc_flags: 0, structure_byte_stride: 0 };
const SMALL: BufferDesc = BufferDesc { byte_width: 16, usage: Usage(0), cpu_access_flags: 0, ..DYNAMIC };

/// Returns the nanoseconds one loop of the operation took, called from Rust through the traits, with no COM.
fn through_traits<M: Direct3D11>(module: &M, operation: Operation, iterations: u64) -> u64 {
    let (device, _, context) = module.create_device(None, DriverType(1), Hmodule(0), CreateDeviceFlag(0), &[FeatureLevel(0xb000)], 7).expect("a device");
    let dynamic = device.create_buffer(&DYNAMIC, None).expect("a dynamic buffer");
    let fixed = device.create_buffer(&SMALL, None).expect("a buffer");
    let start = Instant::now();
    match operation {
        Operation::Draw => (0..iterations).for_each(|_| context.draw(black_box(3), 0)),
        Operation::SetVertexBuffers => (0..iterations).for_each(|_| {
            context.ia_set_vertex_buffers(0, black_box(&[Some(&dynamic), Some(&fixed)]), &[16, 8], &[0, 4]);
        }),
        Operation::AddRefRelease | Operation::QueryInterfaceRelease => (0..iterations).for_each(|_| drop(black_box(dynamic.clone()))),
        Operation::MapUnmap => (0..iterations).for_each(|_| {
            drop(black_box(context.map(ResourceRef::Buffer(&dynamic), 0, Map(4), 0).ok()));
            context.unmap(ResourceRef::Buffer(&dynamic), 0);
        }),
        Operation::GetDeviceRelease => (0..iterations).for_each(|_| drop(black_box(dynamic.get_device()))),
        Operation::CreateBufferRelease => (0..iterations).for_each(|_| {
            let initial = SubresourceData { sys_mem: vec![0_u8; 16], sys_mem_pitch: 0, sys_mem_slice_pitch: 0 };
            drop(black_box(device.create_buffer(&SMALL, Some(&initial)).ok()));
        }),
    }
    start.elapsed().as_nanos() as u64
}

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

/// Returns the median nanoseconds per iteration of `run`.
fn per_call(iterations: u64, mut run: impl FnMut(u64) -> u64) -> f64 {
    run(iterations / 10);
    median((0..REPETITIONS).map(|_| run(iterations) as f64 / iterations as f64).collect())
}

fn load() -> String {
    std::fs::read_to_string("/proc/loadavg").unwrap_or_default().trim().to_string()
}

/// Reads a setting of the profile run from the environment.
fn setting(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

/// One operation on one path, long enough for a sampling profiler: `PROFILE_OPERATION` (an `Operation`'s name),
/// `PROFILE_PATH` (`served`: the C game on the front through the serving glue; `calling`: Rust through the calling glue
/// to the fake; anything else: pass-through) and `PROFILE_ITERATIONS`.
#[test]
#[ignore = "a profile, run on its own under a sampling profiler"]
fn profile() {
    let operation = match setting("PROFILE_OPERATION").as_str() {
        "MapUnmap" => Operation::MapUnmap,
        "CreateBufferRelease" => Operation::CreateBufferRelease,
        "GetDeviceRelease" => Operation::GetDeviceRelease,
        "AddRefRelease" => Operation::AddRefRelease,
        _ => Operation::Draw,
    };
    let iterations = setting("PROFILE_ITERATIONS").parse().unwrap_or(5_000_000);
    let nanoseconds = match setting("PROFILE_PATH").as_str() {
        "served" => Entry::served(Front).time(operation, iterations),
        "calling" => through_traits(&Entry::fake().system(), operation, iterations),
        _ => Entry::pass_through().time(operation, iterations),
    };
    println!("PROFILE {operation:?} {}: {:.1} ns a call", setting("PROFILE_PATH"), nanoseconds as f64 / iterations as f64);
}

#[test]
#[ignore = "a measurement, run on its own"]
fn measures() {
    println!("load before: {}", load());
    let (fake, served_front, pass_through) = (Entry::fake(), Entry::served(Front), Entry::pass_through());
    let system = Entry::fake().system();
    println!("| operation | fake, raw COM | system through calling glue, Rust | pass-through, raw COM | front, Rust | front through serving glue, raw COM | serving glue | pass-through over raw |");
    println!("|---|---|---|---|---|---|---|---|");
    let operations = [
        (Operation::Draw, 2_000_000),
        (Operation::SetVertexBuffers, 1_000_000),
        (Operation::AddRefRelease, 1_000_000),
        (Operation::QueryInterfaceRelease, 1_000_000),
        (Operation::GetDeviceRelease, 500_000),
        (Operation::MapUnmap, 500_000),
        (Operation::CreateBufferRelease, 200_000),
    ];
    for (operation, iterations) in operations {
        let raw = per_call(iterations, |n| fake.time(operation, n));
        let calling = per_call(iterations, |n| through_traits(&system, operation, n));
        let chain = per_call(iterations, |n| pass_through.time(operation, n));
        let direct = per_call(iterations, |n| through_traits(&Front, operation, n));
        let served = per_call(iterations, |n| served_front.time(operation, n));
        println!("| {operation:?} | {raw:.1} | {calling:.1} | {chain:.1} | {direct:.1} | {served:.1} | {:.1} | {:.1} |", served - direct, chain - raw);
    }
    println!("load after: {}", load());
}
