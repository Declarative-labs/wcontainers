//! Hostile tests of the whole APIs' rows (src/api.rs), and thirty safe-Rust probes of the whole-set calling glue.
//!
//! The system is the generated recorder, reached through both glues (`served`: the serving glue's own exports, called
//! through the calling glue) as if it were d3d11.dll and dxgi.dll; every call that reaches it is logged with the values
//! passed by value. Each probe tries, with safe code only, to make the system run code, read or write memory the caller
//! did not lend, or outlive an object, and prints `PROBE <n> <name>: HELD` or `PROBE <n> <name>: THROUGH <what>`. Run
//! with one test thread: the recorders' logs are the process's.

#![forbid(unsafe_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_component_object_model::structures::{Foreign, Guid, Handle, Hmodule, Hwnd};
use wcontainers_direct3d_11 as d3d11;
use wcontainers_direct3d_11::constants::{
    IID_ID3D11_INFO_QUEUE, IID_ID3D11_MULTITHREAD, IID_ID3D11_SWITCH_TO_REF,
    IID_ID3D11_VIDEO_CONTEXT, IID_ID3D11_VIDEO_DECODER,
};
use wcontainers_direct3d_11::enumerations::{
    ContextType, CreateDeviceFlag, Map, MessageCategory, MessageId, MessageSeverity, QueryKind,
    Usage, VideoDecoderBufferType,
};
use wcontainers_direct3d_11::structures::{BufferDesc, QueryDesc1, SubresourceData, Texture2DDesc};
use wcontainers_direct3d_11::{
    AsynchronousRef, Device as _, Device1 as _, Device2 as _, Device3 as _, DeviceChild as _,
    DeviceContext as _, DeviceContext1 as _, Direct3D11 as _, InfoQueue as _, Multithread as _,
    ResourceRef, SwitchToRef as _, UnknownObject, UnknownRef, VideoContext as _,
};
use wcontainers_direct3d_11_direct::{SystemDevice, SystemDeviceContext, SystemDirect3D11, served};
use wcontainers_direct3d_11_recorder::{self as recorder, Recorder};
use wcontainers_direct3d_common::enumerations::{DriverType, FeatureLevel};
use wcontainers_directx_graphics_infrastructure as dxgi;
use wcontainers_directx_graphics_infrastructure::constants::{
    IID_IDXGI_FACTORY, IID_IDXGI_INFO_QUEUE, IID_IDXGI_OUTPUT_DUPLICATION,
};
use wcontainers_directx_graphics_infrastructure::enumerations::{
    CreateFactoryFlags, Format, InfoQueueMessageCategory, InfoQueueMessageSeverity, MapFlags,
    ModeScaling, ModeScanlineOrder, MwaFlags, OfferResourcePriority, SwapEffect,
};
use wcontainers_directx_graphics_infrastructure::structures::{
    ModeDesc, Rational, SampleDesc, SwapChainDesc,
};
use wcontainers_directx_graphics_infrastructure::{
    DirectXGraphicsInfrastructure as _, Factory as _, OutputDuplication as _,
};
use wcontainers_directx_graphics_infrastructure_direct::SystemDirectXGraphicsInfrastructure;
use wcontainers_directx_graphics_infrastructure_recorder as dxgi_recorder;
use wcontainers_volatile_bytes::VolatileBytes;

const LEVELS: [FeatureLevel; 1] = [FeatureLevel(0xb000)];
/// D3D11_SDK_VERSION.
const SDK_VERSION: u32 = 7;
/// D3D11_CREATE_DEVICE_SINGLETHREADED, D3D11_CREATE_DEVICE_DEBUG, and a bit d3d11.h declares for no flag.
const SINGLE_THREADED: CreateDeviceFlag = CreateDeviceFlag(1);
const DEBUG: CreateDeviceFlag = CreateDeviceFlag(2);
const UNDECLARED: CreateDeviceFlag = CreateDeviceFlag(0x4000_0000);
/// An address the system would run as code, were it handed over: a function of this test.
fn code_address() -> usize {
    (code_address as fn() -> usize) as usize
}

fn api() -> SystemDirect3D11 {
    served(Recorder)
}

fn dxgi_api() -> SystemDirectXGraphicsInfrastructure {
    wcontainers_directx_graphics_infrastructure_direct::served(dxgi_recorder::Recorder)
}

fn device() -> (
    SystemDevice<SystemDirect3D11>,
    SystemDeviceContext<SystemDirect3D11>,
) {
    let (device, _, context) = api()
        .create_device(
            None,
            DriverType(1),
            Hmodule(0),
            CreateDeviceFlag(0),
            &LEVELS,
            SDK_VERSION,
        )
        .expect("a device");
    (device, context)
}

fn buffer_desc(byte_width: u32) -> BufferDesc {
    BufferDesc {
        byte_width,
        usage: Usage(0),
        bind_flags: 1,
        cpu_access_flags: 0,
        misc_flags: 0,
        structure_byte_stride: 0,
    }
}

fn swap_chain_desc() -> SwapChainDesc {
    let mode = ModeDesc {
        width: 8,
        height: 2,
        refresh_rate: Rational {
            numerator: 60,
            denominator: 1,
        },
        format: Format(28),
        scanline_ordering: ModeScanlineOrder(0),
        scaling: ModeScaling(0),
    };
    SwapChainDesc {
        buffer_desc: mode,
        sample_desc: SampleDesc {
            count: 1,
            quality: 0,
        },
        buffer_usage: dxgi::enumerations::Usage(0x20),
        buffer_count: 1,
        output_window: Hwnd(0),
        windowed: true,
        swap_effect: SwapEffect(0),
        flags: 0,
    }
}

/// A Direct3D 11 object of the kind `iid` names, which the recorder makes for any interface id.
fn object(device: &SystemDevice<SystemDirect3D11>, iid: &Guid) -> UnknownObject<SystemDirect3D11> {
    d3d11::Device::open_shared_resource(device, Handle(1), iid)
        .expect("the recorder answers any interface id")
}

/// How many calls of `place` reached the Direct3D 11 recorder.
fn reached(place: &str) -> usize {
    recorder::calls(place).len()
}

