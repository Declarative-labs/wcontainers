#!/usr/bin/env python3
"""Writes src/com/generated.rs: the COM trait impls of the windows crate (0.62.2), each method
delegating to a hand-written inherent method or answering that it is not served.

    python3 crates/wcontainers-directx/generate.py <the windows-0.62.2 crate directory>
    cargo fmt -p wcontainers-directx

The crate directory is the one cargo unpacked (`cargo metadata` lists its manifest path)."""
import os, re, sys

crate = sys.argv[1]
traits = {}
for api in ("Direct3D11", "Direct3D10", "Dxgi"):
    text = open(os.path.join(crate, "src", "Windows", "Win32", "Graphics", api, "mod.rs")).read()
    for m in re.finditer(r"pub trait (\w+)_Impl: [^{]*\{\n(.*?)\n\}", text, re.S):
        traits[m.group(1)] = [l.strip() for l in m.group(2).splitlines() if l.strip().startswith("fn ")]

CHILD = ["ID3D11DeviceChild"]
# What DXGI offers on every resource, and on buffers and textures of one subresource.
DXGI_RESOURCE = ["IDXGIResource1", "IDXGIResource", "IDXGIDeviceSubObject", "IDXGIObject"]
DXGI_SURFACE = ["IDXGISurface1", "IDXGISurface"]
# Direct3D 10's interfaces, which Direct3D 11's objects also offer.
D3D10_CHILD = ["ID3D10DeviceChild"]
D3D10_RESOURCE = ["ID3D10Resource"] + D3D10_CHILD
D3D10_VIEW = ["ID3D10View"] + D3D10_CHILD
for empty in ("ID3D10InputLayout", "ID3D10VertexShader", "ID3D10PixelShader", "ID3D10GeometryShader", "ID3D10Predicate", "ID3DDeviceContextState", "ID3D11InputLayout", "ID3D11VertexShader", "ID3D11PixelShader", "ID3D11ComputeShader", "ID3D11GeometryShader", "ID3D11HullShader", "ID3D11DomainShader", "ID3D11Predicate"):
    traits[empty] = []
