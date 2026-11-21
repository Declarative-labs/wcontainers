//! What a waiting call costs on our kernel: guest/waitcost.c repeats one kind of wait, and each
//! kind runs at two counts, so the difference is the cost of the extra waits alone. Prints the
//! wall time and the heap allocations of the whole test process per wait. Not a check: run it
//! by name, `cargo test --release -p wcontainers-executor-unicorn --test waitcost -- --ignored
//! --nocapture`, with `WAITCOST_COUNT` waits in the smaller run (twice as many in the larger) and
//! `WAITCOST_ROUNDS` runs of each, whose median is printed; `WAITCOST_KIND` keeps only the kinds
//! whose name holds it.

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use wcontainers_executor_unicorn::network::DOCKER_HOST_NAME;
use wcontainers_executor_unicorn::{
    driver, guest, guest_directory_entry, image, reference, standin,
};

/// Counts every allocation the test process makes.
struct Counting;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

// SAFETY: every call is passed to the system allocator unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: as the caller's contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: as the caller's contract.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: as the caller's contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

const DEFAULT_COUNT: u64 = 700;
const DEFAULT_ROUNDS: usize = 5;

fn knob<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// A server on this machine that echoes every byte at once as it comes. Returns its port.
fn echoing_server() -> u16 {
    let listener = TcpListener::bind("0.0.0.0:0").expect("a TCP port");
    let port = listener.local_addr().expect("an address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = stream.set_nodelay(true);
            std::thread::spawn(move || {
                let mut byte = [0u8];
                while matches!(stream.read(&mut byte), Ok(1)) {
                    if stream.write_all(&byte).is_err() {
                        return;
                    }
                }
            });
        }
    });
    port
}

/// One run of the guest with `arguments`, the root on the backend when `backend`; its wall time in
/// nanoseconds and the allocations made meanwhile.
fn run(directory: &Path, arguments: &[&str], backend: bool) -> (u64, u64) {
    let mut files = image::reference_image().to_vec();
    files.extend(image::host_directory(directory, guest_directory_entry()));
    let path = reference::in_guest_directory(std::ffi::OsStr::new("waitcost"));
    let mut all = vec![path.as_str()];
    all.extend_from_slice(arguments);
    let container = driver::Container {
        files: &files,
        path: &path,
        arguments: &all,
        environment: reference::ENVIRONMENT,
        working_directory: reference::ROOT_DIRECTORY,
        backend: backend.then_some(reference::ROOT_DIRECTORY),
        cached: false,
        published: &[],
        root_image: None,
        address_space: standin::SPACE_32_BIT,
    };
    let prepared = driver::prepare(&container);
    let allocations = ALLOCATIONS.load(Ordering::Relaxed);
    let started = Instant::now();
    let ran = driver::run_prepared(prepared, &container, 0, driver::Injections::default())
        .unwrap_or_else(|failure| panic!("no outcome: {failure:?}"));
    let took = started.elapsed().as_nanos() as u64;
    let allocations = ALLOCATIONS.load(Ordering::Relaxed) - allocations;
    let stdout = String::from_utf8_lossy(&ran.outcome.stdout);
    assert!(
        stdout.ends_with("done\n"),
        "{arguments:?}: {stdout}{}",
        String::from_utf8_lossy(&ran.outcome.stderr)
    );
    (took, allocations)
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

#[test]
#[ignore = "a measurement, run by name"]
fn wait_costs() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build("waitcost", &[], &out);
    let directory = program.parent().expect("program has a directory");
    let count: u64 = knob("WAITCOST_COUNT", DEFAULT_COUNT);
    let rounds: usize = knob("WAITCOST_ROUNDS", DEFAULT_ROUNDS);
    let port = echoing_server().to_string();
    let kinds: [(&str, Vec<String>, bool); 4] = [
        ("pipe ping-pong", vec!["pipe".into()], false),
        (
            "outside socket read",
            vec!["socket".into(), DOCKER_HOST_NAME.into(), port.clone()],
            false,
        ),
        (
            "backend open and read",
            vec!["file".into(), "/etc/os-release".into()],
            true,
        ),
        (
            "name lookup",
            vec!["name".into(), DOCKER_HOST_NAME.into()],
            false,
        ),
    ];
    let only: String = knob("WAITCOST_KIND", String::new());
    for (what, arguments, backend) in kinds.into_iter().filter(|(what, ..)| what.contains(&only)) {
        let (mut times, mut allocations) = (Vec::new(), Vec::new());
        for _ in 0..rounds {
            let at = |n: u64| {
                let mut all: Vec<&str> = arguments.iter().map(String::as_str).collect();
                let n = n.to_string();
                all.push(&n);
                run(directory, &all, backend)
            };
            let (short, short_allocations) = at(count);
            let (long, long_allocations) = at(2 * count);
            times.push((long as f64 - short as f64) / count as f64);
            allocations.push((long_allocations as f64 - short_allocations as f64) / count as f64);
        }
        println!(
            "{what}: {:.0} ns and {:.2} allocations per repetition (median of {rounds}, {count} and {} repetitions; times {times:.0?})",
            median(times.clone()),
            median(allocations),
            2 * count
        );
    }
}
