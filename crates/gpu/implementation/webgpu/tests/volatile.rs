//! What writing from and reading into bytes another party holds costs, against copying them out first
//! (the interface's defaults): run with `--ignored --nocapture`; it prints nanoseconds per MiB.
#![cfg(not(target_arch = "wasm32"))]

use std::cell::RefCell;
use std::time::Instant;

use wcontainers_gpu::{
    Gpu, StandaloneGpu,
    resource::{BufferDesc, BufferUsage, MapIntent, ReadSource, WriteMode},
};
use wcontainers_gpu_webgpu::{WebGpu, WebGpuConfig};
use wcontainers_volatile_bytes::VolatileBytes;

/// Bytes of one write or read back.
const PART: usize = 4 << 20;
/// Writes or read backs timed for each route.
const ROUNDS: usize = 64;
/// Bytes in a MiB.
const MIB: f64 = (1u64 << 20) as f64;

fn device() -> WebGpu {
    <WebGpu as StandaloneGpu>::new(WebGpuConfig {
        backends: wgpu::Backends::PRIMARY,
        software: true,
    })
    .expect("a WebGPU adapter")
}

fn per_mib(start: Instant) -> f64 {
    start.elapsed().as_nanos() as f64 / ((PART * ROUNDS) as f64 / MIB)
}

#[test]
#[ignore = "a measurement: run with --ignored --nocapture"]
fn guest_bytes_copied_once_cost_less_than_copied_out_first() {
    let mut b = device();
    let dst = b
        .create_buffer(&BufferDesc {
            size: PART as u64,
            usage: BufferUsage::COPY_DST | BufferUsage::COPY_SRC,
            intent: MapIntent::Rarely,
        })
        .expect("create_buffer");
    let mut held: Vec<u8> = (0..PART).map(|i| i as u8).collect();
    let guest = RefCell::new(held.as_mut_slice());
    // Warm both routes up once.
    b.write_buffer_from(&dst, 0, &guest, WriteMode::Ordered);
    let fence = b.submit();
    b.wait_fence(&fence);

    let start = Instant::now();
    for _ in 0..ROUNDS {
        let mut copy = vec![0; PART];
        guest.read(&mut copy);
        b.write_buffer(&dst, 0, &copy, WriteMode::Ordered);
        let fence = b.submit();
        b.wait_fence(&fence);
    }
    let copied_out = per_mib(start);
    let start = Instant::now();
    for _ in 0..ROUNDS {
        b.write_buffer_from(&dst, 0, &guest, WriteMode::Ordered);
        let fence = b.submit();
        b.wait_fence(&fence);
    }
    let once = per_mib(start);
    println!(
        "write: copied out first {copied_out:.0} ns/MiB, copied once {once:.0} ns/MiB, saved {:.0} ns/MiB",
        copied_out - once
    );

    let read_back = |b: &mut WebGpu| {
        let rb = b
            .read_back(&ReadSource::Buffer {
                buffer: &dst,
                offset: 0,
                size: PART as u64,
            })
            .expect("read_back");
        b.submit();
        b.wait_read(&rb);
        rb
    };
    let mut copy_time = 0u128;
    for _ in 0..ROUNDS {
        let rb = read_back(&mut b);
        let start = Instant::now();
        let mut copy = vec![0; PART];
        b.finish_read(rb, &mut copy);
        guest.write(&copy);
        copy_time += start.elapsed().as_nanos();
    }
    let mut once_time = 0u128;
    for _ in 0..ROUNDS {
        let rb = read_back(&mut b);
        let start = Instant::now();
        b.finish_read_into(rb, &guest);
        once_time += start.elapsed().as_nanos();
    }
    let mibs = (PART * ROUNDS) as f64 / MIB;
    let (copied_out, once) = (copy_time as f64 / mibs, once_time as f64 / mibs);
    println!(
        "finish read: copied out first {copied_out:.0} ns/MiB, copied once {once:.0} ns/MiB, saved {:.0} ns/MiB",
        copied_out - once
    );
    assert_eq!(
        guest.borrow()[PART - 1],
        (PART - 1) as u8,
        "the bytes read back are the ones written"
    );
}