structs = {
    "Device": ["ID3D11Device3", "ID3D11Device2", "ID3D11Device1", "ID3D11Device", "IDXGIDevice3", "IDXGIDevice2", "IDXGIDevice1", "IDXGIDevice", "IDXGIObject",
               "ID3D10Device1", "ID3D10Device", "ID3D10Multithread"],
    "Context": ["ID3D11DeviceContext3", "ID3D11DeviceContext2", "ID3D11DeviceContext1", "ID3D11DeviceContext"] + CHILD
    + ["ID3D11Multithread", "ID3DUserDefinedAnnotation"],
    "Adapter": ["IDXGIAdapter1", "IDXGIAdapter", "IDXGIObject"],
    "Output": ["IDXGIOutput", "IDXGIObject"],
    "Factory": ["IDXGIFactory6", "IDXGIFactory5", "IDXGIFactory4", "IDXGIFactory3", "IDXGIFactory2",
                "IDXGIFactory1", "IDXGIFactory", "IDXGIObject"],
    "SwapChain": ["IDXGISwapChain4", "IDXGISwapChain3", "IDXGISwapChain2", "IDXGISwapChain1",
                  "IDXGISwapChain", "IDXGIDeviceSubObject", "IDXGIObject"],
    "Buffer": ["ID3D11Buffer", "ID3D11Resource"] + CHILD + DXGI_RESOURCE + DXGI_SURFACE
    + ["ID3D10Buffer"] + D3D10_RESOURCE,
    "Texture1D": ["ID3D11Texture1D", "ID3D11Resource"] + CHILD + DXGI_RESOURCE + DXGI_SURFACE
    + ["ID3D10Texture1D"] + D3D10_RESOURCE,
    "Texture2D": ["ID3D11Texture2D1", "ID3D11Texture2D", "ID3D11Resource"] + CHILD + DXGI_RESOURCE + DXGI_SURFACE
    + ["ID3D10Texture2D"] + D3D10_RESOURCE,
    "Texture3D": ["ID3D11Texture3D1", "ID3D11Texture3D", "ID3D11Resource"] + CHILD + DXGI_RESOURCE + DXGI_SURFACE
    + ["ID3D10Texture3D"] + D3D10_RESOURCE,
    "RenderTargetView": ["ID3D11RenderTargetView1", "ID3D11RenderTargetView", "ID3D11View"] + CHILD
    + ["ID3D10RenderTargetView"] + D3D10_VIEW,
    "DepthStencilView": ["ID3D11DepthStencilView", "ID3D11View"] + CHILD
    + ["ID3D10DepthStencilView"] + D3D10_VIEW,
    "ShaderResourceView": ["ID3D11ShaderResourceView1", "ID3D11ShaderResourceView", "ID3D11View"] + CHILD
    + ["ID3D10ShaderResourceView1", "ID3D10ShaderResourceView"] + D3D10_VIEW,
    "UnorderedAccessView": ["ID3D11UnorderedAccessView1", "ID3D11UnorderedAccessView", "ID3D11View"] + CHILD,
    "SamplerState": ["ID3D11SamplerState"] + CHILD + ["ID3D10SamplerState"] + D3D10_CHILD,
    "BlendState": ["ID3D11BlendState1", "ID3D11BlendState"] + CHILD
    + ["ID3D10BlendState1", "ID3D10BlendState"] + D3D10_CHILD,
    "DepthStencilState": ["ID3D11DepthStencilState"] + CHILD + ["ID3D10DepthStencilState"] + D3D10_CHILD,
    "RasterizerState": ["ID3D11RasterizerState2", "ID3D11RasterizerState1", "ID3D11RasterizerState"] + CHILD
    + ["ID3D10RasterizerState"] + D3D10_CHILD,
    "InputLayout": ["ID3D11InputLayout"] + CHILD + ["ID3D10InputLayout"] + D3D10_CHILD,
    "Query": ["ID3D11Query1", "ID3D11Query", "ID3D11Asynchronous"] + CHILD
    + ["ID3D10Query", "ID3D10Asynchronous"] + D3D10_CHILD,
    "Predicate": ["ID3D11Predicate", "ID3D11Query", "ID3D11Asynchronous"] + CHILD
    + ["ID3D10Predicate", "ID3D10Query", "ID3D10Asynchronous"] + D3D10_CHILD,
    "VertexShader": ["ID3D11VertexShader"] + CHILD + ["ID3D10VertexShader"] + D3D10_CHILD,
    "PixelShader": ["ID3D11PixelShader"] + CHILD + ["ID3D10PixelShader"] + D3D10_CHILD,
    "GeometryShader": ["ID3D11GeometryShader"] + CHILD + ["ID3D10GeometryShader"] + D3D10_CHILD,
    "ComputeShader": ["ID3D11ComputeShader"] + CHILD,
    "HullShader": ["ID3D11HullShader"] + CHILD,
    "DomainShader": ["ID3D11DomainShader"] + CHILD,
    "CommandList": ["ID3D11CommandList"] + CHILD,
    "DeviceContextState": ["ID3DDeviceContextState"] + CHILD,
}