/// How many calls of `place` reached the DXGI recorder.
fn reached_dxgi(place: &str) -> usize {
    dxgi_recorder::calls(place).len()
}

/// Prints a probe's outcome, and returns whether the glue held.
fn outcome(number: u32, name: &str, through: Option<String>) -> bool {
    match &through {
        None => println!("PROBE {number:02} {name}: HELD"),
        Some(what) => println!("PROBE {number:02} {name}: THROUGH {what}"),
    }
    through.is_none()
}

/// Bytes of ours behind a lock, as a caller's buffer of a fixed length.
struct Bytes(std::sync::Mutex<Vec<u8>>);

impl VolatileBytes for Bytes {
    fn len(&self) -> u64 {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len() as u64
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let held = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = into.len().min(held.len());
        into[..count].copy_from_slice(&held[..count]);
        count
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let mut held = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = bytes.len().min(held.len());
        held[..count].copy_from_slice(&bytes[..count]);
        count
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let copy = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        sink.write(&copy)
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut copy = vec![
            0_u8;
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len()
        ];
        let count = source.read(&mut copy)?;
        Ok(self.write(&copy[..count.min(copy.len())]))
    }
}

/// Bytes that lie about their length, and count every byte the glue reads or writes through them.
struct Lying {
    length: u64,
    touched: AtomicUsize,
}

impl VolatileBytes for Lying {
    fn len(&self) -> u64 {
        self.length
    }

    fn read(&self, into: &mut [u8]) -> usize {
        self.touched.fetch_add(into.len(), Ordering::SeqCst);
        0
    }

    fn write(&self, bytes: &[u8]) -> usize {
        self.touched.fetch_add(bytes.len(), Ordering::SeqCst);
        0
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        sink.write(&[])
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        source.read(&mut [])
    }
}

// ---- The rows' hostile tests (src/kinds.rs checks each is here).

#[test]
fn protection_stays_on_through_the_context_and_the_multithread_object() {
    let (device, context) = device();
    let protected = recorder::calls("ID3D11Multithread::SetMultithreadProtected");
    assert!(
        !protected.is_empty() && protected.iter().all(|call| call.contains("true")),
        "the glue turned protection on when the context crossed: {protected:?}"
    );
    assert!(
        context.set_multithread_protected(false),
        "protection was on before the call"
    );
    let Ok(UnknownObject::Multithread(multithread)) =
        d3d11::Device::open_shared_resource(&device, Handle(1), &IID_ID3D11_MULTITHREAD)
    else {
        panic!("a multithread object")
    };
    assert!(multithread.set_multithread_protected(false));
    assert!(
        recorder::calls("ID3D11Multithread::SetMultithreadProtected")
            .iter()
            .all(|call| !call.contains("false")),
        "no FALSE ever reached the system"
    );
}

#[test]
fn the_device_lock_is_never_taken_or_left() {
    let (device, _context) = device();
    let Ok(UnknownObject::Multithread(multithread)) =
        d3d11::Device::open_shared_resource(&device, Handle(1), &IID_ID3D11_MULTITHREAD)
    else {
        panic!("a multithread object")
    };
    multithread.leave();
    multithread.enter();
    assert_eq!(
        (
            reached("ID3D11Multithread::Enter"),
            reached("ID3D11Multithread::Leave")
        ),
        (0, 0)
    );
}

#[test]
fn deferred_contexts_are_never_made() {
    let (device, _context) = device();
    assert!(device.create_deferred_context(0).is_err());
    assert!(device.create_deferred_context1(0).is_err());
    assert!(device.create_deferred_context2(0).is_err());
    assert!(device.create_deferred_context3(0).is_err());
    assert_eq!(
        reached("ID3D11Device::CreateDeferredContext")
            + reached("ID3D11Device1::CreateDeferredContext1")
            + reached("ID3D11Device2::CreateDeferredContext2")
            + reached("ID3D11Device3::CreateDeferredContext3"),
        0
    );
}

#[test]
fn a_context_state_is_asked_free_threaded() {
    let (device, _context) = device();
    let before = reached("ID3D11Device1::CreateDeviceContextState");
    let _ = device.create_device_context_state(
        1,
        &LEVELS,
        SDK_VERSION,
        &d3d11::constants::IID_ID3D11_DEVICE,
    );
    let calls = recorder::calls("ID3D11Device1::CreateDeviceContextState");
    assert_eq!(calls.len(), before + 1);
    assert!(
        calls.iter().all(|call| call.contains("flags=0,")),
        "D3D11_1_CREATE_DEVICE_CONTEXT_STATE_SINGLETHREADED never reaches the system: {calls:?}"
    );
}

#[test]
fn a_single_threaded_device_is_asked_free_threaded() {
    let _ = api()
        .create_device(
            None,
            DriverType(1),
            Hmodule(0),
            SINGLE_THREADED,
            &LEVELS,
            SDK_VERSION,
        )
        .expect("a device");
    let _ = api().create_device_and_swap_chain(
        None,
        DriverType(1),
        Hmodule(0),
        SINGLE_THREADED,
        &LEVELS,
        SDK_VERSION,
        Some(&swap_chain_desc()),
    );
    let calls: Vec<String> = recorder::calls("D3D11CreateDevice")
        .into_iter()
        .chain(recorder::calls("D3D11CreateDeviceAndSwapChain"))
        .collect();
    assert!(!calls.is_empty());
    assert!(
        calls
            .iter()
            .all(|call| !call.contains("CreateDeviceFlag(1)")
                && !call.contains("CreateDeviceFlag(3)")),
        "the flag never reaches the system: {calls:?}"
    );
}

