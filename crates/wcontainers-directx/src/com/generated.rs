// Written by a generator from the windows crate's (0.62.2) trait definitions: each COM method
// calls the hand-written method of the same name in snake case, or answers that it is not served.
#![allow(non_snake_case, unused_variables, clippy::too_many_arguments)]

use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::*;

use super::*;

impl ID3D11Device_Impl for Device_Impl {
    fn CreateBuffer(
        &self,
        pdesc: *const D3D11_BUFFER_DESC,
        pinitialdata: *const D3D11_SUBRESOURCE_DATA,
        ppbuffer: windows::core::OutRef<ID3D11Buffer>,
    ) -> windows::core::Result<()> {
        self.create_buffer(pdesc, pinitialdata, ppbuffer)
    }
    fn CreateTexture1D(
        &self,
        pdesc: *const D3D11_TEXTURE1D_DESC,
        pinitialdata: *const D3D11_SUBRESOURCE_DATA,
        pptexture1d: windows::core::OutRef<ID3D11Texture1D>,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11Device::CreateTexture1D"))
    }
    fn CreateTexture2D(
        &self,
        pdesc: *const D3D11_TEXTURE2D_DESC,
        pinitialdata: *const D3D11_SUBRESOURCE_DATA,
        pptexture2d: windows::core::OutRef<ID3D11Texture2D>,
    ) -> windows::core::Result<()> {
        self.create_texture2_d(pdesc, pinitialdata, pptexture2d)
    }
    fn CreateTexture3D(
        &self,
        pdesc: *const D3D11_TEXTURE3D_DESC,
        pinitialdata: *const D3D11_SUBRESOURCE_DATA,
        pptexture3d: windows::core::OutRef<ID3D11Texture3D>,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11Device::CreateTexture3D"))
    }
    fn CreateShaderResourceView(
        &self,
        presource: windows::core::Ref<ID3D11Resource>,
        pdesc: *const D3D11_SHADER_RESOURCE_VIEW_DESC,
        ppsrview: windows::core::OutRef<ID3D11ShaderResourceView>,
    ) -> windows::core::Result<()> {
        self.create_shader_resource_view(presource, pdesc, ppsrview)
    }
    fn CreateUnorderedAccessView(
        &self,
        presource: windows::core::Ref<ID3D11Resource>,
        pdesc: *const D3D11_UNORDERED_ACCESS_VIEW_DESC,
        ppuaview: windows::core::OutRef<ID3D11UnorderedAccessView>,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11Device::CreateUnorderedAccessView"))
    }
    fn CreateRenderTargetView(
        &self,
        presource: windows::core::Ref<ID3D11Resource>,
        pdesc: *const D3D11_RENDER_TARGET_VIEW_DESC,
        pprtview: windows::core::OutRef<ID3D11RenderTargetView>,
    ) -> windows::core::Result<()> {
        self.create_render_target_view(presource, pdesc, pprtview)
    }
    fn CreateDepthStencilView(
        &self,
        presource: windows::core::Ref<ID3D11Resource>,
        pdesc: *const D3D11_DEPTH_STENCIL_VIEW_DESC,
        ppdepthstencilview: windows::core::OutRef<ID3D11DepthStencilView>,
    ) -> windows::core::Result<()> {
        self.create_depth_stencil_view(presource, pdesc, ppdepthstencilview)
    }
    fn CreateInputLayout(
        &self,
        pinputelementdescs: *const D3D11_INPUT_ELEMENT_DESC,
        numelements: u32,
        pshaderbytecodewithinputsignature: *const core::ffi::c_void,
        bytecodelength: usize,
        ppinputlayout: windows::core::OutRef<ID3D11InputLayout>,
    ) -> windows::core::Result<()> {
        self.create_input_layout(
            pinputelementdescs,
            numelements,
            pshaderbytecodewithinputsignature,
            bytecodelength,
            ppinputlayout,
        )
    }
    fn CreateVertexShader(
        &self,
        pshaderbytecode: *const core::ffi::c_void,
        bytecodelength: usize,
        pclasslinkage: windows::core::Ref<ID3D11ClassLinkage>,
        ppvertexshader: windows::core::OutRef<ID3D11VertexShader>,
    ) -> windows::core::Result<()> {
        self.create_vertex_shader(
            pshaderbytecode,
            bytecodelength,
            pclasslinkage,
            ppvertexshader,
        )
    }
    fn CreateGeometryShader(
        &self,
        pshaderbytecode: *const core::ffi::c_void,
        bytecodelength: usize,
        pclasslinkage: windows::core::Ref<ID3D11ClassLinkage>,
        ppgeometryshader: windows::core::OutRef<ID3D11GeometryShader>,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11Device::CreateGeometryShader"))
    }
    fn CreateGeometryShaderWithStreamOutput(
        &self,
        pshaderbytecode: *const core::ffi::c_void,
        bytecodelength: usize,
        psodeclaration: *const D3D11_SO_DECLARATION_ENTRY,
        numentries: u32,
        pbufferstrides: *const u32,
        numstrides: u32,
        rasterizedstream: u32,
        pclasslinkage: windows::core::Ref<ID3D11ClassLinkage>,
        ppgeometryshader: windows::core::OutRef<ID3D11GeometryShader>,
    ) -> windows::core::Result<()> {
        Err(missing(
            "ID3D11Device::CreateGeometryShaderWithStreamOutput",
        ))
    }
    fn CreatePixelShader(
        &self,
        pshaderbytecode: *const core::ffi::c_void,
        bytecodelength: usize,
        pclasslinkage: windows::core::Ref<ID3D11ClassLinkage>,
        pppixelshader: windows::core::OutRef<ID3D11PixelShader>,
    ) -> windows::core::Result<()> {
        self.create_pixel_shader(
            pshaderbytecode,
            bytecodelength,
            pclasslinkage,
            pppixelshader,
        )
    }
    fn CreateHullShader(
        &self,
        pshaderbytecode: *const core::ffi::c_void,
        bytecodelength: usize,
        pclasslinkage: windows::core::Ref<ID3D11ClassLinkage>,
        pphullshader: windows::core::OutRef<ID3D11HullShader>,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11Device::CreateHullShader"))
    }
    fn CreateDomainShader(
        &self,
        pshaderbytecode: *const core::ffi::c_void,
        bytecodelength: usize,
        pclasslinkage: windows::core::Ref<ID3D11ClassLinkage>,
        ppdomainshader: windows::core::OutRef<ID3D11DomainShader>,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11Device::CreateDomainShader"))
    }
    fn CreateComputeShader(
        &self,
        pshaderbytecode: *const core::ffi::c_void,
        bytecodelength: usize,
        pclasslinkage: windows::core::Ref<ID3D11ClassLinkage>,
        ppcomputeshader: windows::core::OutRef<ID3D11ComputeShader>,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11Device::CreateComputeShader"))
    }
    fn CreateClassLinkage(&self) -> windows::core::Result<ID3D11ClassLinkage> {
        Err(missing("ID3D11Device::CreateClassLinkage"))
    }
    fn CreateBlendState(
        &self,
        pblendstatedesc: *const D3D11_BLEND_DESC,
        ppblendstate: windows::core::OutRef<ID3D11BlendState>,
    ) -> windows::core::Result<()> {
        self.create_blend_state(pblendstatedesc, ppblendstate)
    }
    fn CreateDepthStencilState(
        &self,
        pdepthstencildesc: *const D3D11_DEPTH_STENCIL_DESC,
        ppdepthstencilstate: windows::core::OutRef<ID3D11DepthStencilState>,
    ) -> windows::core::Result<()> {
        self.create_depth_stencil_state(pdepthstencildesc, ppdepthstencilstate)
    }
    fn CreateRasterizerState(
        &self,
        prasterizerdesc: *const D3D11_RASTERIZER_DESC,
        pprasterizerstate: windows::core::OutRef<ID3D11RasterizerState>,
    ) -> windows::core::Result<()> {
        self.create_rasterizer_state(prasterizerdesc, pprasterizerstate)
    }
    fn CreateSamplerState(
        &self,
        psamplerdesc: *const D3D11_SAMPLER_DESC,
        ppsamplerstate: windows::core::OutRef<ID3D11SamplerState>,
    ) -> windows::core::Result<()> {
        self.create_sampler_state(psamplerdesc, ppsamplerstate)
    }
    fn CreateQuery(
        &self,
        pquerydesc: *const D3D11_QUERY_DESC,
        ppquery: windows::core::OutRef<ID3D11Query>,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11Device::CreateQuery"))
    }
    fn CreatePredicate(
        &self,
        ppredicatedesc: *const D3D11_QUERY_DESC,
        pppredicate: windows::core::OutRef<ID3D11Predicate>,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11Device::CreatePredicate"))
    }
    fn CreateCounter(
        &self,
        pcounterdesc: *const D3D11_COUNTER_DESC,
        ppcounter: windows::core::OutRef<ID3D11Counter>,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11Device::CreateCounter"))
    }
    fn CreateDeferredContext(
        &self,
        contextflags: u32,
        ppdeferredcontext: windows::core::OutRef<ID3D11DeviceContext>,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11Device::CreateDeferredContext"))
    }
    fn OpenSharedResource(
        &self,
        hresource: windows::Win32::Foundation::HANDLE,
        returnedinterface: *const windows::core::GUID,
        ppresource: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11Device::OpenSharedResource"))
    }
    fn CheckFormatSupport(
        &self,
        format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
    ) -> windows::core::Result<u32> {
        Err(missing("ID3D11Device::CheckFormatSupport"))
    }
    fn CheckMultisampleQualityLevels(
        &self,
        format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
        samplecount: u32,
    ) -> windows::core::Result<u32> {
        Err(missing("ID3D11Device::CheckMultisampleQualityLevels"))
    }
    fn CheckCounterInfo(&self, pcounterinfo: *mut D3D11_COUNTER_INFO) {
        self.unserved("ID3D11Device::CheckCounterInfo")
    }
    fn CheckCounter(
        &self,
        pdesc: *const D3D11_COUNTER_DESC,
        ptype: *mut D3D11_COUNTER_TYPE,
        pactivecounters: *mut u32,
        szname: windows::core::PSTR,
        pnamelength: *mut u32,
        szunits: windows::core::PSTR,
        punitslength: *mut u32,
        szdescription: windows::core::PSTR,
        pdescriptionlength: *mut u32,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11Device::CheckCounter"))
    }
    fn CheckFeatureSupport(
        &self,
        feature: D3D11_FEATURE,
        pfeaturesupportdata: *mut core::ffi::c_void,
        featuresupportdatasize: u32,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11Device::CheckFeatureSupport"))
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.private.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.private.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.private.set_private_data_interface(guid, pdata)
    }
    fn GetFeatureLevel(&self) -> windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL {
        self.get_feature_level()
    }
    fn GetCreationFlags(&self) -> u32 {
        self.get_creation_flags()
    }
    fn GetDeviceRemovedReason(&self) -> windows::core::Result<()> {
        self.get_device_removed_reason()
    }
    fn GetImmediateContext(&self, ppimmediatecontext: windows::core::OutRef<ID3D11DeviceContext>) {
        self.get_immediate_context(ppimmediatecontext)
    }
    fn SetExceptionMode(&self, raiseflags: u32) -> windows::core::Result<()> {
        self.set_exception_mode(raiseflags)
    }
    fn GetExceptionMode(&self) -> u32 {
        self.get_exception_mode()
    }
}

