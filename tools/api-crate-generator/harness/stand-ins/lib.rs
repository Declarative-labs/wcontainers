//! The spike's harness on the generated glue, for tests only: the fake system Direct3D 11 (C), the correct, hostile
//! and fuzzing C programs, a well-behaved implementation (`front`) and a lying one (`liar`), joined through the
//! generated serving glue (`exports`) and calling glue (`SystemDirect3D11`). The C is the spike's, unchanged: it
//! calls every object through its table index, as the real ABI lays it out.

#[path = "front.rs"]
pub mod front;
#[path = "liar.rs"]
pub mod liar;

use wcontainers_component_object_model_direct::Raw;
use wcontainers_direct3d_11::Direct3D11;
use wcontainers_direct3d_11_direct::{Library, SystemDirect3D11, exports};
use wcontainers_directx_graphics_infrastructure::DirectXGraphicsInfrastructure;

/// D3D11CreateDevice as C calls it.
pub type CreateDevice = unsafe extern "system" fn(Raw, i32, usize, u32, *const u8, u32, u32, *mut Raw, *mut u8, *mut Raw) -> i32;
/// D3D11CreateDeviceAndSwapChain as C calls it.
pub type CreateDeviceAndSwapChain =
    unsafe extern "system" fn(Raw, i32, usize, u32, *const u8, u32, u32, *const u8, *mut Raw, *mut Raw, *mut u8, *mut Raw) -> i32;

#[expect(unsafe_code, reason = "the fake's exports, declared as d3d11.h declares them")]
unsafe extern "system" {
    fn D3D11CreateDevice(
        adapter: Raw,
        driver_type: i32,
        software: usize,
        flags: u32,
        feature_levels: *const u8,
        feature_level_count: u32,
        sdk_version: u32,
        device: *mut Raw,
        feature_level: *mut u8,
        immediate_context: *mut Raw,
    ) -> i32;
    fn D3D11CreateDeviceAndSwapChain(
        adapter: Raw,
        driver_type: i32,
        software: usize,
        flags: u32,
        feature_levels: *const u8,
        feature_level_count: u32,
        sdk_version: u32,
        swap_chain_desc: *const u8,
        swap_chain: *mut Raw,
        device: *mut Raw,
        feature_level: *mut u8,
        immediate_context: *mut Raw,
    ) -> i32;
}

/// What the fake saw, as d3d11_slice.h lays it out.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Observed {
    pub create_calls: u32,
    pub create_flags: u32,
    pub ia_calls: u32,
    pub ia_start: u32,
    pub ia_count: u32,
    pub om_calls: u32,
    pub om_targets: u32,
    pub om_depth: u32,
    pub om_views: u32,
    pub om_first_view: u32,
    pub mapped_now: u32,
    pub create_driver_type: i32,
    pub protected_contexts: u32,
    pub unprotections: u32,
    pub single_threaded_creates: u32,
}