#[test]
fn the_debug_layer_is_refused() {
    let before = (
        reached("D3D11CreateDevice"),
        reached("D3D11CreateDeviceAndSwapChain"),
        reached_dxgi("CreateDXGIFactory2"),
    );
    for flags in [
        DEBUG,
        UNDECLARED,
        CreateDeviceFlag(DEBUG.0 | SINGLE_THREADED.0),
    ] {
        assert!(
            api()
                .create_device(None, DriverType(1), Hmodule(0), flags, &LEVELS, SDK_VERSION)
                .is_err(),
            "{flags:?}"
        );
        assert!(
            api()
                .create_device_and_swap_chain(
                    None,
                    DriverType(1),
                    Hmodule(0),
                    flags,
                    &LEVELS,
                    SDK_VERSION,
                    Some(&swap_chain_desc())
                )
                .is_err(),
            "{flags:?}"
        );
    }
    assert!(
        dxgi_api()
            .create_dxgi_factory2(CreateFactoryFlags(1), &IID_IDXGI_FACTORY)
            .is_err(),
        "DXGI_CREATE_FACTORY_DEBUG"
    );
    assert!(
        dxgi_api()
            .create_dxgi_factory2(CreateFactoryFlags(2), &IID_IDXGI_FACTORY)
            .is_err(),
        "an undeclared bit"
    );
    assert_eq!(
        (
            reached("D3D11CreateDevice"),
            reached("D3D11CreateDeviceAndSwapChain"),
            reached_dxgi("CreateDXGIFactory2")
        ),
        before,
        "none reached the system"
    );
}

#[test]
fn a_module_is_never_handed_to_the_system() {
    let before = (
        reached("D3D11CreateDevice"),
        reached("D3D11CreateDeviceAndSwapChain"),
    );
    for module in [Hmodule(1), Hmodule(code_address()), Hmodule(usize::MAX)] {
        assert!(
            api()
                .create_device(
                    None,
                    DriverType(4),
                    module,
                    CreateDeviceFlag(0),
                    &LEVELS,
                    SDK_VERSION
                )
                .is_err()
        );
        assert!(
            api()
                .create_device_and_swap_chain(
                    None,
                    DriverType(4),
                    module,
                    CreateDeviceFlag(0),
                    &LEVELS,
                    SDK_VERSION,
                    Some(&swap_chain_desc())
                )
                .is_err()
        );
    }
    assert_eq!(
        (
            reached("D3D11CreateDevice"),
            reached("D3D11CreateDeviceAndSwapChain")
        ),
        before
    );
}

#[test]
fn exceptions_are_never_turned_on() {
    let (device, _context) = device();
    assert!(device.set_exception_mode(1).is_err());
    assert!(device.set_exception_mode(u32::MAX).is_err());
    assert!(device.set_exception_mode(0).is_ok());
    assert!(
        recorder::calls("ID3D11Device::SetExceptionMode")
            .iter()
            .all(|call| call.contains("raise_flags=0)"))
    );
}

#[test]
fn breaks_are_never_turned_on() {
    let (device, _context) = device();
    let Ok(UnknownObject::InfoQueue(queue)) =
        d3d11::Device::open_shared_resource(&device, Handle(1), &IID_ID3D11_INFO_QUEUE)
    else {
        panic!("an info queue")
    };
    assert!(
        queue
            .set_break_on_severity(MessageSeverity(0), true)
            .is_err()
    );
    assert!(
        queue
            .set_break_on_category(MessageCategory(0), true)
            .is_err()
    );
    assert!(queue.set_break_on_id(MessageId(1), true).is_err());
    assert!(
        queue
            .set_break_on_severity(MessageSeverity(0), false)
            .is_ok()
    );
    for place in [
        "ID3D11InfoQueue::SetBreakOnSeverity",
        "ID3D11InfoQueue::SetBreakOnCategory",
        "ID3D11InfoQueue::SetBreakOnID",
    ] {
        assert!(
            recorder::calls(place)
                .iter()
                .all(|call| call.contains("b_enable=false")),
            "{place}"
        );
    }
    let Ok(dxgi::UnknownObject::InfoQueue(dxgi_queue)) =
        dxgi_api().dxgi_get_debug_interface1(0, &IID_IDXGI_INFO_QUEUE)
    else {
        panic!("DXGI's info queue")
    };
    assert!(
        dxgi::InfoQueue::set_break_on_severity(
            &dxgi_queue,
            Guid::default(),
            InfoQueueMessageSeverity(0),
            true
        )
        .is_err()
    );
    assert!(
        dxgi::InfoQueue::set_break_on_category(
            &dxgi_queue,
            Guid::default(),
            InfoQueueMessageCategory(0),
            true
        )
        .is_err()
    );
    assert!(dxgi::InfoQueue::set_break_on_id(&dxgi_queue, Guid::default(), 1, true).is_err());
    assert_eq!(
        reached_dxgi("IDXGIInfoQueue::SetBreakOnSeverity")
            + reached_dxgi("IDXGIInfoQueue::SetBreakOnCategory")
            + reached_dxgi("IDXGIInfoQueue::SetBreakOnID"),
        0
    );
}

#[test]
fn a_discarding_copy_is_refused() {
    let (device, context) = device();
    let buffer = device
        .create_buffer(&buffer_desc(256), None)
        .expect("a buffer");
    let before = reached("ID3D11DeviceContext1::CopySubresourceRegion1");
    context.copy_subresource_region1(
        ResourceRef::Buffer(&buffer),
        0,
        0,
        0,
        0,
        ResourceRef::Buffer(&buffer),
        0,
        None,
        2,
    );
    context.copy_subresource_region1(
        ResourceRef::Buffer(&buffer),
        0,
        0,
        0,
        0,
        ResourceRef::Buffer(&buffer),
        0,
        None,
        3,
    );
    assert_eq!(
        reached("ID3D11DeviceContext1::CopySubresourceRegion1"),
        before,
        "D3D11_COPY_DISCARD never reaches the system"
    );
    context.copy_subresource_region1(
        ResourceRef::Buffer(&buffer),
        0,
        0,
        0,
        0,
        ResourceRef::Buffer(&buffer),
        0,
        None,
        1,
    );
    assert_eq!(
        reached("ID3D11DeviceContext1::CopySubresourceRegion1"),
        before + 1,
        "a copy without it does"
    );
}

#[test]
fn resources_are_never_discarded() {
    let (device, context) = device();
    let buffer = device
        .create_buffer(&buffer_desc(256), None)
        .expect("a buffer");
    context.discard_resource(ResourceRef::Buffer(&buffer));
    assert_eq!(reached("ID3D11DeviceContext1::DiscardResource"), 0);
}