impl IDXGIDevice_Impl for Device_Impl {
    fn GetAdapter(&self) -> windows::core::Result<IDXGIAdapter> {
        self.get_adapter()
    }
    fn CreateSurface(
        &self,
        pdesc: *const DXGI_SURFACE_DESC,
        numsurfaces: u32,
        usage: DXGI_USAGE,
        psharedresource: *const DXGI_SHARED_RESOURCE,
        ppsurface: *mut Option<IDXGISurface>,
    ) -> windows::core::Result<()> {
        Err(missing("IDXGIDevice::CreateSurface"))
    }
    fn QueryResourceResidency(
        &self,
        ppresources: *const Option<windows::core::IUnknown>,
        presidencystatus: *mut DXGI_RESIDENCY,
        numresources: u32,
    ) -> windows::core::Result<()> {
        Err(missing("IDXGIDevice::QueryResourceResidency"))
    }
    fn SetGPUThreadPriority(&self, priority: i32) -> windows::core::Result<()> {
        Err(missing("IDXGIDevice::SetGPUThreadPriority"))
    }
    fn GetGPUThreadPriority(&self) -> windows::core::Result<i32> {
        Err(missing("IDXGIDevice::GetGPUThreadPriority"))
    }
}

impl IDXGIObject_Impl for Device_Impl {
    fn SetPrivateData(
        &self,
        name: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.private.set_private_data(name, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        name: *const windows::core::GUID,
        punknown: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.private.set_private_data_interface(name, punknown)
    }
    fn GetPrivateData(
        &self,
        name: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.private.get_private_data(name, pdatasize, pdata)
    }
    fn GetParent(
        &self,
        riid: *const windows::core::GUID,
        ppparent: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.get_parent(riid, ppparent)
    }
}

impl ID3D11DeviceContext_Impl for Context_Impl {
    fn VSSetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
    ) {
        self.vs_set_constant_buffers(startslot, numbuffers, ppconstantbuffers)
    }
    fn PSSetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *const Option<ID3D11ShaderResourceView>,
    ) {
        self.ps_set_shader_resources(startslot, numviews, ppshaderresourceviews)
    }
    fn PSSetShader(
        &self,
        ppixelshader: windows::core::Ref<ID3D11PixelShader>,
        ppclassinstances: *const Option<ID3D11ClassInstance>,
        numclassinstances: u32,
    ) {
        self.ps_set_shader(ppixelshader, ppclassinstances, numclassinstances)
    }
    fn PSSetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *const Option<ID3D11SamplerState>,
    ) {
        self.ps_set_samplers(startslot, numsamplers, ppsamplers)
    }
    fn VSSetShader(
        &self,
        pvertexshader: windows::core::Ref<ID3D11VertexShader>,
        ppclassinstances: *const Option<ID3D11ClassInstance>,
        numclassinstances: u32,
    ) {
        self.vs_set_shader(pvertexshader, ppclassinstances, numclassinstances)
    }
    fn DrawIndexed(&self, indexcount: u32, startindexlocation: u32, basevertexlocation: i32) {
        self.draw_indexed(indexcount, startindexlocation, basevertexlocation)
    }
    fn Draw(&self, vertexcount: u32, startvertexlocation: u32) {
        self.draw(vertexcount, startvertexlocation)
    }
    fn Map(
        &self,
        presource: windows::core::Ref<ID3D11Resource>,
        subresource: u32,
        maptype: D3D11_MAP,
        mapflags: u32,
        pmappedresource: *mut D3D11_MAPPED_SUBRESOURCE,
    ) -> windows::core::Result<()> {
        self.map(presource, subresource, maptype, mapflags, pmappedresource)
    }
    fn Unmap(&self, presource: windows::core::Ref<ID3D11Resource>, subresource: u32) {
        self.unmap(presource, subresource)
    }
    fn PSSetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
    ) {
        self.ps_set_constant_buffers(startslot, numbuffers, ppconstantbuffers)
    }
    fn IASetInputLayout(&self, pinputlayout: windows::core::Ref<ID3D11InputLayout>) {
        self.ia_set_input_layout(pinputlayout)
    }
    fn IASetVertexBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppvertexbuffers: *const Option<ID3D11Buffer>,
        pstrides: *const u32,
        poffsets: *const u32,
    ) {
        self.ia_set_vertex_buffers(startslot, numbuffers, ppvertexbuffers, pstrides, poffsets)
    }
    fn IASetIndexBuffer(
        &self,
        pindexbuffer: windows::core::Ref<ID3D11Buffer>,
        format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
        offset: u32,
    ) {
        self.ia_set_index_buffer(pindexbuffer, format, offset)
    }
    fn DrawIndexedInstanced(
        &self,
        indexcountperinstance: u32,
        instancecount: u32,
        startindexlocation: u32,
        basevertexlocation: i32,
        startinstancelocation: u32,
    ) {
        self.draw_indexed_instanced(
            indexcountperinstance,
            instancecount,
            startindexlocation,
            basevertexlocation,
            startinstancelocation,
        )
    }
    fn DrawInstanced(
        &self,
        vertexcountperinstance: u32,
        instancecount: u32,
        startvertexlocation: u32,
        startinstancelocation: u32,
    ) {
        self.draw_instanced(
            vertexcountperinstance,
            instancecount,
            startvertexlocation,
            startinstancelocation,
        )
    }
    fn GSSetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
    ) {
        self.unserved("ID3D11DeviceContext::GSSetConstantBuffers")
    }
    fn GSSetShader(
        &self,
        pshader: windows::core::Ref<ID3D11GeometryShader>,
        ppclassinstances: *const Option<ID3D11ClassInstance>,
        numclassinstances: u32,
    ) {
        self.unserved("ID3D11DeviceContext::GSSetShader")
    }
    fn IASetPrimitiveTopology(
        &self,
        topology: windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY,
    ) {
        self.ia_set_primitive_topology(topology)
    }
    fn VSSetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *const Option<ID3D11ShaderResourceView>,
    ) {
        self.vs_set_shader_resources(startslot, numviews, ppshaderresourceviews)
    }
    fn VSSetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *const Option<ID3D11SamplerState>,
    ) {
        self.vs_set_samplers(startslot, numsamplers, ppsamplers)
    }
    fn Begin(&self, pasync: windows::core::Ref<ID3D11Asynchronous>) {
        self.unserved("ID3D11DeviceContext::Begin")
    }
    fn End(&self, pasync: windows::core::Ref<ID3D11Asynchronous>) {
        self.unserved("ID3D11DeviceContext::End")
    }
    fn GetData(
        &self,
        pasync: windows::core::Ref<ID3D11Asynchronous>,
        pdata: *mut core::ffi::c_void,
        datasize: u32,
        getdataflags: u32,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11DeviceContext::GetData"))
    }
    fn SetPredication(
        &self,
        ppredicate: windows::core::Ref<ID3D11Predicate>,
        predicatevalue: windows::core::BOOL,
    ) {
        self.unserved("ID3D11DeviceContext::SetPredication")
    }
    fn GSSetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *const Option<ID3D11ShaderResourceView>,
    ) {
        self.unserved("ID3D11DeviceContext::GSSetShaderResources")
    }
    fn GSSetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *const Option<ID3D11SamplerState>,
    ) {
        self.unserved("ID3D11DeviceContext::GSSetSamplers")
    }
    fn OMSetRenderTargets(
        &self,
        numviews: u32,
        pprendertargetviews: *const Option<ID3D11RenderTargetView>,
        pdepthstencilview: windows::core::Ref<ID3D11DepthStencilView>,
    ) {
        self.om_set_render_targets(numviews, pprendertargetviews, pdepthstencilview)
    }
    fn OMSetRenderTargetsAndUnorderedAccessViews(
        &self,
        numrtvs: u32,
        pprendertargetviews: *const Option<ID3D11RenderTargetView>,
        pdepthstencilview: windows::core::Ref<ID3D11DepthStencilView>,
        uavstartslot: u32,
        numuavs: u32,
        ppunorderedaccessviews: *const Option<ID3D11UnorderedAccessView>,
        puavinitialcounts: *const u32,
    ) {
        self.unserved("ID3D11DeviceContext::OMSetRenderTargetsAndUnorderedAccessViews")
    }
    fn OMSetBlendState(
        &self,
        pblendstate: windows::core::Ref<ID3D11BlendState>,
        blendfactor: *const f32,
        samplemask: u32,
    ) {
        self.om_set_blend_state(pblendstate, blendfactor, samplemask)
    }
    fn OMSetDepthStencilState(
        &self,
        pdepthstencilstate: windows::core::Ref<ID3D11DepthStencilState>,
        stencilref: u32,
    ) {
        self.om_set_depth_stencil_state(pdepthstencilstate, stencilref)
    }
    fn SOSetTargets(
        &self,
        numbuffers: u32,
        ppsotargets: *const Option<ID3D11Buffer>,
        poffsets: *const u32,
    ) {
        self.unserved("ID3D11DeviceContext::SOSetTargets")
    }
    fn DrawAuto(&self) {
        self.unserved("ID3D11DeviceContext::DrawAuto")
    }
    fn DrawIndexedInstancedIndirect(
        &self,
        pbufferforargs: windows::core::Ref<ID3D11Buffer>,
        alignedbyteoffsetforargs: u32,
    ) {
        self.unserved("ID3D11DeviceContext::DrawIndexedInstancedIndirect")
    }
    fn DrawInstancedIndirect(
        &self,
        pbufferforargs: windows::core::Ref<ID3D11Buffer>,
        alignedbyteoffsetforargs: u32,
    ) {
        self.unserved("ID3D11DeviceContext::DrawInstancedIndirect")
    }
    fn Dispatch(&self, threadgroupcountx: u32, threadgroupcounty: u32, threadgroupcountz: u32) {
        self.unserved("ID3D11DeviceContext::Dispatch")
    }
    fn DispatchIndirect(
        &self,
        pbufferforargs: windows::core::Ref<ID3D11Buffer>,
        alignedbyteoffsetforargs: u32,
    ) {
        self.unserved("ID3D11DeviceContext::DispatchIndirect")
    }
    fn RSSetState(&self, prasterizerstate: windows::core::Ref<ID3D11RasterizerState>) {
        self.rs_set_state(prasterizerstate)
    }
    fn RSSetViewports(&self, numviewports: u32, pviewports: *const D3D11_VIEWPORT) {
        self.rs_set_viewports(numviewports, pviewports)
    }
    fn RSSetScissorRects(&self, numrects: u32, prects: *const windows::Win32::Foundation::RECT) {
        self.rs_set_scissor_rects(numrects, prects)
    }
    fn CopySubresourceRegion(
        &self,
        pdstresource: windows::core::Ref<ID3D11Resource>,
        dstsubresource: u32,
        dstx: u32,
        dsty: u32,
        dstz: u32,
        psrcresource: windows::core::Ref<ID3D11Resource>,
        srcsubresource: u32,
        psrcbox: *const D3D11_BOX,
    ) {
        self.copy_subresource_region(
            pdstresource,
            dstsubresource,
            dstx,
            dsty,
            dstz,
            psrcresource,
            srcsubresource,
            psrcbox,
        )
    }
    fn CopyResource(
        &self,
        pdstresource: windows::core::Ref<ID3D11Resource>,
        psrcresource: windows::core::Ref<ID3D11Resource>,
    ) {
        self.copy_resource(pdstresource, psrcresource)
    }
    fn UpdateSubresource(
        &self,
        pdstresource: windows::core::Ref<ID3D11Resource>,
        dstsubresource: u32,
        pdstbox: *const D3D11_BOX,
        psrcdata: *const core::ffi::c_void,
        srcrowpitch: u32,
        srcdepthpitch: u32,
    ) {
        self.update_subresource(
            pdstresource,
            dstsubresource,
            pdstbox,
            psrcdata,
            srcrowpitch,
            srcdepthpitch,
        )
    }
    fn CopyStructureCount(
        &self,
        pdstbuffer: windows::core::Ref<ID3D11Buffer>,
        dstalignedbyteoffset: u32,
        psrcview: windows::core::Ref<ID3D11UnorderedAccessView>,
    ) {
        self.unserved("ID3D11DeviceContext::CopyStructureCount")
    }
    fn ClearRenderTargetView(
        &self,
        prendertargetview: windows::core::Ref<ID3D11RenderTargetView>,
        colorrgba: *const f32,
    ) {
        self.clear_render_target_view(prendertargetview, colorrgba)
    }
    fn ClearUnorderedAccessViewUint(
        &self,
        punorderedaccessview: windows::core::Ref<ID3D11UnorderedAccessView>,
        values: *const u32,
    ) {
        self.unserved("ID3D11DeviceContext::ClearUnorderedAccessViewUint")
    }
    fn ClearUnorderedAccessViewFloat(
        &self,
        punorderedaccessview: windows::core::Ref<ID3D11UnorderedAccessView>,
        values: *const f32,
    ) {
        self.unserved("ID3D11DeviceContext::ClearUnorderedAccessViewFloat")
    }
    fn ClearDepthStencilView(
        &self,
        pdepthstencilview: windows::core::Ref<ID3D11DepthStencilView>,
        clearflags: u32,
        depth: f32,
        stencil: u8,
    ) {
        self.clear_depth_stencil_view(pdepthstencilview, clearflags, depth, stencil)
    }
    fn GenerateMips(&self, pshaderresourceview: windows::core::Ref<ID3D11ShaderResourceView>) {
        self.unserved("ID3D11DeviceContext::GenerateMips")
    }
    fn SetResourceMinLOD(&self, presource: windows::core::Ref<ID3D11Resource>, minlod: f32) {
        self.unserved("ID3D11DeviceContext::SetResourceMinLOD")
    }
    fn GetResourceMinLOD(&self, presource: windows::core::Ref<ID3D11Resource>) -> f32 {
        self.unserved("ID3D11DeviceContext::GetResourceMinLOD");
        Default::default()
    }
    fn ResolveSubresource(
        &self,
        pdstresource: windows::core::Ref<ID3D11Resource>,
        dstsubresource: u32,
        psrcresource: windows::core::Ref<ID3D11Resource>,
        srcsubresource: u32,
        format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
    ) {
        self.unserved("ID3D11DeviceContext::ResolveSubresource")
    }
    fn ExecuteCommandList(
        &self,
        pcommandlist: windows::core::Ref<ID3D11CommandList>,
        restorecontextstate: windows::core::BOOL,
    ) {
        self.unserved("ID3D11DeviceContext::ExecuteCommandList")
    }
    fn HSSetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *const Option<ID3D11ShaderResourceView>,
    ) {
        self.unserved("ID3D11DeviceContext::HSSetShaderResources")
    }
    fn HSSetShader(
        &self,
        phullshader: windows::core::Ref<ID3D11HullShader>,
        ppclassinstances: *const Option<ID3D11ClassInstance>,
        numclassinstances: u32,
    ) {
        self.unserved("ID3D11DeviceContext::HSSetShader")
    }
    fn HSSetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *const Option<ID3D11SamplerState>,
    ) {
        self.unserved("ID3D11DeviceContext::HSSetSamplers")
    }
    fn HSSetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
    ) {
        self.unserved("ID3D11DeviceContext::HSSetConstantBuffers")
    }
    fn DSSetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *const Option<ID3D11ShaderResourceView>,
    ) {
        self.unserved("ID3D11DeviceContext::DSSetShaderResources")
    }
    fn DSSetShader(
        &self,
        pdomainshader: windows::core::Ref<ID3D11DomainShader>,
        ppclassinstances: *const Option<ID3D11ClassInstance>,
        numclassinstances: u32,
    ) {
        self.unserved("ID3D11DeviceContext::DSSetShader")
    }
    fn DSSetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *const Option<ID3D11SamplerState>,
    ) {
        self.unserved("ID3D11DeviceContext::DSSetSamplers")
    }
    fn DSSetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
    ) {
        self.unserved("ID3D11DeviceContext::DSSetConstantBuffers")
    }
    fn CSSetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *const Option<ID3D11ShaderResourceView>,
    ) {
        self.unserved("ID3D11DeviceContext::CSSetShaderResources")
    }
    fn CSSetUnorderedAccessViews(
        &self,
        startslot: u32,
        numuavs: u32,
        ppunorderedaccessviews: *const Option<ID3D11UnorderedAccessView>,
        puavinitialcounts: *const u32,
    ) {
        self.unserved("ID3D11DeviceContext::CSSetUnorderedAccessViews")
    }
    fn CSSetShader(
        &self,
        pcomputeshader: windows::core::Ref<ID3D11ComputeShader>,
        ppclassinstances: *const Option<ID3D11ClassInstance>,
        numclassinstances: u32,
    ) {
        self.unserved("ID3D11DeviceContext::CSSetShader")
    }
    fn CSSetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *const Option<ID3D11SamplerState>,
    ) {
        self.unserved("ID3D11DeviceContext::CSSetSamplers")
    }
    fn CSSetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
    ) {
        self.unserved("ID3D11DeviceContext::CSSetConstantBuffers")
    }
    fn VSGetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *mut Option<ID3D11Buffer>,
    ) {
        self.unserved("ID3D11DeviceContext::VSGetConstantBuffers")
    }
    fn PSGetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *mut Option<ID3D11ShaderResourceView>,
    ) {
        self.unserved("ID3D11DeviceContext::PSGetShaderResources")
    }
    fn PSGetShader(
        &self,
        pppixelshader: windows::core::OutRef<ID3D11PixelShader>,
        ppclassinstances: windows::core::OutRef<ID3D11ClassInstance>,
        pnumclassinstances: *mut u32,
    ) {
        self.unserved("ID3D11DeviceContext::PSGetShader")
    }
    fn PSGetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *mut Option<ID3D11SamplerState>,
    ) {
        self.unserved("ID3D11DeviceContext::PSGetSamplers")
    }
    fn VSGetShader(
        &self,
        ppvertexshader: windows::core::OutRef<ID3D11VertexShader>,
        ppclassinstances: windows::core::OutRef<ID3D11ClassInstance>,
        pnumclassinstances: *mut u32,
    ) {
        self.unserved("ID3D11DeviceContext::VSGetShader")
    }
    fn PSGetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *mut Option<ID3D11Buffer>,
    ) {
        self.unserved("ID3D11DeviceContext::PSGetConstantBuffers")
    }
    fn IAGetInputLayout(&self, ppinputlayout: windows::core::OutRef<ID3D11InputLayout>) {
        self.unserved("ID3D11DeviceContext::IAGetInputLayout")
    }
    fn IAGetVertexBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppvertexbuffers: windows::core::OutRef<ID3D11Buffer>,
        pstrides: *mut u32,
        poffsets: *mut u32,
    ) {
        self.unserved("ID3D11DeviceContext::IAGetVertexBuffers")
    }
    fn IAGetIndexBuffer(
        &self,
        pindexbuffer: windows::core::OutRef<ID3D11Buffer>,
        format: *mut windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
        offset: *mut u32,
    ) {
        self.unserved("ID3D11DeviceContext::IAGetIndexBuffer")
    }
    fn GSGetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *mut Option<ID3D11Buffer>,
    ) {
        self.unserved("ID3D11DeviceContext::GSGetConstantBuffers")
    }
    fn GSGetShader(
        &self,
        ppgeometryshader: windows::core::OutRef<ID3D11GeometryShader>,
        ppclassinstances: windows::core::OutRef<ID3D11ClassInstance>,
        pnumclassinstances: *mut u32,
    ) {
        self.unserved("ID3D11DeviceContext::GSGetShader")
    }
    fn IAGetPrimitiveTopology(
        &self,
        ptopology: *mut windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY,
    ) {
        self.unserved("ID3D11DeviceContext::IAGetPrimitiveTopology")
    }
    fn VSGetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *mut Option<ID3D11ShaderResourceView>,
    ) {
        self.unserved("ID3D11DeviceContext::VSGetShaderResources")
    }
    fn VSGetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *mut Option<ID3D11SamplerState>,
    ) {
        self.unserved("ID3D11DeviceContext::VSGetSamplers")
    }
    fn GetPredication(
        &self,
        pppredicate: windows::core::OutRef<ID3D11Predicate>,
        ppredicatevalue: *mut windows::core::BOOL,
    ) {
        self.unserved("ID3D11DeviceContext::GetPredication")
    }
    fn GSGetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *mut Option<ID3D11ShaderResourceView>,
    ) {
        self.unserved("ID3D11DeviceContext::GSGetShaderResources")
    }
    fn GSGetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *mut Option<ID3D11SamplerState>,
    ) {
        self.unserved("ID3D11DeviceContext::GSGetSamplers")
    }
    fn OMGetRenderTargets(
        &self,
        numviews: u32,
        pprendertargetviews: *mut Option<ID3D11RenderTargetView>,
        ppdepthstencilview: windows::core::OutRef<ID3D11DepthStencilView>,
    ) {
        self.unserved("ID3D11DeviceContext::OMGetRenderTargets")
    }
    fn OMGetRenderTargetsAndUnorderedAccessViews(
        &self,
        numrtvs: u32,
        pprendertargetviews: *mut Option<ID3D11RenderTargetView>,
        ppdepthstencilview: windows::core::OutRef<ID3D11DepthStencilView>,
        uavstartslot: u32,
        numuavs: u32,
        ppunorderedaccessviews: *mut Option<ID3D11UnorderedAccessView>,
    ) {
        self.unserved("ID3D11DeviceContext::OMGetRenderTargetsAndUnorderedAccessViews")
    }
    fn OMGetBlendState(
        &self,
        ppblendstate: windows::core::OutRef<ID3D11BlendState>,
        blendfactor: *mut f32,
        psamplemask: *mut u32,
    ) {
        self.unserved("ID3D11DeviceContext::OMGetBlendState")
    }
    fn OMGetDepthStencilState(
        &self,
        ppdepthstencilstate: windows::core::OutRef<ID3D11DepthStencilState>,
        pstencilref: *mut u32,
    ) {
        self.unserved("ID3D11DeviceContext::OMGetDepthStencilState")
    }
    fn SOGetTargets(&self, numbuffers: u32, ppsotargets: *mut Option<ID3D11Buffer>) {
        self.unserved("ID3D11DeviceContext::SOGetTargets")
    }
    fn RSGetState(&self, pprasterizerstate: windows::core::OutRef<ID3D11RasterizerState>) {
        self.unserved("ID3D11DeviceContext::RSGetState")
    }
    fn RSGetViewports(&self, pnumviewports: *mut u32, pviewports: *mut D3D11_VIEWPORT) {
        self.unserved("ID3D11DeviceContext::RSGetViewports")
    }
    fn RSGetScissorRects(
        &self,
        pnumrects: *mut u32,
        prects: *mut windows::Win32::Foundation::RECT,
    ) {
        self.unserved("ID3D11DeviceContext::RSGetScissorRects")
    }
    fn HSGetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *mut Option<ID3D11ShaderResourceView>,
    ) {
        self.unserved("ID3D11DeviceContext::HSGetShaderResources")
    }
    fn HSGetShader(
        &self,
        pphullshader: windows::core::OutRef<ID3D11HullShader>,
        ppclassinstances: windows::core::OutRef<ID3D11ClassInstance>,
        pnumclassinstances: *mut u32,
    ) {
        self.unserved("ID3D11DeviceContext::HSGetShader")
    }
    fn HSGetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *mut Option<ID3D11SamplerState>,
    ) {
        self.unserved("ID3D11DeviceContext::HSGetSamplers")
    }
    fn HSGetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *mut Option<ID3D11Buffer>,
    ) {
        self.unserved("ID3D11DeviceContext::HSGetConstantBuffers")
    }
    fn DSGetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *mut Option<ID3D11ShaderResourceView>,
    ) {
        self.unserved("ID3D11DeviceContext::DSGetShaderResources")
    }
    fn DSGetShader(
        &self,
        ppdomainshader: windows::core::OutRef<ID3D11DomainShader>,
        ppclassinstances: windows::core::OutRef<ID3D11ClassInstance>,
        pnumclassinstances: *mut u32,
    ) {
        self.unserved("ID3D11DeviceContext::DSGetShader")
    }
    fn DSGetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *mut Option<ID3D11SamplerState>,
    ) {
        self.unserved("ID3D11DeviceContext::DSGetSamplers")
    }
    fn DSGetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *mut Option<ID3D11Buffer>,
    ) {
        self.unserved("ID3D11DeviceContext::DSGetConstantBuffers")
    }
    fn CSGetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *mut Option<ID3D11ShaderResourceView>,
    ) {
        self.unserved("ID3D11DeviceContext::CSGetShaderResources")
    }
    fn CSGetUnorderedAccessViews(
        &self,
        startslot: u32,
        numuavs: u32,
        ppunorderedaccessviews: *mut Option<ID3D11UnorderedAccessView>,
    ) {
        self.unserved("ID3D11DeviceContext::CSGetUnorderedAccessViews")
    }
    fn CSGetShader(
        &self,
        ppcomputeshader: windows::core::OutRef<ID3D11ComputeShader>,
        ppclassinstances: windows::core::OutRef<ID3D11ClassInstance>,
        pnumclassinstances: *mut u32,
    ) {
        self.unserved("ID3D11DeviceContext::CSGetShader")
    }
    fn CSGetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *mut Option<ID3D11SamplerState>,
    ) {
        self.unserved("ID3D11DeviceContext::CSGetSamplers")
    }
    fn CSGetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *mut Option<ID3D11Buffer>,
    ) {
        self.unserved("ID3D11DeviceContext::CSGetConstantBuffers")
    }
    fn ClearState(&self) {
        self.clear_state()
    }
    fn Flush(&self) {
        self.flush()
    }
    fn GetType(&self) -> D3D11_DEVICE_CONTEXT_TYPE {
        self.get_type()
    }
    fn GetContextFlags(&self) -> u32 {
        self.get_context_flags()
    }
    fn FinishCommandList(
        &self,
        restoredeferredcontextstate: windows::core::BOOL,
        ppcommandlist: windows::core::OutRef<ID3D11CommandList>,
    ) -> windows::core::Result<()> {
        Err(missing("ID3D11DeviceContext::FinishCommandList"))
    }
}

