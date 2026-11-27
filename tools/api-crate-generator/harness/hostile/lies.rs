//! The hostile implementation set (src/glue_hostile.rs) through both glues, for every safe trait src/trust.rs lists:
//! the caller's bytes (lengths that change, panics, re-entry, calls that never return), the implementation's methods,
//! its objects' `Drop` and the bytes its Map lends (panics, re-entry, calls that never return, and for the bytes, lying
//! lengths and contents), and its `Clone`, `PartialEq` and `Hash`, which panic and must never be asked. The generated
//! driver calls every method the calling glue carries on every object it could make. Each test prints `LIE <what>: ...`.
//! A report of AddressSanitizer, a hang (`LIE HANG`, or the run's time limit) or a crash other than a caught panic is a
//! failure. Run one test per process.

#![forbid(unsafe_code)]

use std::sync::{Arc, Once};
use std::thread;
use std::time::{Duration, Instant};

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_component_object_model::structures::Hmodule;
use wcontainers_direct3d_11::enumerations::{CreateDeviceFlag, Map, Usage};
use wcontainers_direct3d_11::structures::{BufferDesc, Texture2DDesc};
use wcontainers_direct3d_11::{Device as _, DeviceContext as _, Direct3D11 as _, ResourceRef};
use wcontainers_direct3d_11_hostile::{self as hostile, Driven, Fuzzed, Lie, LyingBytes, Objects, Rng, Site, Way, When, drive, fuzz, make_all};
use wcontainers_direct3d_common::enumerations::{DriverType, FeatureLevel};
use wcontainers_directx_graphics_infrastructure::enumerations::Format;
use wcontainers_directx_graphics_infrastructure::structures::SampleDesc;
use wcontainers_volatile_bytes::VolatileBytes;

/// How long other calls may take while one is blocked by a lie.
const OTHERS_WITHIN: Duration = Duration::from_secs(60);
/// How long to wait for a blocking lie to be met.
const MET_WITHIN: Duration = Duration::from_secs(20);
/// D3D_FEATURE_LEVEL_11_0 and D3D11_SDK_VERSION.
const LEVELS: [FeatureLevel; 1] = [FeatureLevel(0xb000)];
const SDK_VERSION: u32 = 7;
/// D3D11_MAP_READ, WRITE, READ_WRITE and WRITE_DISCARD.
const MAP_TYPES: [Map; 4] = [Map(1), Map(2), Map(3), Map(4)];

/// The lies panic thousands of times: the default hook would print each.
fn quiet() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| std::panic::set_hook(Box::new(|_| {})));
}

/// Ends the process with `LIE HANG` when `body` takes longer than `limit`: a hang must not pass for a slow run.
fn within<R>(what: &str, limit: Duration, body: impl FnOnce() -> R) -> R {
    let (done, finished) = std::sync::mpsc::channel::<()>();
    let what = what.to_string();
    let watchdog = thread::spawn(move || {
        if finished.recv_timeout(limit).is_err() {
            println!("LIE HANG: {what} did not end within {limit:?}");
            std::process::exit(3);
        }
    });
    let answer = body();
    let _ = done.send(());
    let _ = watchdog.join();
    answer
}

/// Every object the makers can make, with a buffer and a texture of valid descriptions given first (a maker passes
/// zeros, which the glue refuses), so that the views and the rest are made from them.
fn world() -> Objects {
    let mut objects = Objects::new();
    make_all(&mut objects);
    seed_resources(&mut objects);
    make_all(&mut objects);
    objects
}

/// Gives a buffer and a texture of valid descriptions where none is held.
fn seed_resources(objects: &mut Objects) {
    let made = objects.device().map(|device| {
        let buffer = if objects.buffer().is_none() { device.create_buffer(&buffer_desc(), None).ok() } else { None };
        let texture = if objects.texture_2d().is_none() { device.create_texture_2d(&texture_desc(), &[]).ok() } else { None };
        (buffer, texture)
    });
    if let Some((buffer, texture)) = made {
        if let Some(buffer) = buffer {
            objects.put_buffer(buffer);
        }
        if let Some(texture) = texture {
            objects.put_texture_2d(texture);
        }
    }
}

fn texture_desc() -> Texture2DDesc {
    Texture2DDesc { width: 4, height: 4, mip_levels: 1, array_size: 1, format: Format(28), sample_desc: SampleDesc { count: 1, quality: 0 }, usage: Usage(0), bind_flags: 0x28, cpu_access_flags: 0, misc_flags: 0 }
}

fn honest() -> LyingBytes {
    LyingBytes::lengths(&[64])
}