#[test]
fn offered_resources_are_never_offered() {
    let (device, _context) = device();
    assert!(dxgi::Device2::offer_resources(&device, &[], OfferResourcePriority(1)).is_err());
    assert_eq!(
        reached_dxgi("IDXGIDevice2::OfferResources") + reached("IDXGIDevice2::OfferResources"),
        0
    );
}

// ---- The probes.

#[test]
fn probe_01_a_module_through_a_software_adapter() {
    let Ok(dxgi::UnknownObject::Factory(factory)) =
        dxgi_api().create_dxgi_factory(&IID_IDXGI_FACTORY)
    else {
        panic!("a factory through both glues")
    };
    let refused = [
        Hmodule(code_address()),
        Hmodule(0x7ff0_0000_0000),
        Hmodule(usize::MAX),
    ]
    .iter()
    .all(|module| factory.create_software_adapter(*module).is_err());
    let reached = reached_dxgi("IDXGIFactory::CreateSoftwareAdapter");
    assert!(outcome(
        1,
        "CreateSoftwareAdapter with a module",
        (!refused || reached > 0).then(|| format!("{reached} calls reached the system"))
    ));
}

#[test]
fn probe_02_a_module_through_create_device() {
    let before = reached("D3D11CreateDevice");
    let refused = api()
        .create_device(
            None,
            DriverType(4),
            Hmodule(code_address()),
            CreateDeviceFlag(0),
            &LEVELS,
            SDK_VERSION,
        )
        .is_err();
    let through = (!refused || reached("D3D11CreateDevice") > before)
        .then(|| "D3D11CreateDevice with a module".to_string());
    assert!(outcome(2, "D3D11CreateDevice with a module", through));
}

#[test]
fn probe_03_a_module_through_create_device_and_swap_chain() {
    let before = reached("D3D11CreateDeviceAndSwapChain");
    let refused = api()
        .create_device_and_swap_chain(
            None,
            DriverType(4),
            Hmodule(code_address()),
            CreateDeviceFlag(0),
            &LEVELS,
            SDK_VERSION,
            Some(&swap_chain_desc()),
        )
        .is_err();
    let through = (!refused || reached("D3D11CreateDeviceAndSwapChain") > before)
        .then(|| "D3D11CreateDeviceAndSwapChain with a module".to_string());
    assert!(outcome(
        3,
        "D3D11CreateDeviceAndSwapChain with a module",
        through
    ));
}

#[test]
fn probe_04_a_forged_iunknown() {
    let (device, _context) = device();
    // A `Foreign` built by safe code around a number: no object of the system's, whose table the system would call.
    let forged = Foreign::new(code_address(), Arc::new(code_address()));
    let before = reached("ID3D11Device::SetPrivateDataInterface");
    let refused = d3d11::Device::set_private_data_interface(
        &device,
        &Guid::default(),
        Some(UnknownRef::Unknown(&forged)),
    )
    .is_err();
    let through = (!refused || reached("ID3D11Device::SetPrivateDataInterface") > before)
        .then(|| "a forged IUnknown".to_string());
    assert!(outcome(
        4,
        "a forged IUnknown handed to the system",
        through
    ));
}

#[test]
fn probe_05_exceptions_raised_through_our_frames() {
    let (device, _context) = device();
    let refused =
        device.set_exception_mode(1).is_err() && device.set_exception_mode(0x8000_0000).is_err();
    let through = recorder::calls("ID3D11Device::SetExceptionMode")
        .into_iter()
        .find(|call| !call.contains("raise_flags=0)"));
    assert!(outcome(
        5,
        "SetExceptionMode nonzero",
        through.or_else(|| (!refused).then(|| "accepted".to_string()))
    ));
}

#[test]
fn probe_06_a_break_in_direct3d_11s_info_queue() {
    let (device, _context) = device();
    let UnknownObject::InfoQueue(queue) = object(&device, &IID_ID3D11_INFO_QUEUE) else {
        panic!("an info queue")
    };
    let refused = queue
        .set_break_on_severity(MessageSeverity(1), true)
        .is_err();
    let through = recorder::calls("ID3D11InfoQueue::SetBreakOnSeverity")
        .into_iter()
        .find(|call| call.contains("b_enable=true"));
    assert!(outcome(
        6,
        "ID3D11InfoQueue::SetBreakOnSeverity TRUE",
        through.or_else(|| (!refused).then(|| "accepted".to_string()))
    ));
}

#[test]
fn probe_07_a_break_in_dxgis_info_queue() {
    let Ok(dxgi::UnknownObject::InfoQueue(queue)) =
        dxgi_api().dxgi_get_debug_interface1(0, &IID_IDXGI_INFO_QUEUE)
    else {
        panic!("DXGI's info queue")
    };
    let refused = dxgi::InfoQueue::set_break_on_severity(
        &queue,
        Guid::default(),
        InfoQueueMessageSeverity(1),
        true,
    )
    .is_err();
    let reached = reached_dxgi("IDXGIInfoQueue::SetBreakOnSeverity");
    assert!(outcome(
        7,
        "IDXGIInfoQueue::SetBreakOnSeverity TRUE",
        (!refused || reached > 0).then(|| format!("{reached} calls"))
    ));
}

#[test]
fn probe_08_the_debug_layer() {
    let before = reached("D3D11CreateDevice");
    let refused = api()
        .create_device(None, DriverType(1), Hmodule(0), DEBUG, &LEVELS, SDK_VERSION)
        .is_err();
    let through = (!refused || reached("D3D11CreateDevice") > before)
        .then(|| "D3D11_CREATE_DEVICE_DEBUG".to_string());
    assert!(outcome(
        8,
        "D3D11CreateDevice with the debug layer",
        through
    ));
}

