#!/usr/bin/env python3
"""Writes src/com/generated.rs: the COM trait impls of the windows crate (0.62.2), each method
delegating to a hand-written inherent method or answering that it is not served.

    python3 crates/wcontainers-directx/generate.py <the windows-0.62.2 crate directory>
    cargo fmt -p wcontainers-directx

The crate directory is the one cargo unpacked (`cargo metadata` lists its manifest path)."""
import os, re, sys

crate = sys.argv[1]
traits = {}
for api in ("Direct3D11", "Dxgi"):
    text = open(os.path.join(crate, "src", "Windows", "Win32", "Graphics", api, "mod.rs")).read()
    for m in re.finditer(r"pub trait (\w+)_Impl: [^{]*\{\n(.*?)\n\}", text, re.S):
        traits[m.group(1)] = [l.strip() for l in m.group(2).splitlines() if l.strip().startswith("fn ")]

CHILD = ["ID3D11DeviceChild"]
# What DXGI offers on every resource, and on buffers and textures of one subresource.
DXGI_RESOURCE = ["IDXGIResource1", "IDXGIResource", "IDXGIDeviceSubObject", "IDXGIObject"]
DXGI_SURFACE = ["IDXGISurface1", "IDXGISurface"]
for empty in ("ID3DDeviceContextState", "ID3D11InputLayout", "ID3D11VertexShader", "ID3D11PixelShader", "ID3D11ComputeShader", "ID3D11Predicate"):
    traits[empty] = []
structs = {
    "Device": ["ID3D11Device1", "ID3D11Device", "IDXGIDevice3", "IDXGIDevice2", "IDXGIDevice1", "IDXGIDevice", "IDXGIObject"],
    "Context": ["ID3D11DeviceContext1", "ID3D11DeviceContext"] + CHILD
    + ["ID3D11Multithread", "ID3DUserDefinedAnnotation"],
    "Adapter": ["IDXGIAdapter1", "IDXGIAdapter", "IDXGIObject"],
    "Output": ["IDXGIOutput", "IDXGIObject"],
    "Factory": ["IDXGIFactory2", "IDXGIFactory1", "IDXGIFactory", "IDXGIObject"],
    "SwapChain": ["IDXGISwapChain1", "IDXGISwapChain", "IDXGIDeviceSubObject", "IDXGIObject"],
    "Buffer": ["ID3D11Buffer", "ID3D11Resource"] + CHILD + DXGI_RESOURCE + DXGI_SURFACE,
    "Texture1D": ["ID3D11Texture1D", "ID3D11Resource"] + CHILD + DXGI_RESOURCE + DXGI_SURFACE,
    "Texture2D": ["ID3D11Texture2D", "ID3D11Resource"] + CHILD + DXGI_RESOURCE + DXGI_SURFACE,
    "Texture3D": ["ID3D11Texture3D", "ID3D11Resource"] + CHILD + DXGI_RESOURCE,
    "RenderTargetView": ["ID3D11RenderTargetView", "ID3D11View"] + CHILD,
    "DepthStencilView": ["ID3D11DepthStencilView", "ID3D11View"] + CHILD,
    "ShaderResourceView": ["ID3D11ShaderResourceView", "ID3D11View"] + CHILD,
    "UnorderedAccessView": ["ID3D11UnorderedAccessView", "ID3D11View"] + CHILD,
    "SamplerState": ["ID3D11SamplerState"] + CHILD,
    "BlendState": ["ID3D11BlendState1", "ID3D11BlendState"] + CHILD,
    "DepthStencilState": ["ID3D11DepthStencilState"] + CHILD,
    "RasterizerState": ["ID3D11RasterizerState1", "ID3D11RasterizerState"] + CHILD,
    "InputLayout": ["ID3D11InputLayout"] + CHILD,
    "Query": ["ID3D11Query", "ID3D11Asynchronous"] + CHILD,
    "Predicate": ["ID3D11Predicate", "ID3D11Query", "ID3D11Asynchronous"] + CHILD,
    "VertexShader": ["ID3D11VertexShader"] + CHILD,
    "PixelShader": ["ID3D11PixelShader"] + CHILD,
    "ComputeShader": ["ID3D11ComputeShader"] + CHILD,
    "CommandList": ["ID3D11CommandList"] + CHILD,
    "DeviceContextState": ["ID3DDeviceContextState"] + CHILD,
}