#[expect(unsafe_code, reason = "the stand-ins' own C functions")]
unsafe extern "C" {
    fn fake_live_objects() -> i64;
    fn fake_live_by_kind(out: *mut i64);
    fn fake_observed(out: *mut Observed);
    fn game_play(create: CreateDevice, report: *mut u8, capacity: usize) -> i32;
    fn game_hostile(served: CreateDevice, foreign: CreateDevice, report: *mut u8, capacity: usize) -> i32;
    fn game_hammer(create: CreateDevice, threads: u32, iterations: u32) -> i32;
    fn game_time(create: CreateDevice, operation: u32, iterations: u64) -> u64;
    fn game_textures(create: CreateDeviceAndSwapChain, report: *mut u8, capacity: usize) -> i32;
    fn attack_stale_reuse(create: CreateDevice, out: *mut u8, capacity: usize) -> i32;
    fn attack_stale_bound(create: CreateDevice, later: u32, out: *mut u8, capacity: usize) -> i32;
    fn attack_table_full(
        create: CreateDevice,
        count: u32,
        made: *mut u32,
        refused: *mut u32,
        query_made: *mut u32,
        out: *mut u8,
        capacity: usize,
    ) -> i32;
    fn attack_release_inside_call(create: CreateDevice, out: *mut u8, capacity: usize) -> i32;
    fn attack_release_from_other_thread(create: CreateDevice, out: *mut u8, capacity: usize) -> i32;
    fn attack_write_mapped(create: CreateDevice, width: u32, out: *mut u8, capacity: usize) -> i32;
    fn attack_write_mapped_texture(create: CreateDevice, side: u32, out: *mut u8, capacity: usize) -> i32;
    fn attack_output_merger(create: CreateDevice, out: *mut u8, capacity: usize) -> i32;
    fn attack_undo_threads(create: CreateDevice, out: *mut u8, capacity: usize) -> i32;
    fn check2_get_device_twice(create: CreateDevice, out: *mut u8, capacity: usize) -> i32;
    fn check2_held_texture_after_draw(create: CreateDevice, out: *mut u8, capacity: usize) -> i32;
    fn check2_get_type_then_get_desc(create: CreateDevice, exact: i32, out: *mut u8, capacity: usize) -> i32;
    fn check2_const_initial(create: CreateDevice, out: *mut u8, capacity: usize) -> i32;
    fn check2_fill_by_depth_pitch(create: CreateDevice, out: *mut u8, capacity: usize) -> i32;
    fn check2_huge_texture(create: CreateDevice, out: *mut u8, capacity: usize) -> i32;
    fn check2_callbacks(create: CreateDevice, out: *mut u8, capacity: usize) -> i32;
    fn check2_write_after_release(create: CreateDevice, out: *mut u8, capacity: usize) -> i32;
    fn check2_map_race(create: CreateDevice, threads: u32, iterations: u32, out: *mut u8, capacity: usize) -> i32;
    fn fuzz_correct(create: CreateDevice, create_swap: CreateDeviceAndSwapChain, seed: u64, calls: u64, threads: u32) -> u64;
    fn fuzz_hostile(create: CreateDevice, create_swap: CreateDeviceAndSwapChain, seed: u64, calls: u64, threads: u32) -> u64;
    fn fuzz_progress() -> u64;
    fn fuzz_violation_count() -> u32;
    fn fuzz_first_violation() -> *const core::ffi::c_char;
    fn fuzz_count(out: *mut u64);
    #[link_name = "fuzz_violations_by_line"]
    fn fuzz_violation_lines(out: *mut u32, lines: u32);
}

/// What `Entry::time` times: one call or one pair, through raw COM.
#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Draw = 0,
    SetVertexBuffers = 1,
    AddRefRelease = 2,
    MapUnmap = 3,
    GetDeviceRelease = 4,
    QueryInterfaceRelease = 5,
    CreateBufferRelease = 6,
}

/// The C attacks, each a hostile or careless program.
#[derive(Clone, Copy, Debug)]
pub enum Attack {
    StaleReuse,
    StaleBound(u32),
    ReleaseInsideCall,
    ReleaseFromOtherThread,
    WriteMapped(u32),
    WriteMappedTexture(u32),
    OutputMerger,
    /// Asks for a single-threaded device and turns the context's multithread protection off, twice.
    UndoThreads,
}

/// check-2's C games (c/check2.c).
#[derive(Clone, Copy, Debug)]
pub enum Check2 {
    GetDeviceTwice,
    HeldTextureAfterDraw,
    GetTypeThenGetDesc { exact: bool },
    ConstInitial,
    FillByDepthPitch,
    HugeTexture,
    Callbacks,
    WriteAfterRelease,
    MapRace { threads: u32, iterations: u32 },
}

/// How a full table answered: buffers made and refused, and whether a query was made after all were released.
#[derive(Clone, Copy, Debug, Default)]
pub struct TableFull {
    pub ok_but_null: i32,
    pub made: u32,
    pub refused: u32,
    pub query_made: u32,
}