impl ID3D11DeviceChild_Impl for Context_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl IDXGIAdapter1_Impl for Adapter_Impl {
    fn GetDesc1(&self) -> windows::core::Result<DXGI_ADAPTER_DESC1> {
        self.get_desc1()
    }
}

impl IDXGIAdapter_Impl for Adapter_Impl {
    fn EnumOutputs(&self, output: u32) -> windows::core::Result<IDXGIOutput> {
        self.enum_outputs(output)
    }
    fn GetDesc(&self) -> windows::core::Result<DXGI_ADAPTER_DESC> {
        self.get_desc()
    }
    fn CheckInterfaceSupport(
        &self,
        interfacename: *const windows::core::GUID,
    ) -> windows::core::Result<i64> {
        Err(missing("IDXGIAdapter::CheckInterfaceSupport"))
    }
}

impl IDXGIObject_Impl for Adapter_Impl {
    fn SetPrivateData(
        &self,
        name: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.private.set_private_data(name, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        name: *const windows::core::GUID,
        punknown: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.private.set_private_data_interface(name, punknown)
    }
    fn GetPrivateData(
        &self,
        name: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.private.get_private_data(name, pdatasize, pdata)
    }
    fn GetParent(
        &self,
        riid: *const windows::core::GUID,
        ppparent: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.get_parent(riid, ppparent)
    }
}

impl ID3D11Buffer_Impl for Buffer_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_BUFFER_DESC) {
        self.get_desc(pdesc)
    }
}

