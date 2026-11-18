//! The shader cache on the recording back end: a second pipeline of the same shaders and the next
//! run's pipeline take their conversions from the cache. Its own test binary, since the cache is the
//! process's.

#[allow(dead_code)]
#[path = "../../wcontainers-directx-graphics-infrastructure/tests/support/record.rs"]
mod record;
#[allow(dead_code)]
#[path = "support/triangle.rs"]
mod triangle;

use std::io::Write;
use std::sync::{Arc, Mutex};

use record::Recorder;
use wcontainers_direct3d_12::{Device, cache};
use wcontainers_shaders::cache::Cache;

/// A file the test reads back: the bytes appended.
#[derive(Clone, Default)]
struct File(Arc<Mutex<Vec<u8>>>);

impl Write for File {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn pipelines_take_shaders_converted_before_in_the_process_and_in_the_last_run() {
    // First run: an empty file gains each conversion once.
    let file = File::default();
    let empty = Cache::default();
    file.0.lock().unwrap().extend_from_slice(empty.bytes());
    cache::open(empty, Box::new(file.clone()));
    let mut device = Device::new(Recorder::new());
    triangle::pipeline(&mut device, triangle::VS).unwrap();
    let (converted, decoded) = cache::counts();
    assert!(converted >= 2, "the vertex and pixel shaders: {converted}");
    triangle::pipeline(&mut device, triangle::VS).unwrap();
    assert_eq!(cache::counts(), (converted, decoded + converted));
    drop(device);
    let bytes = file.0.lock().unwrap().clone();
    let (loaded, dropped) = Cache::load(bytes).unwrap();
    assert_eq!((loaded.len() as u64, dropped), (converted, 0));

    // Next run: nothing is converted and nothing appended.
    let file = File::default();
    cache::open(loaded, Box::new(file.clone()));
    let mut device = Device::new(Recorder::new());
    triangle::pipeline(&mut device, triangle::VS).unwrap();
    assert_eq!(cache::counts(), (0, converted));
    assert!(file.0.lock().unwrap().is_empty());
}