/// Which of the two fuzzing drivers of c/fuzz.c.
#[derive(Clone, Copy, Debug)]
pub enum Driver {
    /// A correct game, which checks every answer the spike's glue promised.
    Correct,
    /// A hostile game: random entries, random objects, random arguments.
    Hostile,
}

/// The two exports of a Direct3D 11: the fake's, or ones the serving glue makes.
#[derive(Clone, Copy)]
pub struct Entry(Library);

impl Entry {
    /// The fake system Direct3D 11's own exports.
    pub fn fake() -> Entry {
        Entry(Library { d_3d11_create_device: D3D11CreateDevice, d_3d11_create_device_and_swap_chain: D3D11CreateDeviceAndSwapChain })
    }

    /// The serving glue's exports, serving `module`.
    pub fn served<M>(module: M) -> Entry
    where
        M: Direct3D11 + DirectXGraphicsInfrastructure<Buffer = <M as Direct3D11>::Texture2D>,
    {
        Entry(exports(module))
    }

    /// The fake through both glues: the serving glue's exports over the calling glue over the fake.
    pub fn pass_through() -> Entry {
        #[expect(unsafe_code, reason = "the fake's exports, which take what the metadata gives")]
        // SAFETY: each entry is the fake's export of that name, declared as d3d11.h declares it.
        let system = unsafe { SystemDirect3D11::new(Entry::fake().0) };
        Entry::served(system)
    }

    /// The calling glue over these exports.
    pub fn system(self) -> SystemDirect3D11 {
        #[expect(unsafe_code, reason = "exports that take what the metadata gives")]
        // SAFETY: every `Entry` is the fake's exports or the serving glue's, both as d3d11.h declares them.
        unsafe {
            SystemDirect3D11::new(self.0)
        }
    }

    /// Plays the game, and returns the first failed check.
    pub fn play(self) -> Result<(), String> {
        let mut report = vec![0; REPORT_BYTES];
        #[expect(unsafe_code, reason = "the game's C function")]
        // SAFETY: the entry behaves as documented; the report is ours for the call.
        let failures = unsafe { game_play(self.0.d_3d11_create_device, report.as_mut_ptr(), report.len()) };
        outcome(failures, &report)
    }

    /// Plays the game's textures, back buffers and output targets, and returns the first failed check.
    pub fn play_textures(self) -> Result<(), String> {
        let mut report = vec![0; REPORT_BYTES];
        #[expect(unsafe_code, reason = "the game's C function")]
        // SAFETY: the entry behaves as documented; the report is ours for the call.
        let failures = unsafe { game_textures(self.0.d_3d11_create_device_and_swap_chain, report.as_mut_ptr(), report.len()) };
        outcome(failures, &report)
    }

    /// Plays the hostile program against this entry, with objects of `foreign` where ours are expected.
    pub fn play_hostile(self, foreign: Entry) -> Result<(), String> {
        let mut report = vec![0; REPORT_BYTES];
        #[expect(unsafe_code, reason = "the game's C function")]
        // SAFETY: both entries behave as documented; the report is ours for the call.
        let failures =
            unsafe { game_hostile(self.0.d_3d11_create_device, foreign.0.d_3d11_create_device, report.as_mut_ptr(), report.len()) };
        outcome(failures, &report)
    }

    /// Returns how many checks failed while threads called one device and one buffer at once.
    pub fn hammer(self, threads: u32, iterations: u32) -> i32 {
        #[expect(unsafe_code, reason = "the game's C function")]
        // SAFETY: the entry behaves as documented, and its device and objects are free threaded.
        unsafe {
            game_hammer(self.0.d_3d11_create_device, threads, iterations)
        }
    }