impl ID3D11Resource_Impl for Buffer_Impl {
    fn GetType(&self, presourcedimension: *mut D3D11_RESOURCE_DIMENSION) {
        self.get_type(presourcedimension)
    }
    fn SetEvictionPriority(&self, evictionpriority: u32) {
        self.set_eviction_priority(evictionpriority)
    }
    fn GetEvictionPriority(&self) -> u32 {
        self.get_eviction_priority()
    }
}

impl ID3D11DeviceChild_Impl for Buffer_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11Texture2D_Impl for Texture2D_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_TEXTURE2D_DESC) {
        self.get_desc(pdesc)
    }
}

impl ID3D11Resource_Impl for Texture2D_Impl {
    fn GetType(&self, presourcedimension: *mut D3D11_RESOURCE_DIMENSION) {
        self.get_type(presourcedimension)
    }
    fn SetEvictionPriority(&self, evictionpriority: u32) {
        self.set_eviction_priority(evictionpriority)
    }
    fn GetEvictionPriority(&self) -> u32 {
        self.get_eviction_priority()
    }
}

impl ID3D11DeviceChild_Impl for Texture2D_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11RenderTargetView_Impl for RenderTargetView_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_RENDER_TARGET_VIEW_DESC) {
        self.get_desc(pdesc)
    }
}