hand = {
    "ID3D11Device": "CreateBuffer CreateTexture1D CreateTexture2D CreateTexture3D CreateShaderResourceView CreateRenderTargetView CreateDepthStencilView CreateInputLayout CreateVertexShader CreatePixelShader CreateBlendState CreateDepthStencilState CreateRasterizerState CreateSamplerState CreateQuery CreatePredicate CreateUnorderedAccessView CreateComputeShader CreateGeometryShader CreateGeometryShaderWithStreamOutput CreateHullShader CreateDomainShader CreateDeferredContext CheckFormatSupport CheckMultisampleQualityLevels CheckFeatureSupport GetFeatureLevel GetCreationFlags GetDeviceRemovedReason GetImmediateContext SetExceptionMode GetExceptionMode",
    "ID3D11Device1": "GetImmediateContext1 CreateDeferredContext1 CreateDeviceContextState CreateBlendState1 CreateRasterizerState1",
    "ID3D11Device2": "GetImmediateContext2 CreateDeferredContext2 CheckMultisampleQualityLevels1",
    "ID3D11Device3": "CreateTexture2D1 CreateTexture3D1 CreateRasterizerState2 CreateShaderResourceView1 CreateUnorderedAccessView1 CreateRenderTargetView1 CreateQuery1 GetImmediateContext3 CreateDeferredContext3",
    "ID3D11DeviceContext2": "IsAnnotationEnabled SetMarkerInt BeginEventInt",
    "ID3D11DeviceContext3": "Flush1 GetHardwareProtectionState",
    "IDXGIDevice1": "SetMaximumFrameLatency GetMaximumFrameLatency",
    "IDXGIDevice2": "OfferResources ReclaimResources",
    "IDXGIDevice3": "Trim",
    "ID3D11DeviceContext1": "SwapDeviceContextState CopySubresourceRegion1 UpdateSubresource1 DiscardResource DiscardView DiscardView1 VSSetConstantBuffers1 PSSetConstantBuffers1 VSGetConstantBuffers1 PSGetConstantBuffers1 CSSetConstantBuffers1 CSGetConstantBuffers1 ClearView",
    "ID3D11Multithread": "Enter Leave SetMultithreadProtected GetMultithreadProtected",
    "ID3DUserDefinedAnnotation": "BeginEvent EndEvent SetMarker GetStatus",
    "IDXGIResource": "GetUsage",
    "IDXGISurface1": "GetDC ReleaseDC",
    "ID3D11BlendState1": "GetDesc1",
    "ID3D11RasterizerState1": "GetDesc1",
    "ID3D11RasterizerState2": "GetDesc2",
    "IDXGIObject": "GetParent",
    "IDXGIDevice": "GetAdapter",
    "IDXGIAdapter": "GetDesc EnumOutputs CheckInterfaceSupport",
    "IDXGIAdapter1": "GetDesc1",
    "IDXGIOutput": "GetDesc GetDisplayModeList FindClosestMatchingMode",
    "IDXGIFactory": "EnumAdapters MakeWindowAssociation GetWindowAssociation CreateSwapChain",
    "IDXGIFactory1": "EnumAdapters1 IsCurrent",
    "IDXGIFactory2": "IsWindowedStereoEnabled CreateSwapChainForHwnd",
    "IDXGIFactory3": "GetCreationFlags",
    "IDXGIFactory4": "EnumAdapterByLuid",
    "IDXGIFactory5": "CheckFeatureSupport",
    "IDXGIFactory6": "EnumAdapterByGpuPreference",
    "IDXGIDeviceSubObject": "GetDevice",
    "IDXGISwapChain": "Present GetBuffer SetFullscreenState GetFullscreenState GetDesc ResizeBuffers GetLastPresentCount",
    "IDXGISwapChain1": "GetDesc1 GetHwnd Present1 SetRotation GetRotation",
    "IDXGISwapChain2": "SetMaximumFrameLatency GetMaximumFrameLatency",
    "IDXGISwapChain3": "GetCurrentBackBufferIndex CheckColorSpaceSupport SetColorSpace1 ResizeBuffers1",
    "IDXGISwapChain4": "SetHDRMetaData",
    "ID3D11DeviceContext": "VSSetConstantBuffers PSSetShaderResources PSSetShader PSSetSamplers VSSetShader DrawIndexed Draw Map Unmap PSSetConstantBuffers IASetInputLayout IASetVertexBuffers IASetIndexBuffer DrawIndexedInstanced DrawInstanced IASetPrimitiveTopology VSSetShaderResources VSSetSamplers OMSetRenderTargets OMSetBlendState OMSetDepthStencilState RSSetState RSSetViewports RSSetScissorRects CopySubresourceRegion CopyResource UpdateSubresource ClearRenderTargetView ClearDepthStencilView GenerateMips Begin End GetData ClearState Flush GetType GetContextFlags VSGetShader PSGetShader VSGetConstantBuffers PSGetConstantBuffers VSGetShaderResources PSGetShaderResources VSGetSamplers PSGetSamplers IAGetInputLayout IAGetVertexBuffers IAGetIndexBuffer IAGetPrimitiveTopology OMGetRenderTargets OMGetBlendState OMGetDepthStencilState RSGetState RSGetViewports RSGetScissorRects CSSetShader CSGetShader CSSetConstantBuffers CSGetConstantBuffers CSSetShaderResources CSGetShaderResources CSSetSamplers CSGetSamplers CSSetUnorderedAccessViews CSGetUnorderedAccessViews DrawInstancedIndirect DrawIndexedInstancedIndirect Dispatch DispatchIndirect ClearUnorderedAccessViewUint ClearUnorderedAccessViewFloat CopyStructureCount OMSetRenderTargetsAndUnorderedAccessViews OMGetRenderTargetsAndUnorderedAccessViews ExecuteCommandList FinishCommandList ResolveSubresource SetResourceMinLOD GetResourceMinLOD GSSetShader GSGetShader GSSetShaderResources GSGetShaderResources SOSetTargets SOGetTargets DrawAuto HSSetShader HSGetShader HSSetShaderResources HSGetShaderResources DSSetShader DSGetShader DSSetShaderResources DSGetShaderResources",
    "ID3D11Resource": "GetType SetEvictionPriority GetEvictionPriority",
    "ID3D11View": "GetResource",
    "ID3D11Asynchronous": "GetDataSize",
    "ID3D11Query": "GetDesc",
    "ID3D11Query1": "GetDesc1",
    "ID3D11CommandList": "GetContextFlags",
}
for t in ["ID3D11Texture2D1", "ID3D11Texture3D1", "ID3D11RenderTargetView1", "ID3D11ShaderResourceView1", "ID3D11UnorderedAccessView1"]:
    hand[t] = "GetDesc1"