    /// Returns the nanoseconds the game took for `iterations` of the operation.
    pub fn time(self, operation: Operation, iterations: u64) -> u64 {
        #[expect(unsafe_code, reason = "the game's C function")]
        // SAFETY: the entry behaves as documented.
        unsafe {
            game_time(self.0.d_3d11_create_device, operation as u32, iterations)
        }
    }

    /// Runs one of the C attacks and returns its status and report.
    pub fn attack(self, attack: Attack) -> (i32, String) {
        let mut report = vec![0_u8; REPORT_BYTES];
        let (out, capacity, create) = (report.as_mut_ptr(), report.len(), self.0.d_3d11_create_device);
        #[expect(unsafe_code, reason = "the C attacks")]
        // SAFETY: the entry behaves as documented; the report is ours for the call.
        let status = unsafe {
            match attack {
                Attack::StaleReuse => attack_stale_reuse(create, out, capacity),
                Attack::StaleBound(later) => attack_stale_bound(create, later, out, capacity),
                Attack::ReleaseInsideCall => attack_release_inside_call(create, out, capacity),
                Attack::ReleaseFromOtherThread => attack_release_from_other_thread(create, out, capacity),
                Attack::WriteMapped(width) => attack_write_mapped(create, width, out, capacity),
                Attack::WriteMappedTexture(side) => attack_write_mapped_texture(create, side, out, capacity),
                Attack::OutputMerger => attack_output_merger(create, out, capacity),
                Attack::UndoThreads => attack_undo_threads(create, out, capacity),
            }
        };
        (status, text(&report))
    }

    /// Fills the object table with `count` buffers, releases them, then asks for a query.
    pub fn fill_table(self, count: u32) -> (TableFull, String) {
        let mut report = vec![0_u8; REPORT_BYTES];
        let mut full = TableFull::default();
        #[expect(unsafe_code, reason = "the C attack")]
        // SAFETY: the entry behaves as documented; the counts and the report are ours for the call.
        let ok_but_null = unsafe {
            attack_table_full(
                self.0.d_3d11_create_device,
                count,
                &mut full.made,
                &mut full.refused,
                &mut full.query_made,
                report.as_mut_ptr(),
                report.len(),
            )
        };
        full.ok_but_null = ok_but_null;
        (full, text(&report))
    }

    /// Runs one of check-2's C games and returns its status and report.
    pub fn check2(self, game: Check2) -> (i32, String) {
        let mut report = vec![0_u8; REPORT_BYTES];
        let (out, capacity, create) = (report.as_mut_ptr(), report.len(), self.0.d_3d11_create_device);
        #[expect(unsafe_code, reason = "check-2's C games")]
        // SAFETY: the entry's exports are the serving glue's or the fake's; the report is ours for the call.
        let status = unsafe {
            match game {
                Check2::GetDeviceTwice => check2_get_device_twice(create, out, capacity),
                Check2::HeldTextureAfterDraw => check2_held_texture_after_draw(create, out, capacity),
                Check2::GetTypeThenGetDesc { exact } => check2_get_type_then_get_desc(create, i32::from(exact), out, capacity),
                Check2::ConstInitial => check2_const_initial(create, out, capacity),
                Check2::FillByDepthPitch => check2_fill_by_depth_pitch(create, out, capacity),
                Check2::HugeTexture => check2_huge_texture(create, out, capacity),
                Check2::Callbacks => check2_callbacks(create, out, capacity),
                Check2::WriteAfterRelease => check2_write_after_release(create, out, capacity),
                Check2::MapRace { threads, iterations } => check2_map_race(create, threads, iterations, out, capacity),
            }
        };
        (status, text(&report))
    }

