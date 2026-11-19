# Writes com.rs from the windows crate's *_Impl traits: python3 com.py > com.rs
# Each entry of OBJECTS names an object, its interface and the methods objects.rs or present.rs do.
import re, glob, os
CARGO = os.environ.get('CARGO_HOME', os.path.expanduser('~/.cargo'))
W = glob.glob(f'{CARGO}/registry/src/*/windows-0.62.2/src/Windows/Win32/Graphics')[0]
src = {m: open(f'{W}/{m}/mod.rs').read() for m in ['Direct3D12', 'Dxgi', 'Direct3D']}

def trait(name):
    for mod, s in src.items():
        m = re.search(r'^pub trait %s_Impl: ([\w:]+) \{(\}|\n(.*?)^\})' % name, s, re.S | re.M)
        if m:
            parent = None if 'IUnknownImpl' in m.group(1) else m.group(1).split('::')[-1][:-len('_Impl')]
            body = m.group(3) or ''
            return mod, parent, [l.strip() for l in body.splitlines() if l.strip().startswith('fn ')]
    raise SystemExit('no trait ' + name)

def qualify(sig, mod):
    sig = sig.replace('super::super::Foundation::', 'windows::Win32::Foundation::')
    sig = sig.replace('super::Dxgi::Common::', 'windows::Win32::Graphics::Dxgi::Common::')
    sig = sig.replace('super::Direct3D::', 'windows::Win32::Graphics::Direct3D::')
    sig = sig.replace('super::Dxgi::', 'windows::Win32::Graphics::Dxgi::')
    sig = sig.replace('super::super::System::', 'windows::Win32::System::')
    sig = sig.replace('super::super::Security::', 'windows::Win32::Security::')
    sig = re.sub(r'super::Common::', 'windows::Win32::Graphics::Dxgi::Common::', sig)
    sig = re.sub(r'super::Gdi::', 'windows::Win32::Graphics::Gdi::', sig)
    return sig

def body(sig, kind):
    ret = sig.split('->')[-1].strip().rstrip(';') if '->' in sig else '()'
    name = re.match(r'fn (\w+)', sig).group(1)
    if ret.startswith('windows_core::Result'):
        return 'Err(self.unsupported("%s"))' % name
    if ret == 'windows_core::HRESULT':
        return 'self.unsupported("%s").code()' % name
    if ret == '()':
        return 'self.unsupported("%s");' % name
    # A plain value has no way to say "not implemented": objects.rs must answer it.
    raise SystemExit('%s returns %s: objects.rs must answer it' % (name, ret))

def args(sig):
    inner = sig[sig.index('(') + 1: sig.index(')')]
    parts = [p.strip() for p in inner.split(',')][1:]
    return ', '.join(p.split(':')[0] for p in parts if p)

