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
for empty in ("ID3D11InputLayout", "ID3D11VertexShader", "ID3D11PixelShader"):
    traits[empty] = []
structs = {
    "Device": ["ID3D11Device", "IDXGIDevice", "IDXGIObject"],
    "Context": ["ID3D11DeviceContext"] + CHILD,
    "Adapter": ["IDXGIAdapter1", "IDXGIAdapter", "IDXGIObject"],
    "Buffer": ["ID3D11Buffer", "ID3D11Resource"] + CHILD,
    "Texture2D": ["ID3D11Texture2D", "ID3D11Resource"] + CHILD,
    "RenderTargetView": ["ID3D11RenderTargetView", "ID3D11View"] + CHILD,
    "DepthStencilView": ["ID3D11DepthStencilView", "ID3D11View"] + CHILD,
    "ShaderResourceView": ["ID3D11ShaderResourceView", "ID3D11View"] + CHILD,
    "SamplerState": ["ID3D11SamplerState"] + CHILD,
    "BlendState": ["ID3D11BlendState"] + CHILD,
    "DepthStencilState": ["ID3D11DepthStencilState"] + CHILD,
    "RasterizerState": ["ID3D11RasterizerState"] + CHILD,
    "InputLayout": ["ID3D11InputLayout"] + CHILD,
    "VertexShader": ["ID3D11VertexShader"] + CHILD,
    "PixelShader": ["ID3D11PixelShader"] + CHILD,
}

hand = {
    "ID3D11Device": "CreateBuffer CreateTexture2D CreateShaderResourceView CreateRenderTargetView CreateDepthStencilView CreateInputLayout CreateVertexShader CreatePixelShader CreateBlendState CreateDepthStencilState CreateRasterizerState CreateSamplerState CheckFormatSupport CheckMultisampleQualityLevels CheckFeatureSupport GetFeatureLevel GetCreationFlags GetDeviceRemovedReason GetImmediateContext SetExceptionMode GetExceptionMode",
    "IDXGIObject": "GetParent",
    "IDXGIDevice": "GetAdapter",
    "IDXGIAdapter": "GetDesc EnumOutputs",
    "IDXGIAdapter1": "GetDesc1",
    "ID3D11DeviceContext": "VSSetConstantBuffers PSSetShaderResources PSSetShader PSSetSamplers VSSetShader DrawIndexed Draw Map Unmap PSSetConstantBuffers IASetInputLayout IASetVertexBuffers IASetIndexBuffer DrawIndexedInstanced DrawInstanced IASetPrimitiveTopology VSSetShaderResources VSSetSamplers OMSetRenderTargets OMSetBlendState OMSetDepthStencilState RSSetState RSSetViewports RSSetScissorRects CopySubresourceRegion CopyResource UpdateSubresource ClearRenderTargetView ClearDepthStencilView ClearState Flush GetType GetContextFlags VSGetShader PSGetShader VSGetConstantBuffers PSGetConstantBuffers VSGetShaderResources PSGetShaderResources VSGetSamplers PSGetSamplers IAGetInputLayout IAGetVertexBuffers IAGetIndexBuffer IAGetPrimitiveTopology OMGetRenderTargets OMGetBlendState OMGetDepthStencilState RSGetState RSGetViewports RSGetScissorRects",
    "ID3D11Resource": "GetType SetEvictionPriority GetEvictionPriority",
    "ID3D11View": "GetResource",
}
for t in ["ID3D11Buffer", "ID3D11Texture2D", "ID3D11RenderTargetView", "ID3D11DepthStencilView", "ID3D11ShaderResourceView", "ID3D11SamplerState", "ID3D11BlendState", "ID3D11DepthStencilState", "ID3D11RasterizerState"]:
    hand[t] = "GetDesc"
private = {"GetPrivateData", "SetPrivateData", "SetPrivateDataInterface"}
# Structs whose unserved void calls remove the device (the context, and the device itself).
removes = {"Device", "Context"}


def unused_stage(name, p, what):
    """The body of a context method on a stage or feature not served: served while it binds nothing."""
    if re.fullmatch(r"(GS|HS|DS|CS)SetShader|SetPredication", name):
        return f'self.only_unbinding("{what}", !{p[0]}.is_null())'
    if re.fullmatch(r"(GS|HS|DS|CS)Set(ConstantBuffers|ShaderResources|Samplers)|CSSetUnorderedAccessViews", name):
        return f'self.only_unbinding("{what}", super::context::binds({p[2]}, {p[1]}))'
    if name == "SOSetTargets":
        return f'self.only_unbinding("{what}", super::context::binds({p[1]}, {p[0]}))'
    if re.fullmatch(r"(GS|HS|DS|CS)Get(ConstantBuffers|ShaderResources|Samplers)|CSGetUnorderedAccessViews", name):
        return f"super::context::nothing_bound({p[2]}, {p[1]})"
    if name == "SOGetTargets":
        return f"super::context::nothing_bound({p[1]}, {p[0]})"
    if re.fullmatch(r"(GS|HS|DS|CS)GetShader", name):
        return f"super::context::no_instances({p[2]}); let _ = {p[0]}.write(None);"
    if name == "GetPredication":
        return (f"let _ = {p[0]}.write(None); "
                f"if let Some(v) = unsafe {{ {p[1]}.as_mut() }} {{ *v = false.into(); }}")
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
            unused = t == "ID3D11DeviceContext" and unused_stage(name, params, f"{t}::{name}")
            if unused:
                body = unused
            elif t == "ID3D11DeviceChild":
                body = f"self.child.{snake(name)}({args})"
            elif t in ("ID3D11Device", "IDXGIObject") and name in private:
                body = f"self.private.{snake(name)}({args})"
            elif name in hand.get(t, "").split():
                body = f"self.{snake(name)}({args})"
            elif ret.startswith("-> windows::core::Result"):
                body = f'Err(missing("{t}::{name}"))'
            elif struct in removes:
                body = f'self.unserved("{t}::{name}")' + ("" if not ret else "; Default::default()")
            elif not ret:
                body = f'missing_void("{t}::{name}")'
            else:
                body = f'missing_void("{t}::{name}"); Default::default()'
            out.append(f"    {sig} {{")
            out.append("        let _call = self.call();" if struct in removes else "        let _lock = lock();")
            out.append(f"        {body}")
            out.append("    }")
        out.append("}")
        out.append("")
open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "src", "com", "generated.rs"), "w").write(
    "\n".join(out) + "\n"
)
