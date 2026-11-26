//! A call stream recorded through [`Traced`] into a file replays on a new back end to the same bytes
//! as the recorded run read back.

use wcontainers_gpu::resource::{BufferDesc, BufferUsage, MapIntent, ReadSource, WriteMode};
use wcontainers_gpu::{Gpu, StandaloneGpu, error::DeviceStatus};
use wcontainers_gpu_recording::{Recorder, RecorderConfig};
use wcontainers_gpu_trace::{Replay, Traced, TracedConfig};

/// The bytes the recorded run writes, copies and reads back.
const WRITTEN: [u8; 16] = [3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3];

/// Writes [`WRITTEN`] into one buffer, copies it into a second and reads the second back.
fn copy_and_read<B: Gpu>(b: &mut B) -> Vec<u8> {
    let size = WRITTEN.len() as u64;
    let desc = |usage| BufferDesc {
        size,
        usage,
        intent: MapIntent::Rarely,
    };
    let src = b
        .create_buffer(&desc(BufferUsage::COPY_SRC | BufferUsage::COPY_DST))
        .expect("create_buffer");
    let dst = b
        .create_buffer(&desc(BufferUsage::COPY_SRC | BufferUsage::COPY_DST))
        .expect("create_buffer");
    b.write_buffer(&src, 0, &WRITTEN, WriteMode::Ordered);
    b.copy_buffer(&src, 0, &dst, 0, size);
    let mut read = vec![0; WRITTEN.len()];
    b.read_back_now(
        &ReadSource::Buffer {
            buffer: &dst,
            offset: 0,
            size,
        },
        &mut read,
    )
    .expect("read_back_now");
    read
}

#[test]
fn a_recorded_stream_replays_to_the_bytes_the_recorded_run_read() {
    let path =
        std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gpu-trace-recorded.stream");
    let recorded = {
        let mut b = <Traced<Recorder> as StandaloneGpu>::new(TracedConfig {
            inner: RecorderConfig { caps: None },
            file: Some(path.clone()),
        })
        .expect("a recorder and the stream file");
        let read = copy_and_read(&mut b);
        assert_eq!(b.status(), DeviceStatus::Ok);
        read
    };
    assert_eq!(recorded, WRITTEN);
    let stream = std::fs::read(&path).expect("the stream file");
    assert!(!stream.is_empty(), "the stream holds the calls");
    let mut b =
        <Recorder as StandaloneGpu>::new(RecorderConfig { caps: None }).expect("a recorder");
    let mut replay = Replay::new(stream);
    while let Some(frame) = replay.frame(&mut b, None, &mut |_, _| {}) {
        let frame = frame.expect("the stream decodes");
        assert_eq!(frame.failed, 0, "{:?}", frame.first_failure);
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(replay.read(), WRITTEN);
}