for t in ["ID3D11Buffer", "ID3D11Texture1D", "ID3D11Texture2D", "ID3D11Texture3D", "ID3D11RenderTargetView", "ID3D11DepthStencilView", "ID3D11ShaderResourceView", "ID3D11UnorderedAccessView", "ID3D11SamplerState", "ID3D11BlendState", "ID3D11DepthStencilState", "ID3D11RasterizerState"]:
    hand[t] = "GetDesc"
private = {"GetPrivateData", "SetPrivateData", "SetPrivateDataInterface"}
# Direct3D 10's calls served by a hand-written method named with a 10 after the call's name, and
# those served by Direct3D 11's own.
hand10 = {"GetDesc", "GetDesc1", "GetResource", "GetType", "GetDevice", "Map", "Unmap", "Begin", "End", "GetData", "GetDataSize"}
same10 = {"GetEvictionPriority", "SetEvictionPriority"} | private
# DXGI's calls on resources that Direct3D 11's own calls or the child's serve.
on_resource = {
    ("IDXGIObject", "GetParent"): "self.child.query_device(riid, ppparent)",
    ("IDXGIDeviceSubObject", "GetDevice"): "self.child.query_device(riid, ppdevice)",
    ("IDXGIResource", "SetEvictionPriority"): "{ self.set_eviction_priority(evictionpriority.0); Ok(()) }",
    ("IDXGISurface", "GetDesc"): "self.get_desc_dxgi()",
    ("IDXGIResource", "GetEvictionPriority"): "Ok(DXGI_RESOURCE_PRIORITY(self.get_eviction_priority()))",
    ("IDXGIResource", "GetSharedHandle"): "self.shared_handle()",
    ("IDXGIResource1", "CreateSharedHandle"): "self.nt_shared_handle()",
}
# Direct3D 10's device on the device: calls of the same form as Direct3D 11's served by those, the
# counters and shared resources not served, every other by a method named with a 10 after it.
dev10_same = {"GetDeviceRemovedReason", "SetExceptionMode", "GetExceptionMode", "GetCreationFlags",
              "CheckFormatSupport", "CheckMultisampleQualityLevels"}
dev10_missing = {"CreateCounter", "CheckCounterInfo", "CheckCounter", "OpenSharedResource"}
# The immediate context's calls that do nothing while Direct3D 10's behaviour is active on it: all
# but the swap itself and the getters, which read the default state then, except `PSGetSamplers`,
# which writes nothing (as real Direct3D's does: Wine's test_device_context_state).
CONTEXTS = ("ID3D11DeviceContext", "ID3D11DeviceContext1", "ID3D11DeviceContext2", "ID3D11DeviceContext3")


def off(name, ret):
    """The early return of a context call made while it does nothing, or None for a call kept."""
    if name == "PSGetSamplers":
        return "return;"
    if "Get" in name or name == "SwapDeviceContextState":
        return None
    if ret.startswith("-> windows::core::Result"):
        return "return Err(E_INVALIDARG.into());"
    if ret == "-> windows::core::HRESULT":
        return "return E_INVALIDARG;"
    return "return;" if not ret else "return Default::default();"


# Structs whose unserved void calls remove the device (the context, and the device itself).
removes = {"Device", "Context"}


def unused_stage(name, p, what):
    """The body of a context method on a stage or feature not served: served while it binds nothing."""
    if name in ("SetPredication", "GetPredication"):
        return f"self.{snake(name)}({', '.join(p)})"
    if re.fullmatch(r"(GS|HS|DS)Set(ConstantBuffers1?|Samplers)", name):
        return f"self.{snake(name)}({', '.join(p)})"
    if re.fullmatch(r"(GS|HS|DS)Get(ConstantBuffers1?|Samplers)", name):
        return f"self.{snake(name)}({', '.join(p)})"
    return None


def snake(name):
    s = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", name)
    s = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", s)
    return s.lower()