fn plan(site: Site, way: Way, when: When) {
    hostile::plan(Some(Lie { site, way, when }));
}

/// Honest again; prints what is left alive, and fails when the glue asked a trait it never asks.
fn settle(what: &str, before: i64) {
    hostile::plan(None);
    hostile::on_reentry(None);
    let never = hostile::never_asked_calls();
    println!("LIE {what}: objects of the implementation alive {} (before {before}), never-asked calls {never}", hostile::live());
    assert_eq!(never, 0, "the glue asked an implementation's Clone, PartialEq or Hash");
}

fn show(what: &str, driven: Driven) {
    println!("LIE {what}: called {}, skipped {}, panicked back {}", driven.called, driven.skipped, driven.panicked);
}

/// How many calls of the implementation's methods an honest world and drive make.
fn honest_calls() -> u64 {
    hostile::plan(None);
    let objects = world();
    let _ = drive(&objects, &honest);
    drop(objects);
    hostile::calls_at(Site::Method)
}

/// The calls `At` tries: about `samples` of them, spread over `total`.
fn sampled(total: u64, samples: u64) -> impl Iterator<Item = u64> {
    let step = (total / samples.max(1)).max(1);
    (0..total).step_by(usize::try_from(step).unwrap_or(1))
}

/// Runs `blocked` on a thread while blocking lies block; each time a call is blocked, runs `others` here, which must end
/// within `OTHERS_WITHIN`, then releases the blocked call. Returns how many blocks were met and the longest `others`.
fn while_blocked(rounds: usize, blocked: impl FnOnce() + Send + 'static, others: impl Fn()) -> (usize, Duration) {
    hostile::set_blocking(true);
    let mut seen = hostile::blocks_begun();
    let worker = thread::spawn(blocked);
    let mut met = 0;
    let mut longest = Duration::ZERO;
    for _ in 0..rounds {
        // A new block, or the worker's end.
        let start = Instant::now();
        while hostile::blocks_begun() == seen && !worker.is_finished() && start.elapsed() < MET_WITHIN {
            thread::sleep(Duration::from_millis(1));
        }
        if hostile::blocks_begun() == seen {
            break;
        }
        seen = hostile::blocks_begun();
        met += 1;
        let began = Instant::now();
        within("other calls while one is blocked", OTHERS_WITHIN, &others);
        longest = longest.max(began.elapsed());
        hostile::release();
    }
    hostile::set_blocking(false);
    within("the blocked call after its release", OTHERS_WITHIN, || {
        let _ = worker.join();
    });
    (met, longest)
}

#[test]
fn an_honest_drive_reaches_every_method() {
    quiet();
    let before = hostile::live();
    let objects = world();
    println!("LIE honest: kinds with an object {}", objects.kinds_held());
    let driven = within("an honest drive", OTHERS_WITHIN, || drive(&objects, &honest));
    show("honest", driven);
    println!("LIE honest: calls of the implementation's methods {}", hostile::calls_at(Site::Method));
    assert!(driven.called > 0);
    drop(objects);
    settle("honest", before);
}

#[test]
fn the_callers_bytes_change_their_length_on_every_call() {
    quiet();
    let before = hostile::live();
    let objects = world();
    let sequences: [&[u64]; 9] = [&[0, 8], &[8, 0], &[16, 4096], &[4096, 16], &[u64::MAX], &[1 << 40, 0], &[0], &[3, 1 << 20], &[1 << 20, 3]];
    for lengths in sequences {
        let driven = within("a drive with changing lengths", OTHERS_WITHIN, || drive(&objects, &|| LyingBytes::lengths(lengths)));
        show(&format!("caller's bytes, lengths {lengths:?}"), driven);
    }
    drop(objects);
    settle("caller's bytes, lengths", before);
}

#[test]
fn the_callers_bytes_panic_at_each_call() {
    quiet();
    let before = hostile::live();
    let objects = world();
    for at in 0..4 {
        let driven = within("a drive with panicking bytes", OTHERS_WITHIN, || drive(&objects, &|| LyingBytes::lying(Way::Panic, at)));
        show(&format!("caller's bytes, panic at call {at}"), driven);
    }
    drop(objects);
    settle("caller's bytes, panic", before);
}