impl ID3D11View_Impl for RenderTargetView_Impl {
    fn GetResource(&self, ppresource: windows::core::OutRef<ID3D11Resource>) {
        self.get_resource(ppresource)
    }
}

impl ID3D11DeviceChild_Impl for RenderTargetView_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11DepthStencilView_Impl for DepthStencilView_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_DEPTH_STENCIL_VIEW_DESC) {
        self.get_desc(pdesc)
    }
}

impl ID3D11View_Impl for DepthStencilView_Impl {
    fn GetResource(&self, ppresource: windows::core::OutRef<ID3D11Resource>) {
        self.get_resource(ppresource)
    }
}

impl ID3D11DeviceChild_Impl for DepthStencilView_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11ShaderResourceView_Impl for ShaderResourceView_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_SHADER_RESOURCE_VIEW_DESC) {
        self.get_desc(pdesc)
    }
}

impl ID3D11View_Impl for ShaderResourceView_Impl {
    fn GetResource(&self, ppresource: windows::core::OutRef<ID3D11Resource>) {
        self.get_resource(ppresource)
    }
}

impl ID3D11DeviceChild_Impl for ShaderResourceView_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11SamplerState_Impl for SamplerState_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_SAMPLER_DESC) {
        self.get_desc(pdesc)
    }
}

impl ID3D11DeviceChild_Impl for SamplerState_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11BlendState_Impl for BlendState_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_BLEND_DESC) {
        self.get_desc(pdesc)
    }
}

impl ID3D11DeviceChild_Impl for BlendState_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11DepthStencilState_Impl for DepthStencilState_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_DEPTH_STENCIL_DESC) {
        self.get_desc(pdesc)
    }
}

impl ID3D11DeviceChild_Impl for DepthStencilState_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11RasterizerState_Impl for RasterizerState_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_RASTERIZER_DESC) {
        self.get_desc(pdesc)
    }
}

impl ID3D11DeviceChild_Impl for RasterizerState_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11InputLayout_Impl for InputLayout_Impl {}

impl ID3D11DeviceChild_Impl for InputLayout_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11VertexShader_Impl for VertexShader_Impl {}

impl ID3D11DeviceChild_Impl for VertexShader_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11PixelShader_Impl for PixelShader_Impl {}

impl ID3D11DeviceChild_Impl for PixelShader_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        self.child.set_private_data_interface(guid, pdata)
    }
}