OBJECTS = [
    'Factory:IDXGIFactory7:EnumWarpAdapter,RegisterAdaptersChangedEvent,UnregisterAdaptersChangedEvent,EnumAdapterByGpuPreference,CheckFeatureSupport,MakeWindowAssociation,GetWindowAssociation,EnumAdapters1,EnumAdapters,EnumAdapterByLuid,IsCurrent,GetCreationFlags,IsWindowedStereoEnabled,CreateSwapChainForHwnd,CreateSwapChain',
    'Adapter:IDXGIAdapter4:GetDesc3,GetDesc2,GetDesc1,GetDesc,EnumOutputs,GetParent',
    'Output:IDXGIOutput6:GetParent,GetDesc,GetDesc1,GetDisplayModeList,GetDisplayModeList1,FindClosestMatchingMode,FindClosestMatchingMode1,CheckHardwareCompositionSupport,SupportsOverlays',
    'SwapChain:IDXGISwapChain4:GetParent,GetContainingOutput,Present,Present1,GetBuffer,ResizeBuffers,GetDesc1,GetDesc,GetHwnd,GetDevice,GetCurrentBackBufferIndex,GetLastPresentCount,SetMaximumFrameLatency,GetMaximumFrameLatency,GetFrameLatencyWaitableObject,GetFullscreenState,SetFullscreenState,IsTemporaryMonoSupported,CheckColorSpaceSupport,SetColorSpace1,SetHDRMetaData',
    'Device:ID3D12Device8:CreatePipelineState,CreatePipelineLibrary,SetEventOnMultipleFenceCompletion,MakeResident,Evict,SetResidencyPriority,EnqueueMakeResident,CreateCommandList1,CreateCommittedResource1,CreateCommittedResource2,CreatePlacedResource1,CreateHeap1,GetCopyableFootprints1,EnumerateMetaCommands,EnumerateMetaCommandParameters,CreateMetaCommand,CheckDriverMatchingIdentifier,RemoveDevice,SetBackgroundProcessingMode,GetResourceAllocationInfo1,GetResourceAllocationInfo2,GetNodeCount,CreateCommandQueue,CreateCommandAllocator,CreateGraphicsPipelineState,CreateCommandList,CreateQueryHeap,CreateComputePipelineState,CheckFeatureSupport,CreateDescriptorHeap,GetDescriptorHandleIncrementSize,CreateRootSignature,CreateRenderTargetView,CreateDepthStencilView,CreateConstantBufferView,CreateShaderResourceView,CreateUnorderedAccessView,CreateSampler,CopyDescriptorsSimple,CopyDescriptors,CreateCommittedResource,CreateHeap,CreatePlacedResource,CreateCommandSignature,CreateFence,GetDeviceRemovedReason,GetCopyableFootprints,GetAdapterLuid,GetResourceAllocationInfo,GetCustomHeapProperties',
    'Queue:ID3D12CommandQueue:ExecuteCommandLists,Signal,Wait,GetTimestampFrequency,GetClockCalibration,GetDesc',
    'Allocator:ID3D12CommandAllocator:Reset',
    'List:ID3D12GraphicsCommandList6:BeginRenderPass,EndRenderPass,SetProtectedResourceSession,WriteBufferImmediate,Close,Reset,ExecuteBundle,DrawInstanced,DrawIndexedInstanced,CopyResource,DiscardResource,CopyBufferRegion,CopyTextureRegion,IASetPrimitiveTopology,RSSetViewports,RSSetScissorRects,OMSetBlendFactor,OMSetStencilRef,SetPipelineState,ResourceBarrier,SetGraphicsRootSignature,SetComputeRootSignature,SetDescriptorHeaps,SetGraphicsRootDescriptorTable,SetComputeRootDescriptorTable,SetGraphicsRootConstantBufferView,SetComputeRootConstantBufferView,SetGraphicsRootShaderResourceView,SetComputeRootShaderResourceView,SetGraphicsRootUnorderedAccessView,SetComputeRootUnorderedAccessView,SetGraphicsRoot32BitConstant,SetComputeRoot32BitConstant,SetGraphicsRoot32BitConstants,SetComputeRoot32BitConstants,IASetIndexBuffer,IASetVertexBuffers,SOSetTargets,OMSetRenderTargets,ClearRenderTargetView,ClearDepthStencilView,ClearUnorderedAccessViewUint,ClearUnorderedAccessViewFloat,Dispatch,ExecuteIndirect,ResolveSubresource,BeginQuery,EndQuery,ResolveQueryData,SetPredication,GetType',
    'Fence:ID3D12Fence1:GetCreationFlags,GetCompletedValue,SetEventOnCompletion,Signal',
    'Resource:ID3D12Resource2:GetDesc1,GetProtectedResourceSession,Map,Unmap,GetDesc,GetHeapProperties,GetGPUVirtualAddress,WriteToSubresource,ReadFromSubresource',
    'RootSignature:ID3D12RootSignature:',
    'PipelineState:ID3D12PipelineState:',
    'QueryHeap:ID3D12QueryHeap:',
    'CommandSignature:ID3D12CommandSignature:',
    'Heap:ID3D12Heap:GetDesc',
    'DescriptorHeap:ID3D12DescriptorHeap:GetDesc,GetCPUDescriptorHandleForHeapStart,GetGPUDescriptorHandleForHeapStart',
    'Blob:ID3DBlob:GetBufferPointer,GetBufferSize',
]

# Objects with a `Base` (base.rs), which answers these methods.
BASED = {'Device', 'Queue', 'Allocator', 'List', 'Fence', 'Resource', 'RootSignature', 'PipelineState', 'QueryHeap', 'CommandSignature', 'Heap', 'DescriptorHeap'}
BASE_METHODS = {'GetDevice', 'GetPrivateData', 'SetPrivateData', 'SetPrivateDataInterface', 'SetName'}

HEADER = '''//! The methods of each object's interfaces that the front does not do yet, made by com.py from the
//! `windows` crate's interface traits: each reports itself as not implemented (a command list's also
//! fails its `Close`), and the methods objects.rs and present.rs do are passed to them.

#![allow(clippy::too_many_arguments, unused_variables)]

use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::*;

use crate::base::HasBase;
use crate::objects::*;
use crate::present::*;
'''

out = [HEADER]
for spec in OBJECTS:
    ty, iface, hand = spec.split(':')
    hand = set(hand.split(',')) if hand else set()
    chain = []
    name = iface
    while name:
        mod, parent, methods = trait(name)
        chain.append((name, mod, methods))
        name = parent
    for name, mod, methods in chain:
        out.append('impl %s_Impl for %s_Impl {' % (name, ty))
        for sig in methods:
            sig = qualify(sig.rstrip(';'), mod)
            m = re.match(r'fn (\w+)', sig).group(1)
            if ty in BASED and m in BASE_METHODS:
                out.append('    %s {\n        self.base().%s(%s)\n    }' % (sig, m, args(sig)))
            elif m in hand:
                out.append('    %s {\n        %s::%s(self, %s)\n    }' % (sig, ty, m, args(sig)))
                hand.discard(m)
            else:
                stub = re.sub(r'([(,] )(\w+): ', lambda m: m.group(1) + '_' + m.group(2) + ': ', sig)
                out.append('    %s {\n        %s\n    }' % (stub, body(sig, ty)))
        out.append('}\n')
    if hand:
        raise SystemExit('not in %s: %s' % (iface, hand))
print('\n'.join(out))