#[test]
fn probe_09_undeclared_device_flags() {
    let before = reached("D3D11CreateDevice");
    let refused = [
        UNDECLARED,
        CreateDeviceFlag(0x1000),
        CreateDeviceFlag(u32::MAX),
    ]
    .iter()
    .all(|flags| {
        api()
            .create_device(
                None,
                DriverType(1),
                Hmodule(0),
                *flags,
                &LEVELS,
                SDK_VERSION,
            )
            .is_err()
    });
    let through = (!refused || reached("D3D11CreateDevice") > before)
        .then(|| "an undeclared flag".to_string());
    assert!(outcome(
        9,
        "D3D11CreateDevice with undeclared flags",
        through
    ));
}

#[test]
fn probe_10_dxgis_debug_layer() {
    let before = reached_dxgi("CreateDXGIFactory2");
    let refused = dxgi_api()
        .create_dxgi_factory2(CreateFactoryFlags(1), &IID_IDXGI_FACTORY)
        .is_err();
    let through = (!refused || reached_dxgi("CreateDXGIFactory2") > before)
        .then(|| "DXGI_CREATE_FACTORY_DEBUG".to_string());
    assert!(outcome(
        10,
        "CreateDXGIFactory2 with the debug layer",
        through
    ));
}

#[test]
fn probe_11_protection_turned_off() {
    let (_device, context) = device();
    let answered =
        context.set_multithread_protected(false) && context.set_multithread_protected(false);
    let through = recorder::calls("ID3D11Multithread::SetMultithreadProtected")
        .into_iter()
        .find(|call| call.contains("false"));
    assert!(outcome(
        11,
        "SetMultithreadProtected FALSE on the context",
        through.or_else(|| (!answered).then(|| "protection reported off".to_string()))
    ));
}

#[test]
fn probe_12_the_device_lock_left_unbalanced() {
    let (device, _context) = device();
    let UnknownObject::Multithread(multithread) = object(&device, &IID_ID3D11_MULTITHREAD) else {
        panic!("a multithread object")
    };
    multithread.leave();
    multithread.leave();
    let reached = reached("ID3D11Multithread::Leave") + reached("ID3D11Multithread::Enter");
    assert!(outcome(
        12,
        "Leave without Enter",
        (reached > 0).then(|| format!("{reached} calls"))
    ));
}

#[test]
fn probe_13_a_deferred_context() {
    let (device, _context) = device();
    let refused = device.create_deferred_context(0).is_err();
    let reached = reached("ID3D11Device::CreateDeferredContext");
    assert!(outcome(
        13,
        "CreateDeferredContext (not free threaded, typed Sync)",
        (!refused || reached > 0).then(|| format!("{reached} calls"))
    ));
}

#[test]
fn probe_14_the_reference_rasterizer_switched_in() {
    let (device, _context) = device();
    let UnknownObject::SwitchToRef(switch) = object(&device, &IID_ID3D11_SWITCH_TO_REF) else {
        panic!("a switch")
    };
    let _ = switch.set_use_ref(true);
    let reached = reached("ID3D11SwitchToRef::SetUseRef");
    assert!(outcome(
        14,
        "SetUseRef TRUE",
        (reached > 0).then(|| format!("{reached} calls"))
    ));
}

#[test]
fn probe_15_a_window_handle_of_anyones() {
    // A window handle is a kind the kernel checks (src/api.rs HANDLES): it passes by rule, and this probe shows it does.
    let Ok(dxgi::UnknownObject::Factory(factory)) =
        dxgi_api().create_dxgi_factory(&IID_IDXGI_FACTORY)
    else {
        panic!("a factory")
    };
    let _ = factory.make_window_association(Hwnd(0xdead_beef), MwaFlags(0));
    let calls = dxgi_recorder::calls("IDXGIFactory::MakeWindowAssociation");
    let through = calls
        .iter()
        .find(|call| call.contains("0xdeadbeef") || call.contains("3735928559"))
        .cloned();
    assert!(
        through.is_some(),
        "a kernel-checked handle passes by its rule: {calls:?}"
    );
    outcome(
        15,
        "MakeWindowAssociation with any HWND (kernel-checked: passes by rule)",
        through,
    );
}

#[test]
fn probe_16_a_shared_handle_of_anyones() {
    // A HANDLE is a kind the kernel checks: it passes by rule, and this probe shows it does.
    let (device, _context) = device();
    let before = reached("ID3D11Device::OpenSharedResource");
    let _ = d3d11::Device::open_shared_resource(
        &device,
        Handle(0xbeef),
        &d3d11::constants::IID_ID3D11_BUFFER,
    );
    let through = (reached("ID3D11Device::OpenSharedResource") > before)
        .then(|| "OpenSharedResource(0xbeef)".to_string());
    assert!(through.is_some());
    outcome(
        16,
        "OpenSharedResource with any HANDLE (kernel-checked: passes by rule)",
        through,
    );
}

#[test]
fn probe_17_cpu_writes_through_a_raw_pointer() {
    let (device, _context) = device();
    let buffer = device
        .create_buffer(&buffer_desc(256), None)
        .expect("a buffer");
    let lying = Lying {
        length: 1 << 40,
        touched: AtomicUsize::new(0),
    };
    device.write_to_subresource(ResourceRef::Buffer(&buffer), 0, None, &lying, 0, 0);
    device.read_from_subresource(&lying, 0, 0, ResourceRef::Buffer(&buffer), 0, None);
    let reached = reached("ID3D11Device3::WriteToSubresource")
        + reached("ID3D11Device3::ReadFromSubresource");
    let touched = lying.touched.load(Ordering::SeqCst);
    assert!(outcome(
        17,
        "WriteToSubresource and ReadFromSubresource (raw pointers)",
        (reached > 0 || touched > 0).then(|| format!("{reached} calls, {touched} bytes"))
    ));
}