hand = {
    "ID3D11Device": "CreateBuffer CreateTexture1D CreateTexture2D CreateTexture3D CreateShaderResourceView CreateRenderTargetView CreateDepthStencilView CreateInputLayout CreateVertexShader CreatePixelShader CreateBlendState CreateDepthStencilState CreateRasterizerState CreateSamplerState CreateQuery CreatePredicate CreateUnorderedAccessView CreateComputeShader CreateDeferredContext CheckFormatSupport CheckMultisampleQualityLevels CheckFeatureSupport GetFeatureLevel GetCreationFlags GetDeviceRemovedReason GetImmediateContext SetExceptionMode GetExceptionMode",
    "ID3D11Device1": "GetImmediateContext1 CreateDeferredContext1 CreateDeviceContextState CreateBlendState1 CreateRasterizerState1",
    "IDXGIDevice1": "SetMaximumFrameLatency GetMaximumFrameLatency",
    "IDXGIDevice2": "OfferResources ReclaimResources",
    "IDXGIDevice3": "Trim",
    "ID3D11DeviceContext1": "SwapDeviceContextState CopySubresourceRegion1 UpdateSubresource1 DiscardResource DiscardView DiscardView1 VSSetConstantBuffers1 PSSetConstantBuffers1 VSGetConstantBuffers1 PSGetConstantBuffers1 CSSetConstantBuffers1 CSGetConstantBuffers1 ClearView",
    "ID3D11Multithread": "Enter Leave SetMultithreadProtected GetMultithreadProtected",
    "ID3DUserDefinedAnnotation": "BeginEvent EndEvent SetMarker GetStatus",
    "IDXGIResource": "GetUsage",
    "ID3D11BlendState1": "GetDesc1",
    "ID3D11RasterizerState1": "GetDesc1",
    "IDXGIObject": "GetParent",
    "IDXGIDevice": "GetAdapter",
    "IDXGIAdapter": "GetDesc EnumOutputs CheckInterfaceSupport",
    "IDXGIAdapter1": "GetDesc1",
    "IDXGIOutput": "GetDesc GetDisplayModeList FindClosestMatchingMode",
    "IDXGIFactory": "EnumAdapters MakeWindowAssociation GetWindowAssociation CreateSwapChain",
    "IDXGIFactory1": "EnumAdapters1 IsCurrent",
    "IDXGIFactory2": "IsWindowedStereoEnabled CreateSwapChainForHwnd",
    "IDXGIDeviceSubObject": "GetDevice",
    "IDXGISwapChain": "Present GetBuffer SetFullscreenState GetFullscreenState GetDesc ResizeBuffers GetLastPresentCount",
    "IDXGISwapChain1": "GetDesc1 GetHwnd Present1 SetRotation GetRotation",
    "ID3D11DeviceContext": "VSSetConstantBuffers PSSetShaderResources PSSetShader PSSetSamplers VSSetShader DrawIndexed Draw Map Unmap PSSetConstantBuffers IASetInputLayout IASetVertexBuffers IASetIndexBuffer DrawIndexedInstanced DrawInstanced IASetPrimitiveTopology VSSetShaderResources VSSetSamplers OMSetRenderTargets OMSetBlendState OMSetDepthStencilState RSSetState RSSetViewports RSSetScissorRects CopySubresourceRegion CopyResource UpdateSubresource ClearRenderTargetView ClearDepthStencilView GenerateMips Begin End GetData ClearState Flush GetType GetContextFlags VSGetShader PSGetShader VSGetConstantBuffers PSGetConstantBuffers VSGetShaderResources PSGetShaderResources VSGetSamplers PSGetSamplers IAGetInputLayout IAGetVertexBuffers IAGetIndexBuffer IAGetPrimitiveTopology OMGetRenderTargets OMGetBlendState OMGetDepthStencilState RSGetState RSGetViewports RSGetScissorRects CSSetShader CSGetShader CSSetConstantBuffers CSGetConstantBuffers CSSetShaderResources CSGetShaderResources CSSetSamplers CSGetSamplers CSSetUnorderedAccessViews CSGetUnorderedAccessViews DrawInstancedIndirect DrawIndexedInstancedIndirect Dispatch DispatchIndirect ClearUnorderedAccessViewUint ClearUnorderedAccessViewFloat CopyStructureCount OMSetRenderTargetsAndUnorderedAccessViews OMGetRenderTargetsAndUnorderedAccessViews ExecuteCommandList FinishCommandList ResolveSubresource SetResourceMinLOD GetResourceMinLOD",
    "ID3D11Resource": "GetType SetEvictionPriority GetEvictionPriority",
    "ID3D11View": "GetResource",
    "ID3D11Asynchronous": "GetDataSize",
    "ID3D11Query": "GetDesc",
    "ID3D11CommandList": "GetContextFlags",
}
for t in ["ID3D11Buffer", "ID3D11Texture1D", "ID3D11Texture2D", "ID3D11Texture3D", "ID3D11RenderTargetView", "ID3D11DepthStencilView", "ID3D11ShaderResourceView", "ID3D11UnorderedAccessView", "ID3D11SamplerState", "ID3D11BlendState", "ID3D11DepthStencilState", "ID3D11RasterizerState"]:
    hand[t] = "GetDesc"