#[test]
fn the_callers_bytes_reenter_the_glue() {
    quiet();
    let before = hostile::live();
    let shared = Arc::new(world());
    // A call back into the glue from inside the bytes: every method again, on the same objects.
    let inner = Arc::clone(&shared);
    hostile::on_reentry(Some(Arc::new(move || {
        let _ = drive(&inner, &honest);
    })));
    for at in 0..3 {
        let driven = within("a drive with re-entering bytes", OTHERS_WITHIN * 5, || drive(&shared, &|| LyingBytes::lying(Way::Reenter, at)));
        show(&format!("caller's bytes, re-enter at call {at}"), driven);
    }
    hostile::on_reentry(None);
    drop(shared);
    settle("caller's bytes, re-enter", before);
}

#[test]
fn the_callers_bytes_never_return() {
    quiet();
    let before = hostile::live();
    for at in 0..2 {
        let (met, longest) = while_blocked(
            256,
            move || {
                let objects = world();
                let _ = drive(&objects, &|| LyingBytes::lying(Way::Block, at));
            },
            || {
                let objects = world();
                let _ = drive(&objects, &honest);
            },
        );
        println!("LIE caller's bytes, never return at call {at}: blocked {met} times, every other drive ended (longest {longest:?})");
    }
    settle("caller's bytes, never return", before);
}

#[test]
fn the_implementations_methods_panic() {
    quiet();
    let before = hostile::live();
    let total = honest_calls();
    println!("LIE implementation's methods: an honest world and drive make {total} calls");
    let mut whens: Vec<When> = [1, 2, 3, 5, 7, 11].into_iter().map(When::Every).collect();
    whens.extend(sampled(total, 150).map(When::At));
    let mut panicked = 0;
    for when in &whens {
        plan(Site::Method, Way::Panic, *when);
        let objects = world();
        let driven = within("a drive with panicking methods", OTHERS_WITHIN, || drive(&objects, &honest));
        panicked += hostile::lies_told();
        drop(objects);
        let _ = driven;
    }
    println!("LIE implementation's methods, panic: {} plans, {panicked} panics told, every one caught", whens.len());
    settle("implementation's methods, panic", before);
}

#[test]
fn the_implementations_methods_reenter_the_glue() {
    quiet();
    let before = hostile::live();
    let total = honest_calls();
    // A call back into the glue from inside a method: a new world made and driven, then dropped.
    hostile::on_reentry(Some(Arc::new(|| {
        let objects = world();
        let _ = drive(&objects, &honest);
    })));
    let mut told = 0;
    let mut whens: Vec<When> = vec![When::Every(97), When::Every(211)];
    whens.extend(sampled(total, 40).map(When::At));
    for when in &whens {
        plan(Site::Method, Way::Reenter, *when);
        hostile::on_reentry(Some(Arc::new(|| {
            let objects = world();
            let _ = drive(&objects, &honest);
        })));
        let objects = world();
        let _ = within("a drive with re-entering methods", OTHERS_WITHIN * 10, || drive(&objects, &honest));
        told += hostile::lies_told();
        drop(objects);
    }
    // And on the same objects as the call under way.
    let shared = Arc::new(world());
    let inner = Arc::clone(&shared);
    plan(Site::Method, Way::Reenter, When::Every(53));
    hostile::on_reentry(Some(Arc::new(move || {
        let _ = drive(&inner, &honest);
    })));
    let _ = within("a drive re-entering on its own objects", OTHERS_WITHIN * 10, || drive(&shared, &honest));
    told += hostile::lies_told();
    hostile::on_reentry(None);
    drop(shared);
    println!("LIE implementation's methods, re-enter: {} plans, {told} re-entries", whens.len() + 1);
    settle("implementation's methods, re-enter", before);
}

#[test]
fn the_implementations_methods_never_return() {
    quiet();
    let before = hostile::live();
    let total = honest_calls();
    let mut met = 0;
    let mut longest = Duration::ZERO;
    let samples: Vec<u64> = sampled(total, 30).collect();
    for at in &samples {
        plan(Site::Method, Way::Block, When::At(*at));
        let (blocked, took) = while_blocked(
            1,
            || {
                let objects = world();
                let _ = drive(&objects, &honest);
            },
            || {
                let objects = world();
                let _ = drive(&objects, &honest);
            },
        );
        met += blocked;
        longest = longest.max(took);
    }
    println!("LIE implementation's methods, never return: {} plans, blocked {met} times, every other drive ended (longest {longest:?})", samples.len());
    settle("implementation's methods, never return", before);
}