#[test]
fn probe_18_update_subresource_reads_a_box_of_bytes() {
    let (device, context) = device();
    let buffer = device
        .create_buffer(&buffer_desc(256), None)
        .expect("a buffer");
    // The whole buffer is 256 bytes: four are refused before the system sees them, whatever the pitches say; so is a box
    // past the buffer's end.
    let short = vec![0_u8; 4];
    let before = reached("ID3D11DeviceContext::UpdateSubresource");
    context.update_subresource(
        ResourceRef::Buffer(&buffer),
        0,
        None,
        &short,
        1 << 20,
        1 << 30,
    );
    let past = d3d11::structures::Box {
        left: 0,
        top: 0,
        front: 0,
        right: 257,
        bottom: 1,
        back: 1,
    };
    let long = vec![0_u8; 1024];
    context.update_subresource(ResourceRef::Buffer(&buffer), 0, Some(&past), &long, 0, 0);
    let refused = reached("ID3D11DeviceContext::UpdateSubresource") - before;
    // Exactly the buffer's bytes, each slice a heap allocation of that length, pass (the serving glue copies no more).
    let whole = vec![7_u8; 256];
    context.update_subresource(ResourceRef::Buffer(&buffer), 0, None, &whole, 0, 0);
    let region = d3d11::structures::Box {
        left: 16,
        top: 0,
        front: 0,
        right: 48,
        bottom: 1,
        back: 1,
    };
    let part = vec![9_u8; 32];
    context.update_subresource(ResourceRef::Buffer(&buffer), 0, Some(&region), &part, 0, 0);
    let passed = reached("ID3D11DeviceContext::UpdateSubresource") - before - refused;
    assert_eq!(
        passed, 2,
        "the exact bytes of the whole buffer and of a box reach the implementation"
    );
    assert!(outcome(
        18,
        "UpdateSubresource with 4 bytes and huge pitches, and a box past the end",
        (refused > 0).then(|| format!("{refused} calls"))
    ));
}

#[test]
fn probe_31_create_query_hands_out_a_query_or_a_predicate() {
    let (device, context) = device();
    // The recorder makes the least derived kind, a query, for any description; the calling glue asks the object which
    // kind it is (a query answers no ID3D11Predicate).
    let made = device
        .create_query(&d3d11::structures::QueryDesc {
            query: QueryKind(5),
            misc_flags: 0,
        })
        .expect("an occlusion predicate's object");
    let query = match made {
        d3d11::QueryObject::Query(query) => query,
        d3d11::QueryObject::Predicate(_) => panic!("the recorder's query crossed as a predicate"),
    };
    let data = Bytes(std::sync::Mutex::new(vec![0x5a; 4]));
    let ready = context.get_data(AsynchronousRef::Query(&query), Some(&data), 0);
    assert_eq!(
        ready,
        Ok(true),
        "the recorder's data is ready (S_OK through both glues)"
    );
    assert!(outcome(
        31,
        "CreateQuery: the object crosses as the kind it answers",
        None
    ));
}

#[test]
fn probe_32_input_elements_carry_their_semantic_names() {
    let (device, _context) = device();
    let element = |name: &[u8]| d3d11::structures::InputElementDesc {
        semantic_name: std::ffi::CString::new(name).expect("no zero byte"),
        semantic_index: 0,
        format: dxgi::enumerations::Format(2),
        input_slot: 0,
        aligned_byte_offset: 0,
        input_slot_class: d3d11::enumerations::InputClassification(0),
        instance_data_step_rate: 0,
    };
    let before = reached("ID3D11Device::CreateInputLayout");
    let made =
        device.create_input_layout(&[element(b"POSITION"), element(b"TEXCOORD")], &[0_u8; 16]);
    let short = reached("ID3D11Device::CreateInputLayout") - before;
    assert!(
        made.is_ok() && short == 1,
        "two elements with short names reach the implementation once"
    );
    // A name past the serving glue's limit (65,536 bytes) is refused before the implementation sees it.
    let long = vec![b'A'; 70_000];
    let refused = device
        .create_input_layout(&[element(&long)], &[0_u8; 16])
        .is_err();
    let through = reached("ID3D11Device::CreateInputLayout") - before - short;
    assert!(outcome(
        32,
        "CreateInputLayout with a semantic name of 70,000 bytes",
        (!refused || through > 0).then(|| format!("{through} calls"))
    ));
}

#[test]
fn probe_19_get_data_into_bytes_that_lie() {
    let (device, context) = device();
    // CreateQuery1 hands out a query of the newest interface.
    let query = device
        .create_query1(&QueryDesc1 {
            query: QueryKind(0),
            misc_flags: 0,
            context_type: ContextType(0),
        })
        .expect("a query");
    let lying = Lying {
        length: u64::MAX,
        touched: AtomicUsize::new(0),
    };
    let before = reached("ID3D11DeviceContext::GetData");
    let refused = context
        .get_data(AsynchronousRef::Query(&query), Some(&lying), 0)
        .is_err();
    let reached = reached("ID3D11DeviceContext::GetData") - before;
    let touched = lying.touched.load(Ordering::SeqCst);
    assert!(outcome(
        19,
        "GetData into bytes of length u64::MAX",
        (!refused || reached > 0 || touched > 0)
            .then(|| format!("{reached} calls, {touched} bytes"))
    ));
}

#[test]
fn probe_20_vertex_buffers_of_mismatched_counts() {
    let (device, context) = device();
    let buffer = device
        .create_buffer(&buffer_desc(256), None)
        .expect("a buffer");
    let before = reached("ID3D11DeviceContext::IASetVertexBuffers");
    context.ia_set_vertex_buffers(
        0,
        &[Some(&buffer), Some(&buffer), Some(&buffer)],
        &[16],
        &[0, 0, 0],
    );
    let reached = reached("ID3D11DeviceContext::IASetVertexBuffers") - before;
    assert!(outcome(
        20,
        "IASetVertexBuffers: three buffers, one stride",
        (reached > 0).then(|| format!("{reached} calls"))
    ));
}

#[test]
fn probe_21_vertex_buffers_past_the_last_slot() {
    let (device, context) = device();
    let buffer = device
        .create_buffer(&buffer_desc(256), None)
        .expect("a buffer");
    let before = reached("ID3D11DeviceContext::IASetVertexBuffers");
    let buffers = [Some(&buffer); 5];
    context.ia_set_vertex_buffers(30, &buffers, &[16; 5], &[0; 5]);
    context.ia_set_vertex_buffers(u32::MAX, &buffers[..1], &[16], &[0]);
    let reached = reached("ID3D11DeviceContext::IASetVertexBuffers") - before;
    assert!(outcome(
        21,
        "IASetVertexBuffers past slot 32 and at u32::MAX",
        (reached > 0).then(|| format!("{reached} calls"))
    ));
}