private = {"GetPrivateData", "SetPrivateData", "SetPrivateDataInterface"}
# DXGI's calls on resources that Direct3D 11's own calls or the child's serve.
on_resource = {
    ("IDXGIObject", "GetParent"): "self.child.query_device(riid, ppparent)",
    ("IDXGIDeviceSubObject", "GetDevice"): "self.child.query_device(riid, ppdevice)",
    ("IDXGIResource", "SetEvictionPriority"): "{ self.set_eviction_priority(evictionpriority.0); Ok(()) }",
    ("IDXGISurface", "GetDesc"): "self.get_desc_dxgi()",
    ("IDXGIResource", "GetEvictionPriority"): "Ok(DXGI_RESOURCE_PRIORITY(self.get_eviction_priority()))",
}
# Structs whose unserved void calls remove the device (the context, and the device itself).
removes = {"Device", "Context"}


def unused_stage(name, p, what):
    """The body of a context method on a stage or feature not served: served while it binds nothing."""
    if name in ("SetPredication", "GetPredication"):
        return f"self.{snake(name)}({', '.join(p)})"
    if re.fullmatch(r"(GS|HS|DS)SetShader", name):
        return f'self.only_unbinding("{what}", !{p[0]}.is_null())'
    if re.fullmatch(r"(GS|HS|DS)Set(ConstantBuffers1?|Samplers)", name):
        return f"self.{snake(name)}({', '.join(p)})"
    if re.fullmatch(r"(GS|HS|DS)Get(ConstantBuffers1?|Samplers)", name):
        return f"self.{snake(name)}({', '.join(p)})"
    if re.fullmatch(r"(GS|HS|DS)SetShaderResources", name):
        return f'self.only_unbinding("{what}", super::context::binds({p[2]}, {p[1]}))'
    if name == "SOSetTargets":
        return f'self.only_unbinding("{what}", super::context::binds({p[1]}, {p[0]}))'
    if re.fullmatch(r"(GS|HS|DS)GetConstantBuffers1", name):
        return f"super::context::nothing_bound(super::context::out_array({p[2]}), {p[1]})"
    if re.fullmatch(r"(GS|HS|DS)Get(ConstantBuffers|ShaderResources|Samplers)", name):
        return f"super::context::nothing_bound({p[2]}, {p[1]})"
    if name == "SOGetTargets":
        return f"super::context::nothing_bound({p[1]}, {p[0]})"
    if re.fullmatch(r"(GS|HS|DS)GetShader", name):
        return f"super::context::no_instances({p[2]}); let _ = {p[0]}.write(None);"
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
       "use windows::Win32::Graphics::Direct3D11::*;", "use windows::Win32::Graphics::Dxgi::*;", "",
       "use super::*;", ""]
for struct, tlist in structs.items():
    for t in tlist:
        out.append(f"impl {t}_Impl for {struct}_Impl {{")
        for sig in traits[t]:
            sig = fix(sig)
            name = re.match(r"fn (\w+)\(", sig).group(1)
            end = sig.find(") ->") if ") ->" in sig else sig.rfind(")")
            params = re.findall(r"(\w+): ", sig.split("(", 1)[1][: end - sig.index("(") - 1])
            params = [p for p in params if p != "self"]
            args = ", ".join(params)
            ret = sig[end + 1:].strip()
            unused = t.startswith("ID3D11DeviceContext") and unused_stage(name, params, f"{t}::{name}")
            if unused:
                body = unused
            elif t == "ID3D11DeviceChild":
                body = f"self.child.{snake(name)}({args})"
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
            out.append(f"        {body}")
            out.append("    }")
        out.append("}")
        out.append("")
open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "src", "com", "generated.rs"), "w").write(
    "\n".join(out) + "\n"
)
