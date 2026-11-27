//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The conformance suite of direct3d-11, run on this implementation through both glues (`served`), every answer and every
//! argument the implementation received compared with the same call made on the implementation directly.

wcontainers_direct3d_11_conformance::tests!(
    wcontainers_direct3d_11_direct::served(wcontainers_direct3d_11_recorder::Recorder),
    wcontainers_direct3d_11_recorder::Recorder,
    wcontainers_direct3d_11_recorder::arguments_logged,
    wcontainers_direct3d_11_recorder::arguments_since,
    &[
        "IDXGIInfoQueue::SetBreakOnCategory",
        "IDXGIInfoQueue::SetBreakOnSeverity",
        "IDXGIInfoQueue::SetBreakOnID",
        "ID3D11InfoQueue::SetBreakOnCategory",
        "ID3D11InfoQueue::SetBreakOnSeverity",
        "ID3D11InfoQueue::SetBreakOnID",
        "ID3D11Device::SetExceptionMode",
        "ID3D11Device::CreateTexture1D",
        "ID3D11Device::CreateTexture2D",
        "ID3D11Device::CreateTexture3D",
        "ID3D11VideoContext3::DecoderBeginFrame1",
        "ID3D11Device::GetImmediateContext",
        "ID3D11Device1::GetImmediateContext1",
        "ID3D11Device2::GetImmediateContext2",
        "ID3D11Device3::GetImmediateContext3",
        "ID3D11Device1::CreateDeviceContextState",
        "ID3D11DeviceChild::GetDevice on DeviceContext",
        "D3D11CreateDevice",
        "D3D11CreateDeviceAndSwapChain",
        "ID3D11DeviceContext::Map"
    ]
);