#[test]
fn probe_22_unordered_access_views_wrapping_around() {
    let (_device, context) = device();
    let before = reached("ID3D11DeviceContext::OMSetRenderTargetsAndUnorderedAccessViews");
    context.om_set_render_targets_and_unordered_access_views(
        None,
        None,
        u32::MAX - 1,
        Some(&[None, None, None]),
        Some(&[0, 0, 0]),
    );
    let reached =
        reached("ID3D11DeviceContext::OMSetRenderTargetsAndUnorderedAccessViews") - before;
    assert!(outcome(
        22,
        "OMSetRenderTargetsAndUnorderedAccessViews with a start that wraps",
        (reached > 0).then(|| format!("{reached} calls"))
    ));
}

#[test]
fn probe_23_initial_data_shorter_than_the_buffer() {
    let (device, _context) = device();
    let before = reached("ID3D11Device::CreateBuffer");
    let short = SubresourceData {
        sys_mem: vec![7; 16],
        sys_mem_pitch: 0,
        sys_mem_slice_pitch: 0,
    };
    let refused = device
        .create_buffer(&buffer_desc(4096), Some(&short))
        .is_err();
    let reached = reached("ID3D11Device::CreateBuffer") - before;
    assert!(outcome(
        23,
        "CreateBuffer of 4096 bytes with 16 bytes of initial data",
        (!refused || reached > 0).then(|| format!("{reached} calls"))
    ));
}

#[test]
fn probe_24_a_buffer_past_the_largest_resource() {
    let (device, _context) = device();
    let before = reached("ID3D11Device::CreateBuffer");
    let refused = device.create_buffer(&buffer_desc(u32::MAX), None).is_err();
    let reached = reached("ID3D11Device::CreateBuffer") - before;
    assert!(outcome(
        24,
        "CreateBuffer of u32::MAX bytes",
        (!refused || reached > 0).then(|| format!("{reached} calls"))
    ));
}

#[test]
fn probe_25_map_with_an_undeclared_kind() {
    let (device, context) = device();
    let buffer = device
        .create_buffer(&buffer_desc(256), None)
        .expect("a buffer");
    let before = reached("ID3D11DeviceContext::Map");
    let refused = context
        .map(ResourceRef::Buffer(&buffer), 0, Map(9), 0)
        .is_err()
        && context
            .map(ResourceRef::Buffer(&buffer), 0, Map(0), 0)
            .is_err();
    let reached = reached("ID3D11DeviceContext::Map") - before;
    assert!(outcome(
        25,
        "Map with D3D11_MAP 9 and 0",
        (!refused || reached > 0).then(|| format!("{reached} calls"))
    ));
}

#[test]
fn probe_26_one_subresource_mapped_twice() {
    let (device, context) = device();
    let buffer = device
        .create_buffer(&buffer_desc(256), None)
        .expect("a buffer");
    let first = context
        .map(ResourceRef::Buffer(&buffer), 0, Map(3), 0)
        .expect("the first Map");
    let before = reached("ID3D11DeviceContext::Map");
    let second = context.map(ResourceRef::Buffer(&buffer), 0, Map(3), 0);
    let reached = reached("ID3D11DeviceContext::Map") - before;
    let through = (second.is_ok() || reached > 0).then(|| format!("a second Map: {reached} calls"));
    context.unmap(ResourceRef::Buffer(&buffer), 0);
    drop(first);
    assert!(outcome(26, "the same subresource mapped twice", through));
}

#[test]
fn probe_27_lent_memory_after_its_unmap() {
    let (device, context) = device();
    let buffer = device
        .create_buffer(&buffer_desc(256), None)
        .expect("a buffer");
    let mapped = context
        .map(ResourceRef::Buffer(&buffer), 0, Map(3), 0)
        .expect("a Map");
    // The description's 256 bytes raised to the pitches the system gave (lengths.rs `at_least_pitches`), which the
    // serving glue's shadow holds whole.
    let lent = usize::try_from(mapped.data.len()).expect("a length");
    assert!(lent >= 256, "at least the description's bytes");
    assert_eq!(
        mapped.data.write(&vec![5; lent + 44]),
        lent,
        "no more than lent"
    );
    context.unmap(ResourceRef::Buffer(&buffer), 0);
    let mut read = [0_u8; 256];
    let after = (
        mapped.data.read(&mut read),
        mapped.data.write(&[9; 256]),
        mapped.data.len(),
    );
    let through = (after != (0, 0, 0)).then(|| {
        format!(
            "after Unmap: read {}, wrote {}, length {}",
            after.0, after.1, after.2
        )
    });
    assert!(outcome(
        27,
        "lent memory read and written after its Unmap",
        through
    ));
}

#[test]
fn probe_28_lent_memory_outliving_every_object() {
    let (device, context) = device();
    let buffer = device
        .create_buffer(&buffer_desc(256), None)
        .expect("a buffer");
    let mapped = context
        .map(ResourceRef::Buffer(&buffer), 0, Map(3), 0)
        .expect("a Map");
    let live = recorder::live();
    drop((buffer, context, device));
    // The view holds the context and the resource: the memory stays the system's, and lent, while it lives.
    let written = mapped.data.write(&[3; 256]);
    let mut read = [0_u8; 256];
    let read_count = mapped.data.read(&mut read);
    let still = recorder::live();
    println!(
        "PROBE 28 objects alive {live} before the drops, {still} after (the view holds the context and the buffer)"
    );
    let through = (written != 256 || read_count != 256 || read != [3; 256] || still + 1 < live)
        .then(|| format!("wrote {written}, read {read_count}, objects alive {live} then {still}"));
    assert!(outcome(
        28,
        "lent memory used after the buffer, context and device are dropped",
        through
    ));
}