#[test]
fn the_implementations_drops_lie() {
    quiet();
    let before = hostile::live();
    let mut told = 0;
    for every in [1, 2, 3] {
        plan(Site::Drop, Way::Panic, When::Every(every));
        let objects = world();
        let _ = within("a drive with panicking drops", OTHERS_WITHIN, || drive(&objects, &honest));
        drop(objects);
        told += hostile::lies_told();
    }
    println!("LIE implementation's Drop, panic: {told} panics told");
    told = 0;
    for every in [1, 3] {
        plan(Site::Drop, Way::Reenter, When::Every(every));
        hostile::on_reentry(Some(Arc::new(|| {
            let objects = world();
            let _ = drive(&objects, &honest);
        })));
        let objects = world();
        let _ = within("a drive with re-entering drops", OTHERS_WITHIN * 10, || drive(&objects, &honest));
        drop(objects);
        told += hostile::lies_told();
    }
    hostile::on_reentry(None);
    println!("LIE implementation's Drop, re-enter: {told} re-entries");
    let mut met = 0;
    for at in 0..20 {
        plan(Site::Drop, Way::Block, When::At(at * 7));
        let (blocked, _) = while_blocked(
            1,
            || {
                let objects = world();
                let _ = drive(&objects, &honest);
                drop(objects);
            },
            || {
                let objects = world();
                let _ = drive(&objects, &honest);
            },
        );
        met += blocked;
    }
    println!("LIE implementation's Drop, never return: blocked {met} times, every other drive ended");
    settle("implementation's Drop", before);
}

/// A sink and a source of the test's, for the bytes a Map lends.
struct Sink(usize);

impl ByteSink for Sink {
    type Error = core::convert::Infallible;

    fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
        self.0 += bytes.len();
        Ok(bytes.len())
    }
}

struct Source;

impl ByteSource for Source {
    type Error = core::convert::Infallible;

    fn read(&mut self, into: &mut [u8]) -> Result<usize, Self::Error> {
        into.fill(0x3c);
        Ok(into.len())
    }
}

fn new_device() -> (hostile::Objects, Option<(wcontainers_direct3d_11_direct::SystemDevice<hostile::Api>, wcontainers_direct3d_11_direct::SystemDeviceContext<hostile::Api>)>) {
    let objects = Objects::new();
    let made = objects.api.create_device(None, DriverType(1), Hmodule(0), CreateDeviceFlag(0), &LEVELS, SDK_VERSION).ok().map(|(device, _, context)| (device, context));
    (objects, made)
}

fn buffer_desc() -> BufferDesc {
    BufferDesc { byte_width: 4096, usage: Usage(2), bind_flags: 1, cpu_access_flags: 0x3_0000, misc_flags: 0, structure_byte_stride: 0 }
}

/// Maps the buffer every way, uses the bytes lent through every method, and unmaps; returns how many Maps succeeded.
fn use_mapped(
    context: &wcontainers_direct3d_11_direct::SystemDeviceContext<hostile::Api>,
    buffer: &wcontainers_direct3d_11_direct::SystemBuffer<hostile::Api>,
) -> usize {
    let mut mapped = 0;
    for map_type in MAP_TYPES {
        let Ok(lent) = context.map(ResourceRef::Buffer(buffer), 0, map_type, 0) else { continue };
        mapped += 1;
        let length = usize::try_from(lent.data.len()).unwrap_or(0).min(1 << 20);
        let _ = lent.data.write(&vec![0x5a; length]);
        let mut back = vec![0; length];
        let _ = lent.data.read(&mut back);
        let _ = lent.data.read_to(&mut Sink(0));
        let _ = lent.data.write_from(&mut Source);
        context.unmap(ResourceRef::Buffer(buffer), 0);
        // After its Unmap the memory is gone: reads and writes reach nothing.
        assert_eq!(lent.data.write(&[1; 16]), 0, "lent memory written after its Unmap");
    }
    mapped
}