    /// Runs a fuzzing driver against this entry for about `calls` calls on `threads` threads, and returns the calls
    /// made.
    pub fn fuzz(self, driver: Driver, seed: u64, calls: u64, threads: u32) -> u64 {
        let (create, create_swap) = (self.0.d_3d11_create_device, self.0.d_3d11_create_device_and_swap_chain);
        #[expect(unsafe_code, reason = "the fuzzing drivers' C functions")]
        // SAFETY: the entry's exports are the serving glue's or the fake's; the drivers keep the rules they state.
        unsafe {
            match driver {
                Driver::Correct => fuzz_correct(create, create_swap, seed, calls, threads),
                Driver::Hostile => fuzz_hostile(create, create_swap, seed, calls, threads),
            }
        }
    }
}

/// Returns how many calls the fuzzing drivers have made so far, which a watchdog reads.
pub fn fuzz_calls() -> u64 {
    #[expect(unsafe_code, reason = "the fuzzing drivers' C function")]
    // SAFETY: it reads an atomic counter.
    unsafe {
        fuzz_progress()
    }
}

/// Returns what the fuzzing calls achieved: devices, resources and views made, mappings written, query results read,
/// back buffers got, and the kinds of table the hostile game reached (a bit each of 8).
pub fn fuzz_achieved() -> [u64; 7] {
    let mut counts = [0; 7];
    #[expect(unsafe_code, reason = "the fuzzing drivers' C function")]
    // SAFETY: it writes seven counters, ours for the call.
    unsafe {
        fuzz_count(counts.as_mut_ptr());
    }
    counts
}

/// Returns how many answers broke a promise the C driver checks, and the first.
pub fn fuzz_violations() -> (u32, String) {
    #[expect(unsafe_code, reason = "the fuzzing drivers' C function")]
    // SAFETY: it reads an atomic counter.
    let count = unsafe { fuzz_violation_count() };
    #[expect(unsafe_code, reason = "the fuzzing drivers' C function")]
    // SAFETY: a static string of the driver's, terminated, written once before the count rises.
    let first = unsafe { core::ffi::CStr::from_ptr(fuzz_first_violation()) };
    (count, first.to_string_lossy().into_owned())
}

/// Returns the promises broken by the line of fuzz.c that checks each, those with any.
pub fn fuzz_violations_by_line() -> Vec<(usize, u32)> {
    let mut counts = vec![0_u32; 2048];
    #[expect(unsafe_code, reason = "the fuzzing drivers' C function")]
    // SAFETY: it writes as many counters as asked, ours for the call.
    unsafe {
        fuzz_violation_lines(counts.as_mut_ptr(), 2048);
    }
    counts.into_iter().enumerate().filter(|(_, count)| *count > 0).collect()
}

/// Returns how many of the fake's objects are alive.
pub fn live_fake_objects() -> i64 {
    #[expect(unsafe_code, reason = "the fake's C function")]
    // SAFETY: it only reads an atomic counter.
    unsafe {
        fake_live_objects()
    }
}

/// Returns the fake's objects alive by kind: device, context, buffer, query, texture, render target, depth stencil,
/// unordered access view, swap chain.
pub fn live_fake_by_kind() -> [i64; 9] {
    let mut counts = [0; 9];
    #[expect(unsafe_code, reason = "the fake's C function")]
    // SAFETY: it writes nine counters, ours for the call.
    unsafe {
        fake_live_by_kind(counts.as_mut_ptr());
    }
    counts
}

/// Returns what the fake has seen so far.
pub fn observed() -> Observed {
    let mut seen = Observed::default();
    #[expect(unsafe_code, reason = "the fake's C function")]
    // SAFETY: it writes one `Observed`, ours for the call.
    unsafe {
        fake_observed(&mut seen);
    }
    seen
}

const REPORT_BYTES: usize = 512;

fn text(report: &[u8]) -> String {
    let end = report.iter().position(|&byte| byte == 0).unwrap_or(report.len());
    String::from_utf8_lossy(&report[..end]).into_owned()
}

fn outcome(failures: i32, report: &[u8]) -> Result<(), String> {
    if failures == 0 {
        return Ok(());
    }
    Err(format!("{failures} failed; first {}", text(report)))
}