#[test]
fn probe_29_a_child_outliving_its_device() {
    let (device, context) = device();
    let buffer = device
        .create_buffer(&buffer_desc(256), None)
        .expect("a buffer");
    drop((context, device));
    let again = buffer.get_device();
    let made = again
        .as_ref()
        .map(|device| device.create_buffer(&buffer_desc(64), None).is_ok());
    let through = (made != Some(true)).then(|| format!("the device handed back: {:?}", made));
    assert!(outcome(
        29,
        "a buffer's device used after every other reference dropped",
        through
    ));
}

#[test]
fn probe_30_objects_cloned_and_dropped_across_threads() {
    let base = recorder::live();
    {
        let (device, context) = device();
        let buffer = device
            .create_buffer(&buffer_desc(256), None)
            .expect("a buffer");
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let (device, context, buffer) = (device.clone(), context.clone(), buffer.clone());
                scope.spawn(move || {
                    for _ in 0..2000 {
                        let copies = (
                            device.clone(),
                            context.clone(),
                            buffer.clone(),
                            buffer.get_device(),
                        );
                        drop(copies);
                    }
                });
            }
        });
    }
    let after = recorder::live();
    let through = (after != base).then(|| format!("objects alive {base} before, {after} after"));
    assert!(outcome(
        30,
        "16000 clones and drops on eight threads, then every reference dropped",
        through
    ));
}

#[test]
fn probes_of_calls_never_made() {
    // Never-called rows tried through the glue: a decoder buffer, a surface mapped through DXGI, the desktop's memory.
    let (device, _context) = device();
    let UnknownObject::VideoContext(video) = object(&device, &IID_ID3D11_VIDEO_CONTEXT) else {
        panic!("a video context")
    };
    let UnknownObject::VideoDecoder(decoder) = object(&device, &IID_ID3D11_VIDEO_DECODER) else {
        panic!("a video decoder")
    };
    let lying = Lying {
        length: 1 << 20,
        touched: AtomicUsize::new(0),
    };
    assert!(
        video
            .get_decoder_buffer(&decoder, VideoDecoderBufferType(0), &lying)
            .is_err()
    );
    let texture = device
        .create_texture_2d(
            &Texture2DDesc {
                width: 4,
                height: 4,
                mip_levels: 1,
                array_size: 1,
                format: Format(28),
                sample_desc: SampleDesc {
                    count: 1,
                    quality: 0,
                },
                usage: Usage(0),
                bind_flags: 0x20,
                cpu_access_flags: 0,
                misc_flags: 0,
            },
            &[],
        )
        .expect("a texture");
    assert!(dxgi::Surface::map(&texture, MapFlags(1)).is_err());
    let Ok(dxgi::UnknownObject::OutputDuplication(duplication)) =
        dxgi_api().dxgi_get_debug_interface1(0, &IID_IDXGI_OUTPUT_DUPLICATION)
    else {
        panic!("a duplication")
    };
    assert!(duplication.map_desktop_surface().is_err());
    let reached = reached("ID3D11VideoContext::GetDecoderBuffer")
        + reached_dxgi("IDXGISurface::Map")
        + reached("IDXGISurface::Map")
        + reached_dxgi("IDXGIOutputDuplication::MapDesktopSurface");
    assert_eq!((reached, lying.touched.load(Ordering::SeqCst)), (0, 0));
}

#[test]
fn desktop_surfaces_are_never_mapped() {
    let Ok(dxgi::UnknownObject::OutputDuplication(duplication)) =
        dxgi_api().dxgi_get_debug_interface1(0, &IID_IDXGI_OUTPUT_DUPLICATION)
    else {
        panic!("a duplication")
    };
    assert!(duplication.map_desktop_surface().is_err());
    assert!(duplication.un_map_desktop_surface().is_err());
    assert_eq!(
        reached_dxgi("IDXGIOutputDuplication::MapDesktopSurface")
            + reached_dxgi("IDXGIOutputDuplication::UnMapDesktopSurface"),
        0
    );
}

#[test]
fn surfaces_are_never_mapped_through_dxgi() {
    let (device, _context) = device();
    let texture = device
        .create_texture_2d(
            &Texture2DDesc {
                width: 4,
                height: 4,
                mip_levels: 1,
                array_size: 1,
                format: Format(28),
                sample_desc: SampleDesc {
                    count: 1,
                    quality: 0,
                },
                usage: Usage(0),
                bind_flags: 0x20,
                cpu_access_flags: 0,
                misc_flags: 0,
            },
            &[],
        )
        .expect("a texture");
    assert!(dxgi::Surface::map(&texture, MapFlags(1)).is_err());
    assert!(dxgi::Surface::unmap(&texture).is_err());
    assert!(dxgi::Surface1::get_dc(&texture, false).is_err());
    assert_eq!(
        reached("IDXGISurface::Map")
            + reached("IDXGISurface::Unmap")
            + reached("IDXGISurface1::GetDC"),
        0
    );
}

#[test]
fn decoder_buffers_are_never_lent() {
    let (device, _context) = device();
    let UnknownObject::VideoContext(video) = object(&device, &IID_ID3D11_VIDEO_CONTEXT) else {
        panic!("a video context")
    };
    let UnknownObject::VideoDecoder(decoder) = object(&device, &IID_ID3D11_VIDEO_DECODER) else {
        panic!("a video decoder")
    };
    let lying = Lying {
        length: 64,
        touched: AtomicUsize::new(0),
    };
    assert!(
        video
            .get_decoder_buffer(&decoder, VideoDecoderBufferType(0), &lying)
            .is_err()
    );
    assert!(
        video
            .release_decoder_buffer(&decoder, VideoDecoderBufferType(0))
            .is_err()
    );
    assert_eq!(
        reached("ID3D11VideoContext::GetDecoderBuffer")
            + reached("ID3D11VideoContext::ReleaseDecoderBuffer"),
        0
    );
}

#[test]
fn the_reference_rasterizer_is_never_switched_to() {
    let (device, _context) = device();
    let UnknownObject::SwitchToRef(switch) = object(&device, &IID_ID3D11_SWITCH_TO_REF) else {
        panic!("a switch")
    };
    let _ = switch.set_use_ref(true);
    assert_eq!(reached("ID3D11SwitchToRef::SetUseRef"), 0);
}