def fix(sig):
    sig = sig.replace("super::super::Foundation::", "windows::Win32::Foundation::")
    sig = sig.replace("super::Direct3D::", "windows::Win32::Graphics::Direct3D::")
    sig = sig.replace("super::Dxgi::Common::", "windows::Win32::Graphics::Dxgi::Common::")
    sig = re.sub(r"(?<![:\w])Common::", "windows::Win32::Graphics::Dxgi::Common::", sig)
    sig = sig.replace("super::Gdi::", "windows::Win32::Graphics::Gdi::")
    sig = sig.replace("super::super::Security::", "windows::Win32::Security::")
    sig = sig.replace("windows_core::", "windows::core::")
    return sig.rstrip(";")


out = ["// Written by a generator from the windows crate's (0.62.2) trait definitions: each COM method",
       "// calls the hand-written method of the same name in snake case, or answers that it is not served,",
       "// under the library's lock.",
       "#![allow(non_snake_case, unused_variables, clippy::too_many_arguments)]", "",
       "use windows::Win32::Graphics::Direct3D10::*;", "use windows::Win32::Graphics::Direct3D11::*;",
       "use windows::Win32::Foundation::E_INVALIDARG;", "use windows::Win32::Graphics::Dxgi::*;", "",
       "use super::*;", ""]
for struct, tlist in structs.items():
    for t in tlist:
        out.append(f"impl {t}_Impl for {struct}_Impl {{")
        for sig in traits[t]:
            sig = fix(sig)
            name = re.match(r"fn (\w+)\(", sig).group(1)
            end = sig.find(") ->") if ") ->" in sig else sig.rfind(")")
            params = re.findall(r"((?:r#)?\w+): ", sig.split("(", 1)[1][: end - sig.index("(") - 1])
            params = [p for p in params if p != "self"]
            args = ", ".join(params)
            ret = sig[end + 1:].strip()
            unused = t.startswith("ID3D11DeviceContext") and unused_stage(name, params, f"{t}::{name}")
            if unused:
                body = unused
            elif struct == "Device" and t.startswith("ID3D10") and name not in dev10_missing:
                if name in private:
                    body = f"self.private.{snake(name)}({args})"
                elif name in dev10_same:
                    body = f"self.{snake(name)}({args})"
                else:
                    body = f"self.{snake(name)}10({args})"
            elif (t, name) == ("ID3D11DeviceContext2", "EndEvent"):
                body = "let _ = self.end_event();"
            elif t == "ID3D11DeviceChild" or (t == "ID3D10DeviceChild" and name in private):
                body = f"self.child.{snake(name)}({args})"
            elif t == "ID3D10DeviceChild" and name in hand10:
                body = f"self.child.{snake(name)}10({args})"
            elif t.startswith("ID3D10") and name in hand10:
                body = f"self.{snake(name)}10({args})"
            elif t.startswith("ID3D10") and name in same10:
                body = f"self.{snake(name)}({args})"
            elif t in ("ID3D11Device", "IDXGIObject") and name in private:
                owner = "child.private" if struct.startswith(("Buffer", "Texture")) else "private"
                body = f"self.{owner}.{snake(name)}({args})"
            elif (t, name) in on_resource and struct.startswith(("Buffer", "Texture")):
                body = on_resource[(t, name)]
            elif name in hand.get(t, "").split():
                body = f"self.{snake(name)}({args})"
            elif ret.startswith("-> windows::core::Result"):
                body = f'Err(missing("{t}::{name}"))'
            elif ret == "-> windows::core::HRESULT":
                body = f'missing("{t}::{name}").code()'
            elif struct in removes:
                body = f'self.unserved("{t}::{name}")' + ("" if not ret else "; Default::default()")
            elif not ret:
                body = f'missing_void("{t}::{name}")'
            else:
                body = f'missing_void("{t}::{name}"); Default::default()'
            outs = [q for q in params if re.search(rf"\b{q}: windows::core::OutRef<", sig)]
            if struct == "Device" and name.startswith("Create") and outs and ret.startswith("-> windows::core::Result"):
                last = outs[-1]
                body = f"creating({last}, |{last}| {body})"
            out.append(f"    {sig} {{")
            out.append("        let _call = self.call();" if struct in removes else "        let _lock = lock();")
            early = struct == "Context" and t in CONTEXTS and off(name, ret)
            if early:
                out.append(f"        if self.emulates10() {{ {early} }}")
            out.append(f"        {body}")
            out.append("    }")
        out.append("}")
        out.append("")
open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "src", "com", "generated.rs"), "w").write(
    "\n".join(out) + "\n"
)