#[test]
fn the_bytes_a_map_lends_lie() {
    quiet();
    let before = hostile::live();
    let (objects, made) = new_device();
    let Some((device, context)) = made else { panic!("no device") };
    let buffer = device.create_buffer(&buffer_desc(), None).expect("a buffer");
    let mut mapped = 0;
    // Lying lengths and contents alone, many times over (the lengths take turns).
    hostile::plan(None);
    for _ in 0..64 {
        mapped += within("Maps of lying bytes", OTHERS_WITHIN, || use_mapped(&context, &buffer));
    }
    println!("LIE Map's bytes, lying lengths and contents: {mapped} Maps of 256 succeeded");
    // Panics.
    let mut told = 0;
    for every in [1, 2, 3, 5] {
        plan(Site::Mapped, Way::Panic, When::Every(every));
        for _ in 0..16 {
            let _ = within("Maps of panicking bytes", OTHERS_WITHIN, || use_mapped(&context, &buffer));
        }
        told += hostile::lies_told();
    }
    println!("LIE Map's bytes, panic: {told} panics told");
    // Re-entry: the same buffer unmapped and mapped again from inside the bytes' methods.
    let (inner_context, inner_buffer) = (context.clone(), buffer.clone());
    told = 0;
    for every in [1, 2, 3] {
        plan(Site::Mapped, Way::Reenter, When::Every(every));
        let (c, b) = (inner_context.clone(), inner_buffer.clone());
        hostile::on_reentry(Some(Arc::new(move || {
            c.unmap(ResourceRef::Buffer(&b), 0);
            let _ = use_mapped(&c, &b);
        })));
        for _ in 0..16 {
            let _ = within("Maps of re-entering bytes", OTHERS_WITHIN, || use_mapped(&context, &buffer));
        }
        told += hostile::lies_told();
    }
    hostile::on_reentry(None);
    drop((inner_context, inner_buffer));
    println!("LIE Map's bytes, re-enter: {told} re-entries");
    // Never returning: another buffer of another device is mapped meanwhile.
    let mut met = 0;
    for at in 0..24 {
        plan(Site::Mapped, Way::Block, When::At(at));
        let (c, b) = (context.clone(), buffer.clone());
        let (blocked, _) = while_blocked(
            1,
            move || {
                let _ = use_mapped(&c, &b);
            },
            || {
                let (_objects, made) = new_device();
                if let Some((other_device, other_context)) = made
                    && let Ok(other) = other_device.create_buffer(&buffer_desc(), None)
                {
                    let _ = use_mapped(&other_context, &other);
                }
            },
        );
        met += blocked;
    }
    println!("LIE Map's bytes, never return: blocked {met} times, every other Map ended");
    drop((buffer, context, device, objects));
    settle("Map's bytes", before);
}

/// Fuzzing every method the calling glue carries, through both glues, with random arguments: values with edges,
/// structures of random bytes, slices of one length per call, objects held or none, the caller's bytes of lengths that
/// change; each round of 50,000 calls under a lie of the hostile implementation picked at random (one round in three
/// honest), some objects dropped and made again every 4096 calls. FUZZ_SEED and FUZZ_CALLS choose the run.
#[test]
#[ignore = "a long run: FUZZ_SEED and FUZZ_CALLS"]
fn fuzz_every_method() {
    quiet();
    let number = |name: &str, default: u64| std::env::var(name).ok().and_then(|value| value.parse().ok()).unwrap_or(default);
    let seed = number("FUZZ_SEED", 1);
    let calls = number("FUZZ_CALLS", 100_000);
    let before = hostile::live();
    let mut rng = Rng::new(seed);
    let mut lies = Rng::new(seed ^ 0x5eed_0f11e5);
    let mut objects = world();
    hostile::on_reentry(Some(Arc::new(|| {
        let objects = Objects::new();
        let _ = objects.api.create_device(None, DriverType(1), Hmodule(0), CreateDeviceFlag(0), &LEVELS, SDK_VERSION);
    })));
    let mut total = Fuzzed::default();
    let mut told = 0;
    let mut done = 0;
    let started = Instant::now();
    while done < calls {
        let round = (calls - done).min(50_000);
        if lies.below(3) == 0 {
            hostile::plan(None);
        } else {
            let site = [Site::Method, Site::Drop, Site::Mapped][lies.below(3)];
            let way = [Way::Panic, Way::Reenter][lies.below(2)];
            plan(site, way, When::Every(7 + lies.step() % 500));
        }
        let fuzzed = within("a fuzzing round", OTHERS_WITHIN * 5, || fuzz(&mut objects, &mut rng, round, &seed_resources));
        told += hostile::lies_told();
        total.driven.called += fuzzed.driven.called;
        total.driven.skipped += fuzzed.driven.skipped;
        total.driven.panicked += fuzzed.driven.panicked;
        total.remade += fuzzed.remade;
        done += round;
    }
    hostile::plan(None);
    hostile::on_reentry(None);
    println!(
        "FUZZ seed {seed}: calls {calls} in {:?}; called {}, skipped {}, panicked back {}; lies told {told}; objects made again {} times; kinds held at the end {}",
        started.elapsed(),
        total.driven.called,
        total.driven.skipped,
        total.driven.panicked,
        total.remade,
        objects.kinds_held()
    );
    drop(objects);
    // A Map the fuzzer never unmapped keeps its context and resource alive for the memory lent, as the system would.
    println!("FUZZ seed {seed}: mappings of the system never unmapped {}", wcontainers_direct3d_11_direct::foreign_lent_outstanding());
    settle(&format!("fuzz seed {seed}"), before);
}
