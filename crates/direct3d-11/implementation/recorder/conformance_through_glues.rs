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
        "ID3D11VideoContext3::DecoderBeginFrame1",
        "ID3D11Device::GetImmediateContext",
        "ID3D11Device1::GetImmediateContext1",
        "ID3D11Device2::GetImmediateContext2",
        "ID3D11Device3::GetImmediateContext3",
        "ID3D11DeviceChild::GetDevice",
        "ID3D11ClassInstance::GetClassLinkage"
    ],
    &[
        (
            "D3D11CreateDevice",
            "ID3D11Multithread::SetMultithreadProtected(b_mt_protect=true)"
        ),
        (
            "D3D11CreateDeviceAndSwapChain",
            "ID3D11Multithread::SetMultithreadProtected(b_mt_protect=true)"
        ),
        ("ID3D11DeviceContext::Map", "ID3D11Buffer::GetDesc()"),
        (
            "ID3D11DeviceContext::UpdateSubresource",
            "ID3D11Buffer::GetDesc()"
        ),
        (
            "ID3D11DeviceContext1::UpdateSubresource1",
            "ID3D11Buffer::GetDesc()"
        )
    ]
);
