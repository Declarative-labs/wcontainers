//! Hostile tests of the pins (src/api.rs): a caller tries, through the calling glue, each method of the slice that
//! could undo a fact a mark relies on, and the system (the fake, which records what reached it) must not see it
//! undone. The fake directly is the control: the same C program does undo it there. Run with one test thread: the
//! fake's records are counts for the whole process.

use wcontainers_component_object_model::structures::{Hmodule, Hwnd};
use wcontainers_direct3d_11::enumerations::CreateDeviceFlag;
use wcontainers_direct3d_11::{Direct3D11 as _, Multithread as _};
use wcontainers_direct3d_11_direct::{SystemDirect3D11, SystemMultithread};
use wcontainers_direct3d_11_direct_harness::{Attack, Entry, observed};
use wcontainers_direct3d_common::enumerations::{DriverType, FeatureLevel};
use wcontainers_directx_graphics_infrastructure as dxgi;
use wcontainers_directx_graphics_infrastructure::enumerations::{Format, ModeScaling, ModeScanlineOrder, SwapEffect};
use wcontainers_directx_graphics_infrastructure::structures::{ModeDesc, Rational, SampleDesc, SwapChainDesc};

/// D3D11_CREATE_DEVICE_SINGLETHREADED.
const SINGLE_THREADED: CreateDeviceFlag = CreateDeviceFlag(1);
const LEVELS: [FeatureLevel; 1] = [FeatureLevel(0xb000)];
/// The SDK version the fake accepts (D3D11_SDK_VERSION).
const SDK_VERSION: u32 = 7;

#[test]
fn protection_stays_on_through_the_context_and_the_multithread_object() {
    let system = Entry::fake().system();
    let before = observed();
    let (device, _, context) = system.create_device(None, DriverType(1), Hmodule(0), CreateDeviceFlag(0), &LEVELS, SDK_VERSION).expect("a device");
    assert!(context.set_multithread_protected(false), "protection was on before the first call");
    assert!(context.set_multithread_protected(false), "and still on before the second: the glue did not turn it off");
    let given_up = context.clone();
    let raw = given_up.raw();
    core::mem::forget(given_up);
    #[expect(unsafe_code, reason = "the context's own pointer, a live COM object answering ID3D11Multithread, as an older interface of the kind")]
    // SAFETY: `raw` is the context, live and protected, one of whose references `given_up` held and gave up here.
    let multithread = unsafe { SystemMultithread::<SystemDirect3D11>::from_owned_older(raw) }.expect("the context answers ID3D11Multithread");
    assert!(multithread.set_multithread_protected(false), "the multithread object answers as the context does");
    assert!(multithread.set_multithread_protected(false));
    let after = observed();
    assert_eq!(after.unprotections, before.unprotections, "the system's protection was never turned off");
    drop((multithread, context, device));

    // The same, from a C program through both glues; and the control, the C program on the fake directly.
    let before = observed();
    let (through, report) = Entry::pass_through().attack(Attack::UndoThreads);
    println!("ATTACK UndoThreads pass-through: {through}: {report}");
    assert_eq!(through, 1, "through both glues the protection stays on");
    let after = observed();
    assert_eq!((after.unprotections, after.single_threaded_creates), (before.unprotections, before.single_threaded_creates), "nothing undone reached the system");
    let (direct, report) = Entry::fake().attack(Attack::UndoThreads);
    println!("ATTACK UndoThreads fake directly (the control): {direct}: {report}");
    assert_eq!(direct, 0, "on the fake directly the program does turn protection off");
    assert_eq!(observed().unprotections, after.unprotections + 1, "the fake records an undoing it sees");
}

#[test]
fn a_single_threaded_device_is_asked_free_threaded() {
    let system = Entry::fake().system();
    let before = observed();
    let (device, _, context) = system.create_device(None, DriverType(1), Hmodule(0), SINGLE_THREADED, &LEVELS, SDK_VERSION).expect("a device");
    let seen = observed();
    assert_eq!(seen.create_calls, before.create_calls + 1);
    assert_eq!(seen.single_threaded_creates, before.single_threaded_creates, "D3D11_CREATE_DEVICE_SINGLETHREADED never reaches the system");
    assert_eq!(seen.create_flags & SINGLE_THREADED.0, 0);
    drop((context, device));

    let mode = ModeDesc { width: 8, height: 2, refresh_rate: Rational { numerator: 60, denominator: 1 }, format: Format(28), scanline_ordering: ModeScanlineOrder(0), scaling: ModeScaling(0) };
    let desc = SwapChainDesc { buffer_desc: mode, sample_desc: SampleDesc { count: 1, quality: 0 }, buffer_usage: dxgi::enumerations::Usage(0x20), buffer_count: 1, output_window: Hwnd(0), windowed: true, swap_effect: SwapEffect(0), flags: 0 };
    let made = system.create_device_and_swap_chain(None, DriverType(1), Hmodule(0), SINGLE_THREADED, &LEVELS, SDK_VERSION, Some(&desc)).expect("a swap chain");
    let seen = observed();
    assert_eq!(seen.single_threaded_creates, before.single_threaded_creates, "nor through D3D11CreateDeviceAndSwapChain");
    assert!(seen.protected_contexts >= before.protected_contexts + 2, "both contexts were protected");
    drop(made);
}

#[test]
fn a_module_is_never_handed_to_the_system() {
    let system = Entry::fake().system();
    let before = observed();
    let mode = ModeDesc { width: 8, height: 2, refresh_rate: Rational { numerator: 60, denominator: 1 }, format: Format(28), scanline_ordering: ModeScanlineOrder(0), scaling: ModeScaling(0) };
    let desc = SwapChainDesc { buffer_desc: mode, sample_desc: SampleDesc { count: 1, quality: 0 }, buffer_usage: dxgi::enumerations::Usage(0x20), buffer_count: 1, output_window: Hwnd(0), windowed: true, swap_effect: SwapEffect(0), flags: 0 };
    for module in [Hmodule(1), Hmodule(0x1000), Hmodule(usize::MAX)] {
        assert!(system.create_device(None, DriverType(1), module, CreateDeviceFlag(0), &LEVELS, SDK_VERSION).is_err(), "a module handle is refused");
        assert!(system.create_device_and_swap_chain(None, DriverType(1), module, CreateDeviceFlag(0), &LEVELS, SDK_VERSION, Some(&desc)).is_err(), "through both exports");
    }
    assert_eq!(observed().create_calls, before.create_calls, "no call carrying a module reached the system");
}

#[test]
fn the_debug_layer_is_refused() {
    let system = Entry::fake().system();
    let before = observed();
    /// D3D11_CREATE_DEVICE_DEBUG, and a bit d3d11.h declares for no flag.
    const DEBUG: CreateDeviceFlag = CreateDeviceFlag(2);
    const UNDECLARED: CreateDeviceFlag = CreateDeviceFlag(0x4000_0000);
    for flags in [DEBUG, UNDECLARED, CreateDeviceFlag(DEBUG.0 | SINGLE_THREADED.0)] {
        assert!(system.create_device(None, DriverType(1), Hmodule(0), flags, &LEVELS, SDK_VERSION).is_err(), "refused: {flags:?}");
    }
    assert_eq!(observed().create_calls, before.create_calls, "neither reached the system");
}
