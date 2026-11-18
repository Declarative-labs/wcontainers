// Written by a generator from the windows crate's (0.62.2) trait definitions: each COM method
// calls the hand-written method of the same name in snake case, or answers that it is not served,
// under the library's lock.
#![allow(non_snake_case, unused_variables, clippy::too_many_arguments)]

use windows::Win32::Graphics::Direct3D10::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::*;

use super::*;

impl ID3D11Device3_Impl for Device_Impl {
    fn CreateTexture2D1(
        &self,
        pdesc1: *const D3D11_TEXTURE2D_DESC1,
        pinitialdata: *const D3D11_SUBRESOURCE_DATA,
        pptexture2d: windows::core::OutRef<ID3D11Texture2D1>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(pptexture2d, |pptexture2d| {
            self.create_texture2_d1(pdesc1, pinitialdata, pptexture2d)
        })
    }
    fn CreateTexture3D1(
        &self,
        pdesc1: *const D3D11_TEXTURE3D_DESC1,
        pinitialdata: *const D3D11_SUBRESOURCE_DATA,
        pptexture3d: windows::core::OutRef<ID3D11Texture3D1>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(pptexture3d, |pptexture3d| {
            self.create_texture3_d1(pdesc1, pinitialdata, pptexture3d)
        })
    }
    fn CreateRasterizerState2(
        &self,
        prasterizerdesc: *const D3D11_RASTERIZER_DESC2,
        pprasterizerstate: windows::core::OutRef<ID3D11RasterizerState2>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(pprasterizerstate, |pprasterizerstate| {
            self.create_rasterizer_state2(prasterizerdesc, pprasterizerstate)
        })
    }
    fn CreateShaderResourceView1(
        &self,
        presource: windows::core::Ref<ID3D11Resource>,
        pdesc1: *const D3D11_SHADER_RESOURCE_VIEW_DESC1,
        ppsrview1: windows::core::OutRef<ID3D11ShaderResourceView1>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppsrview1, |ppsrview1| {
            self.create_shader_resource_view1(presource, pdesc1, ppsrview1)
        })
    }
    fn CreateUnorderedAccessView1(
        &self,
        presource: windows::core::Ref<ID3D11Resource>,
        pdesc1: *const D3D11_UNORDERED_ACCESS_VIEW_DESC1,
        ppuaview1: windows::core::OutRef<ID3D11UnorderedAccessView1>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppuaview1, |ppuaview1| {
            self.create_unordered_access_view1(presource, pdesc1, ppuaview1)
        })
    }
    fn CreateRenderTargetView1(
        &self,
        presource: windows::core::Ref<ID3D11Resource>,
        pdesc1: *const D3D11_RENDER_TARGET_VIEW_DESC1,
        pprtview1: windows::core::OutRef<ID3D11RenderTargetView1>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(pprtview1, |pprtview1| {
            self.create_render_target_view1(presource, pdesc1, pprtview1)
        })
    }
    fn CreateQuery1(
        &self,
        pquerydesc1: *const D3D11_QUERY_DESC1,
        ppquery1: windows::core::OutRef<ID3D11Query1>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppquery1, |ppquery1| {
            self.create_query1(pquerydesc1, ppquery1)
        })
    }
    fn GetImmediateContext3(
        &self,
        ppimmediatecontext: windows::core::OutRef<ID3D11DeviceContext3>,
    ) {
        let _call = self.call();
        self.get_immediate_context3(ppimmediatecontext)
    }
    fn CreateDeferredContext3(
        &self,
        contextflags: u32,
        ppdeferredcontext: windows::core::OutRef<ID3D11DeviceContext3>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppdeferredcontext, |ppdeferredcontext| {
            self.create_deferred_context3(contextflags, ppdeferredcontext)
        })
    }
    fn WriteToSubresource(
        &self,
        pdstresource: windows::core::Ref<ID3D11Resource>,
        dstsubresource: u32,
        pdstbox: *const D3D11_BOX,
        psrcdata: *const core::ffi::c_void,
        srcrowpitch: u32,
        srcdepthpitch: u32,
    ) {
        let _call = self.call();
        self.unserved("ID3D11Device3::WriteToSubresource")
    }
    fn ReadFromSubresource(
        &self,
        pdstdata: *mut core::ffi::c_void,
        dstrowpitch: u32,
        dstdepthpitch: u32,
        psrcresource: windows::core::Ref<ID3D11Resource>,
        srcsubresource: u32,
        psrcbox: *const D3D11_BOX,
    ) {
        let _call = self.call();
        self.unserved("ID3D11Device3::ReadFromSubresource")
    }
}

impl ID3D11Device2_Impl for Device_Impl {
    fn GetImmediateContext2(
        &self,
        ppimmediatecontext: windows::core::OutRef<ID3D11DeviceContext2>,
    ) {
        let _call = self.call();
        self.get_immediate_context2(ppimmediatecontext)
    }
    fn CreateDeferredContext2(
        &self,
        contextflags: u32,
        ppdeferredcontext: windows::core::OutRef<ID3D11DeviceContext2>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppdeferredcontext, |ppdeferredcontext| {
            self.create_deferred_context2(contextflags, ppdeferredcontext)
        })
    }
    fn GetResourceTiling(
        &self,
        ptiledresource: windows::core::Ref<ID3D11Resource>,
        pnumtilesforentireresource: *mut u32,
        ppackedmipdesc: *mut D3D11_PACKED_MIP_DESC,
        pstandardtileshapefornonpackedmips: *mut D3D11_TILE_SHAPE,
        pnumsubresourcetilings: *mut u32,
        firstsubresourcetilingtoget: u32,
        psubresourcetilingsfornonpackedmips: *mut D3D11_SUBRESOURCE_TILING,
    ) {
        let _call = self.call();
        self.unserved("ID3D11Device2::GetResourceTiling")
    }
    fn CheckMultisampleQualityLevels1(
        &self,
        format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
        samplecount: u32,
        flags: u32,
    ) -> windows::core::Result<u32> {
        let _call = self.call();
        self.check_multisample_quality_levels1(format, samplecount, flags)
    }
}

impl ID3D11Device1_Impl for Device_Impl {
    fn GetImmediateContext1(
        &self,
        ppimmediatecontext: windows::core::OutRef<ID3D11DeviceContext1>,
    ) {
        let _call = self.call();
        self.get_immediate_context1(ppimmediatecontext)
    }
    fn CreateDeferredContext1(
        &self,
        contextflags: u32,
        ppdeferredcontext: windows::core::OutRef<ID3D11DeviceContext1>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppdeferredcontext, |ppdeferredcontext| {
            self.create_deferred_context1(contextflags, ppdeferredcontext)
        })
    }
    fn CreateBlendState1(
        &self,
        pblendstatedesc: *const D3D11_BLEND_DESC1,
        ppblendstate: windows::core::OutRef<ID3D11BlendState1>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppblendstate, |ppblendstate| {
            self.create_blend_state1(pblendstatedesc, ppblendstate)
        })
    }
    fn CreateRasterizerState1(
        &self,
        prasterizerdesc: *const D3D11_RASTERIZER_DESC1,
        pprasterizerstate: windows::core::OutRef<ID3D11RasterizerState1>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(pprasterizerstate, |pprasterizerstate| {
            self.create_rasterizer_state1(prasterizerdesc, pprasterizerstate)
        })
    }
    fn CreateDeviceContextState(
        &self,
        flags: u32,
        pfeaturelevels: *const windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL,
        featurelevels: u32,
        sdkversion: u32,
        emulatedinterface: *const windows::core::GUID,
        pchosenfeaturelevel: *mut windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL,
        ppcontextstate: windows::core::OutRef<ID3DDeviceContextState>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppcontextstate, |ppcontextstate| {
            self.create_device_context_state(
                flags,
                pfeaturelevels,
                featurelevels,
                sdkversion,
                emulatedinterface,
                pchosenfeaturelevel,
                ppcontextstate,
            )
        })
    }
    fn OpenSharedResource1(
        &self,
        hresource: windows::Win32::Foundation::HANDLE,
        returnedinterface: *const windows::core::GUID,
        ppresource: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        Err(missing("ID3D11Device1::OpenSharedResource1"))
    }
    fn OpenSharedResourceByName(
        &self,
        lpname: &windows::core::PCWSTR,
        dwdesiredaccess: u32,
        returnedinterface: *const windows::core::GUID,
        ppresource: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        Err(missing("ID3D11Device1::OpenSharedResourceByName"))
    }
}

impl ID3D11Device_Impl for Device_Impl {
    fn CreateBuffer(
        &self,
        pdesc: *const D3D11_BUFFER_DESC,
        pinitialdata: *const D3D11_SUBRESOURCE_DATA,
        ppbuffer: windows::core::OutRef<ID3D11Buffer>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppbuffer, |ppbuffer| {
            self.create_buffer(pdesc, pinitialdata, ppbuffer)
        })
    }
    fn CreateTexture1D(
        &self,
        pdesc: *const D3D11_TEXTURE1D_DESC,
        pinitialdata: *const D3D11_SUBRESOURCE_DATA,
        pptexture1d: windows::core::OutRef<ID3D11Texture1D>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(pptexture1d, |pptexture1d| {
            self.create_texture1_d(pdesc, pinitialdata, pptexture1d)
        })
    }
    fn CreateTexture2D(
        &self,
        pdesc: *const D3D11_TEXTURE2D_DESC,
        pinitialdata: *const D3D11_SUBRESOURCE_DATA,
        pptexture2d: windows::core::OutRef<ID3D11Texture2D>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(pptexture2d, |pptexture2d| {
            self.create_texture2_d(pdesc, pinitialdata, pptexture2d)
        })
    }
    fn CreateTexture3D(
        &self,
        pdesc: *const D3D11_TEXTURE3D_DESC,
        pinitialdata: *const D3D11_SUBRESOURCE_DATA,
        pptexture3d: windows::core::OutRef<ID3D11Texture3D>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(pptexture3d, |pptexture3d| {
            self.create_texture3_d(pdesc, pinitialdata, pptexture3d)
        })
    }
    fn CreateShaderResourceView(
        &self,
        presource: windows::core::Ref<ID3D11Resource>,
        pdesc: *const D3D11_SHADER_RESOURCE_VIEW_DESC,
        ppsrview: windows::core::OutRef<ID3D11ShaderResourceView>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppsrview, |ppsrview| {
            self.create_shader_resource_view(presource, pdesc, ppsrview)
        })
    }
    fn CreateUnorderedAccessView(
        &self,
        presource: windows::core::Ref<ID3D11Resource>,
        pdesc: *const D3D11_UNORDERED_ACCESS_VIEW_DESC,
        ppuaview: windows::core::OutRef<ID3D11UnorderedAccessView>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppuaview, |ppuaview| {
            self.create_unordered_access_view(presource, pdesc, ppuaview)
        })
    }
    fn CreateRenderTargetView(
        &self,
        presource: windows::core::Ref<ID3D11Resource>,
        pdesc: *const D3D11_RENDER_TARGET_VIEW_DESC,
        pprtview: windows::core::OutRef<ID3D11RenderTargetView>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(pprtview, |pprtview| {
            self.create_render_target_view(presource, pdesc, pprtview)
        })
    }
    fn CreateDepthStencilView(
        &self,
        presource: windows::core::Ref<ID3D11Resource>,
        pdesc: *const D3D11_DEPTH_STENCIL_VIEW_DESC,
        ppdepthstencilview: windows::core::OutRef<ID3D11DepthStencilView>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppdepthstencilview, |ppdepthstencilview| {
            self.create_depth_stencil_view(presource, pdesc, ppdepthstencilview)
        })
    }
    fn CreateInputLayout(
        &self,
        pinputelementdescs: *const D3D11_INPUT_ELEMENT_DESC,
        numelements: u32,
        pshaderbytecodewithinputsignature: *const core::ffi::c_void,
        bytecodelength: usize,
        ppinputlayout: windows::core::OutRef<ID3D11InputLayout>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppinputlayout, |ppinputlayout| {
            self.create_input_layout(
                pinputelementdescs,
                numelements,
                pshaderbytecodewithinputsignature,
                bytecodelength,
                ppinputlayout,
            )
        })
    }
    fn CreateVertexShader(
        &self,
        pshaderbytecode: *const core::ffi::c_void,
        bytecodelength: usize,
        pclasslinkage: windows::core::Ref<ID3D11ClassLinkage>,
        ppvertexshader: windows::core::OutRef<ID3D11VertexShader>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppvertexshader, |ppvertexshader| {
            self.create_vertex_shader(
                pshaderbytecode,
                bytecodelength,
                pclasslinkage,
                ppvertexshader,
            )
        })
    }
    fn CreateGeometryShader(
        &self,
        pshaderbytecode: *const core::ffi::c_void,
        bytecodelength: usize,
        pclasslinkage: windows::core::Ref<ID3D11ClassLinkage>,
        ppgeometryshader: windows::core::OutRef<ID3D11GeometryShader>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppgeometryshader, |ppgeometryshader| {
            self.create_geometry_shader(
                pshaderbytecode,
                bytecodelength,
                pclasslinkage,
                ppgeometryshader,
            )
        })
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
        let _call = self.call();
        creating(ppgeometryshader, |ppgeometryshader| {
            self.create_geometry_shader_with_stream_output(
                pshaderbytecode,
                bytecodelength,
                psodeclaration,
                numentries,
                pbufferstrides,
                numstrides,
                rasterizedstream,
                pclasslinkage,
                ppgeometryshader,
            )
        })
    }
    fn CreatePixelShader(
        &self,
        pshaderbytecode: *const core::ffi::c_void,
        bytecodelength: usize,
        pclasslinkage: windows::core::Ref<ID3D11ClassLinkage>,
        pppixelshader: windows::core::OutRef<ID3D11PixelShader>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(pppixelshader, |pppixelshader| {
            self.create_pixel_shader(
                pshaderbytecode,
                bytecodelength,
                pclasslinkage,
                pppixelshader,
            )
        })
    }
    fn CreateHullShader(
        &self,
        pshaderbytecode: *const core::ffi::c_void,
        bytecodelength: usize,
        pclasslinkage: windows::core::Ref<ID3D11ClassLinkage>,
        pphullshader: windows::core::OutRef<ID3D11HullShader>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(pphullshader, |pphullshader| {
            self.create_hull_shader(pshaderbytecode, bytecodelength, pclasslinkage, pphullshader)
        })
    }
    fn CreateDomainShader(
        &self,
        pshaderbytecode: *const core::ffi::c_void,
        bytecodelength: usize,
        pclasslinkage: windows::core::Ref<ID3D11ClassLinkage>,
        ppdomainshader: windows::core::OutRef<ID3D11DomainShader>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppdomainshader, |ppdomainshader| {
            self.create_domain_shader(
                pshaderbytecode,
                bytecodelength,
                pclasslinkage,
                ppdomainshader,
            )
        })
    }
    fn CreateComputeShader(
        &self,
        pshaderbytecode: *const core::ffi::c_void,
        bytecodelength: usize,
        pclasslinkage: windows::core::Ref<ID3D11ClassLinkage>,
        ppcomputeshader: windows::core::OutRef<ID3D11ComputeShader>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppcomputeshader, |ppcomputeshader| {
            self.create_compute_shader(
                pshaderbytecode,
                bytecodelength,
                pclasslinkage,
                ppcomputeshader,
            )
        })
    }
    fn CreateClassLinkage(&self) -> windows::core::Result<ID3D11ClassLinkage> {
        let _call = self.call();
        Err(missing("ID3D11Device::CreateClassLinkage"))
    }
    fn CreateBlendState(
        &self,
        pblendstatedesc: *const D3D11_BLEND_DESC,
        ppblendstate: windows::core::OutRef<ID3D11BlendState>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppblendstate, |ppblendstate| {
            self.create_blend_state(pblendstatedesc, ppblendstate)
        })
    }
    fn CreateDepthStencilState(
        &self,
        pdepthstencildesc: *const D3D11_DEPTH_STENCIL_DESC,
        ppdepthstencilstate: windows::core::OutRef<ID3D11DepthStencilState>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppdepthstencilstate, |ppdepthstencilstate| {
            self.create_depth_stencil_state(pdepthstencildesc, ppdepthstencilstate)
        })
    }
    fn CreateRasterizerState(
        &self,
        prasterizerdesc: *const D3D11_RASTERIZER_DESC,
        pprasterizerstate: windows::core::OutRef<ID3D11RasterizerState>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(pprasterizerstate, |pprasterizerstate| {
            self.create_rasterizer_state(prasterizerdesc, pprasterizerstate)
        })
    }
    fn CreateSamplerState(
        &self,
        psamplerdesc: *const D3D11_SAMPLER_DESC,
        ppsamplerstate: windows::core::OutRef<ID3D11SamplerState>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppsamplerstate, |ppsamplerstate| {
            self.create_sampler_state(psamplerdesc, ppsamplerstate)
        })
    }
    fn CreateQuery(
        &self,
        pquerydesc: *const D3D11_QUERY_DESC,
        ppquery: windows::core::OutRef<ID3D11Query>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppquery, |ppquery| self.create_query(pquerydesc, ppquery))
    }
    fn CreatePredicate(
        &self,
        ppredicatedesc: *const D3D11_QUERY_DESC,
        pppredicate: windows::core::OutRef<ID3D11Predicate>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(pppredicate, |pppredicate| {
            self.create_predicate(ppredicatedesc, pppredicate)
        })
    }
    fn CreateCounter(
        &self,
        pcounterdesc: *const D3D11_COUNTER_DESC,
        ppcounter: windows::core::OutRef<ID3D11Counter>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppcounter, |ppcounter| {
            Err(missing("ID3D11Device::CreateCounter"))
        })
    }
    fn CreateDeferredContext(
        &self,
        contextflags: u32,
        ppdeferredcontext: windows::core::OutRef<ID3D11DeviceContext>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        creating(ppdeferredcontext, |ppdeferredcontext| {
            self.create_deferred_context(contextflags, ppdeferredcontext)
        })
    }
    fn OpenSharedResource(
        &self,
        hresource: windows::Win32::Foundation::HANDLE,
        returnedinterface: *const windows::core::GUID,
        ppresource: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        Err(missing("ID3D11Device::OpenSharedResource"))
    }
    fn CheckFormatSupport(
        &self,
        format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
    ) -> windows::core::Result<u32> {
        let _call = self.call();
        self.check_format_support(format)
    }
    fn CheckMultisampleQualityLevels(
        &self,
        format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
        samplecount: u32,
    ) -> windows::core::Result<u32> {
        let _call = self.call();
        self.check_multisample_quality_levels(format, samplecount)
    }
    fn CheckCounterInfo(&self, pcounterinfo: *mut D3D11_COUNTER_INFO) {
        let _call = self.call();
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
        let _call = self.call();
        Err(missing("ID3D11Device::CheckCounter"))
    }
    fn CheckFeatureSupport(
        &self,
        feature: D3D11_FEATURE,
        pfeaturesupportdata: *mut core::ffi::c_void,
        featuresupportdatasize: u32,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        self.check_feature_support(feature, pfeaturesupportdata, featuresupportdatasize)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        self.private.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        self.private.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        self.private.set_private_data_interface(guid, pdata)
    }
    fn GetFeatureLevel(&self) -> windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL {
        let _call = self.call();
        self.get_feature_level()
    }
    fn GetCreationFlags(&self) -> u32 {
        let _call = self.call();
        self.get_creation_flags()
    }
    fn GetDeviceRemovedReason(&self) -> windows::core::Result<()> {
        let _call = self.call();
        self.get_device_removed_reason()
    }
    fn GetImmediateContext(&self, ppimmediatecontext: windows::core::OutRef<ID3D11DeviceContext>) {
        let _call = self.call();
        self.get_immediate_context(ppimmediatecontext)
    }
    fn SetExceptionMode(&self, raiseflags: u32) -> windows::core::Result<()> {
        let _call = self.call();
        self.set_exception_mode(raiseflags)
    }
    fn GetExceptionMode(&self) -> u32 {
        let _call = self.call();
        self.get_exception_mode()
    }
}

impl IDXGIDevice3_Impl for Device_Impl {
    fn Trim(&self) {
        let _call = self.call();
        self.trim()
    }
}

impl IDXGIDevice2_Impl for Device_Impl {
    fn OfferResources(
        &self,
        numresources: u32,
        ppresources: *const Option<IDXGIResource>,
        priority: DXGI_OFFER_RESOURCE_PRIORITY,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        self.offer_resources(numresources, ppresources, priority)
    }
    fn ReclaimResources(
        &self,
        numresources: u32,
        ppresources: *const Option<IDXGIResource>,
        pdiscarded: *mut windows::core::BOOL,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        self.reclaim_resources(numresources, ppresources, pdiscarded)
    }
    fn EnqueueSetEvent(
        &self,
        hevent: windows::Win32::Foundation::HANDLE,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        Err(missing("IDXGIDevice2::EnqueueSetEvent"))
    }
}

impl IDXGIDevice1_Impl for Device_Impl {
    fn SetMaximumFrameLatency(&self, maxlatency: u32) -> windows::core::Result<()> {
        let _call = self.call();
        self.set_maximum_frame_latency(maxlatency)
    }
    fn GetMaximumFrameLatency(&self) -> windows::core::Result<u32> {
        let _call = self.call();
        self.get_maximum_frame_latency()
    }
}

impl IDXGIDevice_Impl for Device_Impl {
    fn GetAdapter(&self) -> windows::core::Result<IDXGIAdapter> {
        let _call = self.call();
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
        let _call = self.call();
        Err(missing("IDXGIDevice::CreateSurface"))
    }
    fn QueryResourceResidency(
        &self,
        ppresources: *const Option<windows::core::IUnknown>,
        presidencystatus: *mut DXGI_RESIDENCY,
        numresources: u32,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        Err(missing("IDXGIDevice::QueryResourceResidency"))
    }
    fn SetGPUThreadPriority(&self, priority: i32) -> windows::core::Result<()> {
        let _call = self.call();
        Err(missing("IDXGIDevice::SetGPUThreadPriority"))
    }
    fn GetGPUThreadPriority(&self) -> windows::core::Result<i32> {
        let _call = self.call();
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
        let _call = self.call();
        self.private.set_private_data(name, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        name: *const windows::core::GUID,
        punknown: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        self.private.set_private_data_interface(name, punknown)
    }
    fn GetPrivateData(
        &self,
        name: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        self.private.get_private_data(name, pdatasize, pdata)
    }
    fn GetParent(
        &self,
        riid: *const windows::core::GUID,
        ppparent: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        self.get_parent(riid, ppparent)
    }
}

impl ID3D11DeviceContext3_Impl for Context_Impl {
    fn Flush1(&self, contexttype: D3D11_CONTEXT_TYPE, hevent: windows::Win32::Foundation::HANDLE) {
        let _call = self.call();
        self.flush1(contexttype, hevent)
    }
    fn SetHardwareProtectionState(&self, hwprotectionenable: windows::core::BOOL) {
        let _call = self.call();
        self.unserved("ID3D11DeviceContext3::SetHardwareProtectionState")
    }
    fn GetHardwareProtectionState(&self, phwprotectionenable: *mut windows::core::BOOL) {
        let _call = self.call();
        self.get_hardware_protection_state(phwprotectionenable)
    }
}

impl ID3D11DeviceContext2_Impl for Context_Impl {
    fn UpdateTileMappings(
        &self,
        ptiledresource: windows::core::Ref<ID3D11Resource>,
        numtiledresourceregions: u32,
        ptiledresourceregionstartcoordinates: *const D3D11_TILED_RESOURCE_COORDINATE,
        ptiledresourceregionsizes: *const D3D11_TILE_REGION_SIZE,
        ptilepool: windows::core::Ref<ID3D11Buffer>,
        numranges: u32,
        prangeflags: *const u32,
        ptilepoolstartoffsets: *const u32,
        prangetilecounts: *const u32,
        flags: u32,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        Err(missing("ID3D11DeviceContext2::UpdateTileMappings"))
    }
    fn CopyTileMappings(
        &self,
        pdesttiledresource: windows::core::Ref<ID3D11Resource>,
        pdestregionstartcoordinate: *const D3D11_TILED_RESOURCE_COORDINATE,
        psourcetiledresource: windows::core::Ref<ID3D11Resource>,
        psourceregionstartcoordinate: *const D3D11_TILED_RESOURCE_COORDINATE,
        ptileregionsize: *const D3D11_TILE_REGION_SIZE,
        flags: u32,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        Err(missing("ID3D11DeviceContext2::CopyTileMappings"))
    }
    fn CopyTiles(
        &self,
        ptiledresource: windows::core::Ref<ID3D11Resource>,
        ptileregionstartcoordinate: *const D3D11_TILED_RESOURCE_COORDINATE,
        ptileregionsize: *const D3D11_TILE_REGION_SIZE,
        pbuffer: windows::core::Ref<ID3D11Buffer>,
        bufferstartoffsetinbytes: u64,
        flags: u32,
    ) {
        let _call = self.call();
        self.unserved("ID3D11DeviceContext2::CopyTiles")
    }
    fn UpdateTiles(
        &self,
        pdesttiledresource: windows::core::Ref<ID3D11Resource>,
        pdesttileregionstartcoordinate: *const D3D11_TILED_RESOURCE_COORDINATE,
        pdesttileregionsize: *const D3D11_TILE_REGION_SIZE,
        psourcetiledata: *const core::ffi::c_void,
        flags: u32,
    ) {
        let _call = self.call();
        self.unserved("ID3D11DeviceContext2::UpdateTiles")
    }
    fn ResizeTilePool(
        &self,
        ptilepool: windows::core::Ref<ID3D11Buffer>,
        newsizeinbytes: u64,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        Err(missing("ID3D11DeviceContext2::ResizeTilePool"))
    }
    fn TiledResourceBarrier(
        &self,
        ptiledresourceorviewaccessbeforebarrier: windows::core::Ref<ID3D11DeviceChild>,
        ptiledresourceorviewaccessafterbarrier: windows::core::Ref<ID3D11DeviceChild>,
    ) {
        let _call = self.call();
        self.unserved("ID3D11DeviceContext2::TiledResourceBarrier")
    }
    fn IsAnnotationEnabled(&self) -> windows::core::BOOL {
        let _call = self.call();
        self.is_annotation_enabled()
    }
    fn SetMarkerInt(&self, plabel: &windows::core::PCWSTR, data: i32) {
        let _call = self.call();
        self.set_marker_int(plabel, data)
    }
    fn BeginEventInt(&self, plabel: &windows::core::PCWSTR, data: i32) {
        let _call = self.call();
        self.begin_event_int(plabel, data)
    }
    fn EndEvent(&self) {
        let _call = self.call();
        let _ = self.end_event();
    }
}

impl ID3D11DeviceContext1_Impl for Context_Impl {
    fn CopySubresourceRegion1(
        &self,
        pdstresource: windows::core::Ref<ID3D11Resource>,
        dstsubresource: u32,
        dstx: u32,
        dsty: u32,
        dstz: u32,
        psrcresource: windows::core::Ref<ID3D11Resource>,
        srcsubresource: u32,
        psrcbox: *const D3D11_BOX,
        copyflags: u32,
    ) {
        let _call = self.call();
        self.copy_subresource_region1(
            pdstresource,
            dstsubresource,
            dstx,
            dsty,
            dstz,
            psrcresource,
            srcsubresource,
            psrcbox,
            copyflags,
        )
    }
    fn UpdateSubresource1(
        &self,
        pdstresource: windows::core::Ref<ID3D11Resource>,
        dstsubresource: u32,
        pdstbox: *const D3D11_BOX,
        psrcdata: *const core::ffi::c_void,
        srcrowpitch: u32,
        srcdepthpitch: u32,
        copyflags: u32,
    ) {
        let _call = self.call();
        self.update_subresource1(
            pdstresource,
            dstsubresource,
            pdstbox,
            psrcdata,
            srcrowpitch,
            srcdepthpitch,
            copyflags,
        )
    }
    fn DiscardResource(&self, presource: windows::core::Ref<ID3D11Resource>) {
        let _call = self.call();
        self.discard_resource(presource)
    }
    fn DiscardView(&self, presourceview: windows::core::Ref<ID3D11View>) {
        let _call = self.call();
        self.discard_view(presourceview)
    }
    fn VSSetConstantBuffers1(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
        pfirstconstant: *const u32,
        pnumconstants: *const u32,
    ) {
        let _call = self.call();
        self.vs_set_constant_buffers1(
            startslot,
            numbuffers,
            ppconstantbuffers,
            pfirstconstant,
            pnumconstants,
        )
    }
    fn HSSetConstantBuffers1(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
        pfirstconstant: *const u32,
        pnumconstants: *const u32,
    ) {
        let _call = self.call();
        self.hs_set_constant_buffers1(
            startslot,
            numbuffers,
            ppconstantbuffers,
            pfirstconstant,
            pnumconstants,
        )
    }
    fn DSSetConstantBuffers1(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
        pfirstconstant: *const u32,
        pnumconstants: *const u32,
    ) {
        let _call = self.call();
        self.ds_set_constant_buffers1(
            startslot,
            numbuffers,
            ppconstantbuffers,
            pfirstconstant,
            pnumconstants,
        )
    }
    fn GSSetConstantBuffers1(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
        pfirstconstant: *const u32,
        pnumconstants: *const u32,
    ) {
        let _call = self.call();
        self.gs_set_constant_buffers1(
            startslot,
            numbuffers,
            ppconstantbuffers,
            pfirstconstant,
            pnumconstants,
        )
    }
    fn PSSetConstantBuffers1(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
        pfirstconstant: *const u32,
        pnumconstants: *const u32,
    ) {
        let _call = self.call();
        self.ps_set_constant_buffers1(
            startslot,
            numbuffers,
            ppconstantbuffers,
            pfirstconstant,
            pnumconstants,
        )
    }
    fn CSSetConstantBuffers1(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
        pfirstconstant: *const u32,
        pnumconstants: *const u32,
    ) {
        let _call = self.call();
        self.cs_set_constant_buffers1(
            startslot,
            numbuffers,
            ppconstantbuffers,
            pfirstconstant,
            pnumconstants,
        )
    }
    fn VSGetConstantBuffers1(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: windows::core::OutRef<ID3D11Buffer>,
        pfirstconstant: *mut u32,
        pnumconstants: *mut u32,
    ) {
        let _call = self.call();
        self.vs_get_constant_buffers1(
            startslot,
            numbuffers,
            ppconstantbuffers,
            pfirstconstant,
            pnumconstants,
        )
    }
    fn HSGetConstantBuffers1(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: windows::core::OutRef<ID3D11Buffer>,
        pfirstconstant: *mut u32,
        pnumconstants: *mut u32,
    ) {
        let _call = self.call();
        self.hs_get_constant_buffers1(
            startslot,
            numbuffers,
            ppconstantbuffers,
            pfirstconstant,
            pnumconstants,
        )
    }
    fn DSGetConstantBuffers1(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: windows::core::OutRef<ID3D11Buffer>,
        pfirstconstant: *mut u32,
        pnumconstants: *mut u32,
    ) {
        let _call = self.call();
        self.ds_get_constant_buffers1(
            startslot,
            numbuffers,
            ppconstantbuffers,
            pfirstconstant,
            pnumconstants,
        )
    }
    fn GSGetConstantBuffers1(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: windows::core::OutRef<ID3D11Buffer>,
        pfirstconstant: *mut u32,
        pnumconstants: *mut u32,
    ) {
        let _call = self.call();
        self.gs_get_constant_buffers1(
            startslot,
            numbuffers,
            ppconstantbuffers,
            pfirstconstant,
            pnumconstants,
        )
    }
    fn PSGetConstantBuffers1(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: windows::core::OutRef<ID3D11Buffer>,
        pfirstconstant: *mut u32,
        pnumconstants: *mut u32,
    ) {
        let _call = self.call();
        self.ps_get_constant_buffers1(
            startslot,
            numbuffers,
            ppconstantbuffers,
            pfirstconstant,
            pnumconstants,
        )
    }
    fn CSGetConstantBuffers1(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: windows::core::OutRef<ID3D11Buffer>,
        pfirstconstant: *mut u32,
        pnumconstants: *mut u32,
    ) {
        let _call = self.call();
        self.cs_get_constant_buffers1(
            startslot,
            numbuffers,
            ppconstantbuffers,
            pfirstconstant,
            pnumconstants,
        )
    }
    fn SwapDeviceContextState(
        &self,
        pstate: windows::core::Ref<ID3DDeviceContextState>,
        pppreviousstate: windows::core::OutRef<ID3DDeviceContextState>,
    ) {
        let _call = self.call();
        self.swap_device_context_state(pstate, pppreviousstate)
    }
    fn ClearView(
        &self,
        pview: windows::core::Ref<ID3D11View>,
        color: *const f32,
        prect: *const windows::Win32::Foundation::RECT,
        numrects: u32,
    ) {
        let _call = self.call();
        self.clear_view(pview, color, prect, numrects)
    }
    fn DiscardView1(
        &self,
        presourceview: windows::core::Ref<ID3D11View>,
        prects: *const windows::Win32::Foundation::RECT,
        numrects: u32,
    ) {
        let _call = self.call();
        self.discard_view1(presourceview, prects, numrects)
    }
}

impl ID3D11DeviceContext_Impl for Context_Impl {
    fn VSSetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
    ) {
        let _call = self.call();
        self.vs_set_constant_buffers(startslot, numbuffers, ppconstantbuffers)
    }
    fn PSSetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *const Option<ID3D11ShaderResourceView>,
    ) {
        let _call = self.call();
        self.ps_set_shader_resources(startslot, numviews, ppshaderresourceviews)
    }
    fn PSSetShader(
        &self,
        ppixelshader: windows::core::Ref<ID3D11PixelShader>,
        ppclassinstances: *const Option<ID3D11ClassInstance>,
        numclassinstances: u32,
    ) {
        let _call = self.call();
        self.ps_set_shader(ppixelshader, ppclassinstances, numclassinstances)
    }
    fn PSSetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *const Option<ID3D11SamplerState>,
    ) {
        let _call = self.call();
        self.ps_set_samplers(startslot, numsamplers, ppsamplers)
    }
    fn VSSetShader(
        &self,
        pvertexshader: windows::core::Ref<ID3D11VertexShader>,
        ppclassinstances: *const Option<ID3D11ClassInstance>,
        numclassinstances: u32,
    ) {
        let _call = self.call();
        self.vs_set_shader(pvertexshader, ppclassinstances, numclassinstances)
    }
    fn DrawIndexed(&self, indexcount: u32, startindexlocation: u32, basevertexlocation: i32) {
        let _call = self.call();
        self.draw_indexed(indexcount, startindexlocation, basevertexlocation)
    }
    fn Draw(&self, vertexcount: u32, startvertexlocation: u32) {
        let _call = self.call();
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
        let _call = self.call();
        self.map(presource, subresource, maptype, mapflags, pmappedresource)
    }
    fn Unmap(&self, presource: windows::core::Ref<ID3D11Resource>, subresource: u32) {
        let _call = self.call();
        self.unmap(presource, subresource)
    }
    fn PSSetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
    ) {
        let _call = self.call();
        self.ps_set_constant_buffers(startslot, numbuffers, ppconstantbuffers)
    }
    fn IASetInputLayout(&self, pinputlayout: windows::core::Ref<ID3D11InputLayout>) {
        let _call = self.call();
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
        let _call = self.call();
        self.ia_set_vertex_buffers(startslot, numbuffers, ppvertexbuffers, pstrides, poffsets)
    }
    fn IASetIndexBuffer(
        &self,
        pindexbuffer: windows::core::Ref<ID3D11Buffer>,
        format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
        offset: u32,
    ) {
        let _call = self.call();
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
        let _call = self.call();
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
        let _call = self.call();
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
        let _call = self.call();
        self.gs_set_constant_buffers(startslot, numbuffers, ppconstantbuffers)
    }
    fn GSSetShader(
        &self,
        pshader: windows::core::Ref<ID3D11GeometryShader>,
        ppclassinstances: *const Option<ID3D11ClassInstance>,
        numclassinstances: u32,
    ) {
        let _call = self.call();
        self.gs_set_shader(pshader, ppclassinstances, numclassinstances)
    }
    fn IASetPrimitiveTopology(
        &self,
        topology: windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY,
    ) {
        let _call = self.call();
        self.ia_set_primitive_topology(topology)
    }
    fn VSSetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *const Option<ID3D11ShaderResourceView>,
    ) {
        let _call = self.call();
        self.vs_set_shader_resources(startslot, numviews, ppshaderresourceviews)
    }
    fn VSSetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *const Option<ID3D11SamplerState>,
    ) {
        let _call = self.call();
        self.vs_set_samplers(startslot, numsamplers, ppsamplers)
    }
    fn Begin(&self, pasync: windows::core::Ref<ID3D11Asynchronous>) {
        let _call = self.call();
        self.begin(pasync)
    }
    fn End(&self, pasync: windows::core::Ref<ID3D11Asynchronous>) {
        let _call = self.call();
        self.end(pasync)
    }
    fn GetData(
        &self,
        pasync: windows::core::Ref<ID3D11Asynchronous>,
        pdata: *mut core::ffi::c_void,
        datasize: u32,
        getdataflags: u32,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        self.get_data(pasync, pdata, datasize, getdataflags)
    }
    fn SetPredication(
        &self,
        ppredicate: windows::core::Ref<ID3D11Predicate>,
        predicatevalue: windows::core::BOOL,
    ) {
        let _call = self.call();
        self.set_predication(ppredicate, predicatevalue)
    }
    fn GSSetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *const Option<ID3D11ShaderResourceView>,
    ) {
        let _call = self.call();
        self.gs_set_shader_resources(startslot, numviews, ppshaderresourceviews)
    }
    fn GSSetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *const Option<ID3D11SamplerState>,
    ) {
        let _call = self.call();
        self.gs_set_samplers(startslot, numsamplers, ppsamplers)
    }
    fn OMSetRenderTargets(
        &self,
        numviews: u32,
        pprendertargetviews: *const Option<ID3D11RenderTargetView>,
        pdepthstencilview: windows::core::Ref<ID3D11DepthStencilView>,
    ) {
        let _call = self.call();
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
        let _call = self.call();
        self.om_set_render_targets_and_unordered_access_views(
            numrtvs,
            pprendertargetviews,
            pdepthstencilview,
            uavstartslot,
            numuavs,
            ppunorderedaccessviews,
            puavinitialcounts,
        )
    }
    fn OMSetBlendState(
        &self,
        pblendstate: windows::core::Ref<ID3D11BlendState>,
        blendfactor: *const f32,
        samplemask: u32,
    ) {
        let _call = self.call();
        self.om_set_blend_state(pblendstate, blendfactor, samplemask)
    }
    fn OMSetDepthStencilState(
        &self,
        pdepthstencilstate: windows::core::Ref<ID3D11DepthStencilState>,
        stencilref: u32,
    ) {
        let _call = self.call();
        self.om_set_depth_stencil_state(pdepthstencilstate, stencilref)
    }
    fn SOSetTargets(
        &self,
        numbuffers: u32,
        ppsotargets: *const Option<ID3D11Buffer>,
        poffsets: *const u32,
    ) {
        let _call = self.call();
        self.so_set_targets(numbuffers, ppsotargets, poffsets)
    }
    fn DrawAuto(&self) {
        let _call = self.call();
        self.draw_auto()
    }
    fn DrawIndexedInstancedIndirect(
        &self,
        pbufferforargs: windows::core::Ref<ID3D11Buffer>,
        alignedbyteoffsetforargs: u32,
    ) {
        let _call = self.call();
        self.draw_indexed_instanced_indirect(pbufferforargs, alignedbyteoffsetforargs)
    }
    fn DrawInstancedIndirect(
        &self,
        pbufferforargs: windows::core::Ref<ID3D11Buffer>,
        alignedbyteoffsetforargs: u32,
    ) {
        let _call = self.call();
        self.draw_instanced_indirect(pbufferforargs, alignedbyteoffsetforargs)
    }
    fn Dispatch(&self, threadgroupcountx: u32, threadgroupcounty: u32, threadgroupcountz: u32) {
        let _call = self.call();
        self.dispatch(threadgroupcountx, threadgroupcounty, threadgroupcountz)
    }
    fn DispatchIndirect(
        &self,
        pbufferforargs: windows::core::Ref<ID3D11Buffer>,
        alignedbyteoffsetforargs: u32,
    ) {
        let _call = self.call();
        self.dispatch_indirect(pbufferforargs, alignedbyteoffsetforargs)
    }
    fn RSSetState(&self, prasterizerstate: windows::core::Ref<ID3D11RasterizerState>) {
        let _call = self.call();
        self.rs_set_state(prasterizerstate)
    }
    fn RSSetViewports(&self, numviewports: u32, pviewports: *const D3D11_VIEWPORT) {
        let _call = self.call();
        self.rs_set_viewports(numviewports, pviewports)
    }
    fn RSSetScissorRects(&self, numrects: u32, prects: *const windows::Win32::Foundation::RECT) {
        let _call = self.call();
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
        let _call = self.call();
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
        let _call = self.call();
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
        let _call = self.call();
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
        let _call = self.call();
        self.copy_structure_count(pdstbuffer, dstalignedbyteoffset, psrcview)
    }
    fn ClearRenderTargetView(
        &self,
        prendertargetview: windows::core::Ref<ID3D11RenderTargetView>,
        colorrgba: *const f32,
    ) {
        let _call = self.call();
        self.clear_render_target_view(prendertargetview, colorrgba)
    }
    fn ClearUnorderedAccessViewUint(
        &self,
        punorderedaccessview: windows::core::Ref<ID3D11UnorderedAccessView>,
        values: *const u32,
    ) {
        let _call = self.call();
        self.clear_unordered_access_view_uint(punorderedaccessview, values)
    }
    fn ClearUnorderedAccessViewFloat(
        &self,
        punorderedaccessview: windows::core::Ref<ID3D11UnorderedAccessView>,
        values: *const f32,
    ) {
        let _call = self.call();
        self.clear_unordered_access_view_float(punorderedaccessview, values)
    }
    fn ClearDepthStencilView(
        &self,
        pdepthstencilview: windows::core::Ref<ID3D11DepthStencilView>,
        clearflags: u32,
        depth: f32,
        stencil: u8,
    ) {
        let _call = self.call();
        self.clear_depth_stencil_view(pdepthstencilview, clearflags, depth, stencil)
    }
    fn GenerateMips(&self, pshaderresourceview: windows::core::Ref<ID3D11ShaderResourceView>) {
        let _call = self.call();
        self.generate_mips(pshaderresourceview)
    }
    fn SetResourceMinLOD(&self, presource: windows::core::Ref<ID3D11Resource>, minlod: f32) {
        let _call = self.call();
        self.set_resource_min_lod(presource, minlod)
    }
    fn GetResourceMinLOD(&self, presource: windows::core::Ref<ID3D11Resource>) -> f32 {
        let _call = self.call();
        self.get_resource_min_lod(presource)
    }
    fn ResolveSubresource(
        &self,
        pdstresource: windows::core::Ref<ID3D11Resource>,
        dstsubresource: u32,
        psrcresource: windows::core::Ref<ID3D11Resource>,
        srcsubresource: u32,
        format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
    ) {
        let _call = self.call();
        self.resolve_subresource(
            pdstresource,
            dstsubresource,
            psrcresource,
            srcsubresource,
            format,
        )
    }
    fn ExecuteCommandList(
        &self,
        pcommandlist: windows::core::Ref<ID3D11CommandList>,
        restorecontextstate: windows::core::BOOL,
    ) {
        let _call = self.call();
        self.execute_command_list(pcommandlist, restorecontextstate)
    }
    fn HSSetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *const Option<ID3D11ShaderResourceView>,
    ) {
        let _call = self.call();
        self.only_unbinding(
            "ID3D11DeviceContext::HSSetShaderResources",
            super::context::binds(ppshaderresourceviews, numviews),
        )
    }
    fn HSSetShader(
        &self,
        phullshader: windows::core::Ref<ID3D11HullShader>,
        ppclassinstances: *const Option<ID3D11ClassInstance>,
        numclassinstances: u32,
    ) {
        let _call = self.call();
        self.only_unbinding("ID3D11DeviceContext::HSSetShader", !phullshader.is_null())
    }
    fn HSSetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *const Option<ID3D11SamplerState>,
    ) {
        let _call = self.call();
        self.hs_set_samplers(startslot, numsamplers, ppsamplers)
    }
    fn HSSetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
    ) {
        let _call = self.call();
        self.hs_set_constant_buffers(startslot, numbuffers, ppconstantbuffers)
    }
    fn DSSetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *const Option<ID3D11ShaderResourceView>,
    ) {
        let _call = self.call();
        self.only_unbinding(
            "ID3D11DeviceContext::DSSetShaderResources",
            super::context::binds(ppshaderresourceviews, numviews),
        )
    }
    fn DSSetShader(
        &self,
        pdomainshader: windows::core::Ref<ID3D11DomainShader>,
        ppclassinstances: *const Option<ID3D11ClassInstance>,
        numclassinstances: u32,
    ) {
        let _call = self.call();
        self.only_unbinding("ID3D11DeviceContext::DSSetShader", !pdomainshader.is_null())
    }
    fn DSSetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *const Option<ID3D11SamplerState>,
    ) {
        let _call = self.call();
        self.ds_set_samplers(startslot, numsamplers, ppsamplers)
    }
    fn DSSetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
    ) {
        let _call = self.call();
        self.ds_set_constant_buffers(startslot, numbuffers, ppconstantbuffers)
    }
    fn CSSetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *const Option<ID3D11ShaderResourceView>,
    ) {
        let _call = self.call();
        self.cs_set_shader_resources(startslot, numviews, ppshaderresourceviews)
    }
    fn CSSetUnorderedAccessViews(
        &self,
        startslot: u32,
        numuavs: u32,
        ppunorderedaccessviews: *const Option<ID3D11UnorderedAccessView>,
        puavinitialcounts: *const u32,
    ) {
        let _call = self.call();
        self.cs_set_unordered_access_views(
            startslot,
            numuavs,
            ppunorderedaccessviews,
            puavinitialcounts,
        )
    }
    fn CSSetShader(
        &self,
        pcomputeshader: windows::core::Ref<ID3D11ComputeShader>,
        ppclassinstances: *const Option<ID3D11ClassInstance>,
        numclassinstances: u32,
    ) {
        let _call = self.call();
        self.cs_set_shader(pcomputeshader, ppclassinstances, numclassinstances)
    }
    fn CSSetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *const Option<ID3D11SamplerState>,
    ) {
        let _call = self.call();
        self.cs_set_samplers(startslot, numsamplers, ppsamplers)
    }
    fn CSSetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *const Option<ID3D11Buffer>,
    ) {
        let _call = self.call();
        self.cs_set_constant_buffers(startslot, numbuffers, ppconstantbuffers)
    }
    fn VSGetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *mut Option<ID3D11Buffer>,
    ) {
        let _call = self.call();
        self.vs_get_constant_buffers(startslot, numbuffers, ppconstantbuffers)
    }
    fn PSGetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *mut Option<ID3D11ShaderResourceView>,
    ) {
        let _call = self.call();
        self.ps_get_shader_resources(startslot, numviews, ppshaderresourceviews)
    }
    fn PSGetShader(
        &self,
        pppixelshader: windows::core::OutRef<ID3D11PixelShader>,
        ppclassinstances: windows::core::OutRef<ID3D11ClassInstance>,
        pnumclassinstances: *mut u32,
    ) {
        let _call = self.call();
        self.ps_get_shader(pppixelshader, ppclassinstances, pnumclassinstances)
    }
    fn PSGetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *mut Option<ID3D11SamplerState>,
    ) {
        let _call = self.call();
        self.ps_get_samplers(startslot, numsamplers, ppsamplers)
    }
    fn VSGetShader(
        &self,
        ppvertexshader: windows::core::OutRef<ID3D11VertexShader>,
        ppclassinstances: windows::core::OutRef<ID3D11ClassInstance>,
        pnumclassinstances: *mut u32,
    ) {
        let _call = self.call();
        self.vs_get_shader(ppvertexshader, ppclassinstances, pnumclassinstances)
    }
    fn PSGetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *mut Option<ID3D11Buffer>,
    ) {
        let _call = self.call();
        self.ps_get_constant_buffers(startslot, numbuffers, ppconstantbuffers)
    }
    fn IAGetInputLayout(&self, ppinputlayout: windows::core::OutRef<ID3D11InputLayout>) {
        let _call = self.call();
        self.ia_get_input_layout(ppinputlayout)
    }
    fn IAGetVertexBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppvertexbuffers: windows::core::OutRef<ID3D11Buffer>,
        pstrides: *mut u32,
        poffsets: *mut u32,
    ) {
        let _call = self.call();
        self.ia_get_vertex_buffers(startslot, numbuffers, ppvertexbuffers, pstrides, poffsets)
    }
    fn IAGetIndexBuffer(
        &self,
        pindexbuffer: windows::core::OutRef<ID3D11Buffer>,
        format: *mut windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
        offset: *mut u32,
    ) {
        let _call = self.call();
        self.ia_get_index_buffer(pindexbuffer, format, offset)
    }
    fn GSGetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *mut Option<ID3D11Buffer>,
    ) {
        let _call = self.call();
        self.gs_get_constant_buffers(startslot, numbuffers, ppconstantbuffers)
    }
    fn GSGetShader(
        &self,
        ppgeometryshader: windows::core::OutRef<ID3D11GeometryShader>,
        ppclassinstances: windows::core::OutRef<ID3D11ClassInstance>,
        pnumclassinstances: *mut u32,
    ) {
        let _call = self.call();
        self.gs_get_shader(ppgeometryshader, ppclassinstances, pnumclassinstances)
    }
    fn IAGetPrimitiveTopology(
        &self,
        ptopology: *mut windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY,
    ) {
        let _call = self.call();
        self.ia_get_primitive_topology(ptopology)
    }
    fn VSGetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *mut Option<ID3D11ShaderResourceView>,
    ) {
        let _call = self.call();
        self.vs_get_shader_resources(startslot, numviews, ppshaderresourceviews)
    }
    fn VSGetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *mut Option<ID3D11SamplerState>,
    ) {
        let _call = self.call();
        self.vs_get_samplers(startslot, numsamplers, ppsamplers)
    }
    fn GetPredication(
        &self,
        pppredicate: windows::core::OutRef<ID3D11Predicate>,
        ppredicatevalue: *mut windows::core::BOOL,
    ) {
        let _call = self.call();
        self.get_predication(pppredicate, ppredicatevalue)
    }
    fn GSGetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *mut Option<ID3D11ShaderResourceView>,
    ) {
        let _call = self.call();
        self.gs_get_shader_resources(startslot, numviews, ppshaderresourceviews)
    }
    fn GSGetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *mut Option<ID3D11SamplerState>,
    ) {
        let _call = self.call();
        self.gs_get_samplers(startslot, numsamplers, ppsamplers)
    }
    fn OMGetRenderTargets(
        &self,
        numviews: u32,
        pprendertargetviews: *mut Option<ID3D11RenderTargetView>,
        ppdepthstencilview: windows::core::OutRef<ID3D11DepthStencilView>,
    ) {
        let _call = self.call();
        self.om_get_render_targets(numviews, pprendertargetviews, ppdepthstencilview)
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
        let _call = self.call();
        self.om_get_render_targets_and_unordered_access_views(
            numrtvs,
            pprendertargetviews,
            ppdepthstencilview,
            uavstartslot,
            numuavs,
            ppunorderedaccessviews,
        )
    }
    fn OMGetBlendState(
        &self,
        ppblendstate: windows::core::OutRef<ID3D11BlendState>,
        blendfactor: *mut f32,
        psamplemask: *mut u32,
    ) {
        let _call = self.call();
        self.om_get_blend_state(ppblendstate, blendfactor, psamplemask)
    }
    fn OMGetDepthStencilState(
        &self,
        ppdepthstencilstate: windows::core::OutRef<ID3D11DepthStencilState>,
        pstencilref: *mut u32,
    ) {
        let _call = self.call();
        self.om_get_depth_stencil_state(ppdepthstencilstate, pstencilref)
    }
    fn SOGetTargets(&self, numbuffers: u32, ppsotargets: *mut Option<ID3D11Buffer>) {
        let _call = self.call();
        self.so_get_targets(numbuffers, ppsotargets)
    }
    fn RSGetState(&self, pprasterizerstate: windows::core::OutRef<ID3D11RasterizerState>) {
        let _call = self.call();
        self.rs_get_state(pprasterizerstate)
    }
    fn RSGetViewports(&self, pnumviewports: *mut u32, pviewports: *mut D3D11_VIEWPORT) {
        let _call = self.call();
        self.rs_get_viewports(pnumviewports, pviewports)
    }
    fn RSGetScissorRects(
        &self,
        pnumrects: *mut u32,
        prects: *mut windows::Win32::Foundation::RECT,
    ) {
        let _call = self.call();
        self.rs_get_scissor_rects(pnumrects, prects)
    }
    fn HSGetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *mut Option<ID3D11ShaderResourceView>,
    ) {
        let _call = self.call();
        super::context::nothing_bound(ppshaderresourceviews, numviews)
    }
    fn HSGetShader(
        &self,
        pphullshader: windows::core::OutRef<ID3D11HullShader>,
        ppclassinstances: windows::core::OutRef<ID3D11ClassInstance>,
        pnumclassinstances: *mut u32,
    ) {
        let _call = self.call();
        super::context::no_instances(pnumclassinstances);
        let _ = pphullshader.write(None);
    }
    fn HSGetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *mut Option<ID3D11SamplerState>,
    ) {
        let _call = self.call();
        self.hs_get_samplers(startslot, numsamplers, ppsamplers)
    }
    fn HSGetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *mut Option<ID3D11Buffer>,
    ) {
        let _call = self.call();
        self.hs_get_constant_buffers(startslot, numbuffers, ppconstantbuffers)
    }
    fn DSGetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *mut Option<ID3D11ShaderResourceView>,
    ) {
        let _call = self.call();
        super::context::nothing_bound(ppshaderresourceviews, numviews)
    }
    fn DSGetShader(
        &self,
        ppdomainshader: windows::core::OutRef<ID3D11DomainShader>,
        ppclassinstances: windows::core::OutRef<ID3D11ClassInstance>,
        pnumclassinstances: *mut u32,
    ) {
        let _call = self.call();
        super::context::no_instances(pnumclassinstances);
        let _ = ppdomainshader.write(None);
    }
    fn DSGetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *mut Option<ID3D11SamplerState>,
    ) {
        let _call = self.call();
        self.ds_get_samplers(startslot, numsamplers, ppsamplers)
    }
    fn DSGetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *mut Option<ID3D11Buffer>,
    ) {
        let _call = self.call();
        self.ds_get_constant_buffers(startslot, numbuffers, ppconstantbuffers)
    }
    fn CSGetShaderResources(
        &self,
        startslot: u32,
        numviews: u32,
        ppshaderresourceviews: *mut Option<ID3D11ShaderResourceView>,
    ) {
        let _call = self.call();
        self.cs_get_shader_resources(startslot, numviews, ppshaderresourceviews)
    }
    fn CSGetUnorderedAccessViews(
        &self,
        startslot: u32,
        numuavs: u32,
        ppunorderedaccessviews: *mut Option<ID3D11UnorderedAccessView>,
    ) {
        let _call = self.call();
        self.cs_get_unordered_access_views(startslot, numuavs, ppunorderedaccessviews)
    }
    fn CSGetShader(
        &self,
        ppcomputeshader: windows::core::OutRef<ID3D11ComputeShader>,
        ppclassinstances: windows::core::OutRef<ID3D11ClassInstance>,
        pnumclassinstances: *mut u32,
    ) {
        let _call = self.call();
        self.cs_get_shader(ppcomputeshader, ppclassinstances, pnumclassinstances)
    }
    fn CSGetSamplers(
        &self,
        startslot: u32,
        numsamplers: u32,
        ppsamplers: *mut Option<ID3D11SamplerState>,
    ) {
        let _call = self.call();
        self.cs_get_samplers(startslot, numsamplers, ppsamplers)
    }
    fn CSGetConstantBuffers(
        &self,
        startslot: u32,
        numbuffers: u32,
        ppconstantbuffers: *mut Option<ID3D11Buffer>,
    ) {
        let _call = self.call();
        self.cs_get_constant_buffers(startslot, numbuffers, ppconstantbuffers)
    }
    fn ClearState(&self) {
        let _call = self.call();
        self.clear_state()
    }
    fn Flush(&self) {
        let _call = self.call();
        self.flush()
    }
    fn GetType(&self) -> D3D11_DEVICE_CONTEXT_TYPE {
        let _call = self.call();
        self.get_type()
    }
    fn GetContextFlags(&self) -> u32 {
        let _call = self.call();
        self.get_context_flags()
    }
    fn FinishCommandList(
        &self,
        restoredeferredcontextstate: windows::core::BOOL,
        ppcommandlist: windows::core::OutRef<ID3D11CommandList>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        self.finish_command_list(restoredeferredcontextstate, ppcommandlist)
    }
}

impl ID3D11DeviceChild_Impl for Context_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _call = self.call();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _call = self.call();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11Multithread_Impl for Context_Impl {
    fn Enter(&self) {
        let _call = self.call();
        self.enter()
    }
    fn Leave(&self) {
        let _call = self.call();
        self.leave()
    }
    fn SetMultithreadProtected(&self, bmtprotect: windows::core::BOOL) -> windows::core::BOOL {
        let _call = self.call();
        self.set_multithread_protected(bmtprotect)
    }
    fn GetMultithreadProtected(&self) -> windows::core::BOOL {
        let _call = self.call();
        self.get_multithread_protected()
    }
}

impl ID3DUserDefinedAnnotation_Impl for Context_Impl {
    fn BeginEvent(&self, name: &windows::core::PCWSTR) -> i32 {
        let _call = self.call();
        self.begin_event(name)
    }
    fn EndEvent(&self) -> i32 {
        let _call = self.call();
        self.end_event()
    }
    fn SetMarker(&self, name: &windows::core::PCWSTR) {
        let _call = self.call();
        self.set_marker(name)
    }
    fn GetStatus(&self) -> windows::core::BOOL {
        let _call = self.call();
        self.get_status()
    }
}

impl IDXGIAdapter1_Impl for Adapter_Impl {
    fn GetDesc1(&self) -> windows::core::Result<DXGI_ADAPTER_DESC1> {
        let _lock = lock();
        self.get_desc1()
    }
}

impl IDXGIAdapter_Impl for Adapter_Impl {
    fn EnumOutputs(&self, output: u32) -> windows::core::Result<IDXGIOutput> {
        let _lock = lock();
        self.enum_outputs(output)
    }
    fn GetDesc(&self) -> windows::core::Result<DXGI_ADAPTER_DESC> {
        let _lock = lock();
        self.get_desc()
    }
    fn CheckInterfaceSupport(
        &self,
        interfacename: *const windows::core::GUID,
    ) -> windows::core::Result<i64> {
        let _lock = lock();
        self.check_interface_support(interfacename)
    }
}

impl IDXGIObject_Impl for Adapter_Impl {
    fn SetPrivateData(
        &self,
        name: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.private.set_private_data(name, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        name: *const windows::core::GUID,
        punknown: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.private.set_private_data_interface(name, punknown)
    }
    fn GetPrivateData(
        &self,
        name: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.private.get_private_data(name, pdatasize, pdata)
    }
    fn GetParent(
        &self,
        riid: *const windows::core::GUID,
        ppparent: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.get_parent(riid, ppparent)
    }
}

impl IDXGIOutput_Impl for Output_Impl {
    fn GetDesc(&self) -> windows::core::Result<DXGI_OUTPUT_DESC> {
        let _lock = lock();
        self.get_desc()
    }
    fn GetDisplayModeList(
        &self,
        enumformat: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
        flags: DXGI_ENUM_MODES,
        pnummodes: *mut u32,
        pdesc: *mut windows::Win32::Graphics::Dxgi::Common::DXGI_MODE_DESC,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.get_display_mode_list(enumformat, flags, pnummodes, pdesc)
    }
    fn FindClosestMatchingMode(
        &self,
        pmodetomatch: *const windows::Win32::Graphics::Dxgi::Common::DXGI_MODE_DESC,
        pclosestmatch: *mut windows::Win32::Graphics::Dxgi::Common::DXGI_MODE_DESC,
        pconcerneddevice: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.find_closest_matching_mode(pmodetomatch, pclosestmatch, pconcerneddevice)
    }
    fn WaitForVBlank(&self) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGIOutput::WaitForVBlank"))
    }
    fn TakeOwnership(
        &self,
        pdevice: windows::core::Ref<windows::core::IUnknown>,
        exclusive: windows::core::BOOL,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGIOutput::TakeOwnership"))
    }
    fn ReleaseOwnership(&self) {
        let _lock = lock();
        missing_void("IDXGIOutput::ReleaseOwnership")
    }
    fn GetGammaControlCapabilities(
        &self,
        pgammacaps: *mut windows::Win32::Graphics::Dxgi::Common::DXGI_GAMMA_CONTROL_CAPABILITIES,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGIOutput::GetGammaControlCapabilities"))
    }
    fn SetGammaControl(
        &self,
        parray: *const windows::Win32::Graphics::Dxgi::Common::DXGI_GAMMA_CONTROL,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGIOutput::SetGammaControl"))
    }
    fn GetGammaControl(
        &self,
        parray: *mut windows::Win32::Graphics::Dxgi::Common::DXGI_GAMMA_CONTROL,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGIOutput::GetGammaControl"))
    }
    fn SetDisplaySurface(
        &self,
        pscanoutsurface: windows::core::Ref<IDXGISurface>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGIOutput::SetDisplaySurface"))
    }
    fn GetDisplaySurfaceData(
        &self,
        pdestination: windows::core::Ref<IDXGISurface>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGIOutput::GetDisplaySurfaceData"))
    }
    fn GetFrameStatistics(&self, pstats: *mut DXGI_FRAME_STATISTICS) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGIOutput::GetFrameStatistics"))
    }
}

impl IDXGIObject_Impl for Output_Impl {
    fn SetPrivateData(
        &self,
        name: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.private.set_private_data(name, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        name: *const windows::core::GUID,
        punknown: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.private.set_private_data_interface(name, punknown)
    }
    fn GetPrivateData(
        &self,
        name: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.private.get_private_data(name, pdatasize, pdata)
    }
    fn GetParent(
        &self,
        riid: *const windows::core::GUID,
        ppparent: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.get_parent(riid, ppparent)
    }
}

impl IDXGIFactory6_Impl for Factory_Impl {
    fn EnumAdapterByGpuPreference(
        &self,
        adapter: u32,
        gpupreference: DXGI_GPU_PREFERENCE,
        riid: *const windows::core::GUID,
        ppvadapter: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.enum_adapter_by_gpu_preference(adapter, gpupreference, riid, ppvadapter)
    }
}

impl IDXGIFactory5_Impl for Factory_Impl {
    fn CheckFeatureSupport(
        &self,
        feature: DXGI_FEATURE,
        pfeaturesupportdata: *mut core::ffi::c_void,
        featuresupportdatasize: u32,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.check_feature_support(feature, pfeaturesupportdata, featuresupportdatasize)
    }
}

impl IDXGIFactory4_Impl for Factory_Impl {
    fn EnumAdapterByLuid(
        &self,
        adapterluid: &windows::Win32::Foundation::LUID,
        riid: *const windows::core::GUID,
        ppvadapter: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.enum_adapter_by_luid(adapterluid, riid, ppvadapter)
    }
    fn EnumWarpAdapter(
        &self,
        riid: *const windows::core::GUID,
        ppvadapter: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGIFactory4::EnumWarpAdapter"))
    }
}

impl IDXGIFactory3_Impl for Factory_Impl {
    fn GetCreationFlags(&self) -> DXGI_CREATE_FACTORY_FLAGS {
        let _lock = lock();
        self.get_creation_flags()
    }
}

impl IDXGIFactory2_Impl for Factory_Impl {
    fn IsWindowedStereoEnabled(&self) -> windows::core::BOOL {
        let _lock = lock();
        self.is_windowed_stereo_enabled()
    }
    fn CreateSwapChainForHwnd(
        &self,
        pdevice: windows::core::Ref<windows::core::IUnknown>,
        hwnd: windows::Win32::Foundation::HWND,
        pdesc: *const DXGI_SWAP_CHAIN_DESC1,
        pfullscreendesc: *const DXGI_SWAP_CHAIN_FULLSCREEN_DESC,
        prestricttooutput: windows::core::Ref<IDXGIOutput>,
    ) -> windows::core::Result<IDXGISwapChain1> {
        let _lock = lock();
        self.create_swap_chain_for_hwnd(pdevice, hwnd, pdesc, pfullscreendesc, prestricttooutput)
    }
    fn CreateSwapChainForCoreWindow(
        &self,
        pdevice: windows::core::Ref<windows::core::IUnknown>,
        pwindow: windows::core::Ref<windows::core::IUnknown>,
        pdesc: *const DXGI_SWAP_CHAIN_DESC1,
        prestricttooutput: windows::core::Ref<IDXGIOutput>,
    ) -> windows::core::Result<IDXGISwapChain1> {
        let _lock = lock();
        Err(missing("IDXGIFactory2::CreateSwapChainForCoreWindow"))
    }
    fn GetSharedResourceAdapterLuid(
        &self,
        hresource: windows::Win32::Foundation::HANDLE,
    ) -> windows::core::Result<windows::Win32::Foundation::LUID> {
        let _lock = lock();
        Err(missing("IDXGIFactory2::GetSharedResourceAdapterLuid"))
    }
    fn RegisterStereoStatusWindow(
        &self,
        windowhandle: windows::Win32::Foundation::HWND,
        wmsg: u32,
    ) -> windows::core::Result<u32> {
        let _lock = lock();
        Err(missing("IDXGIFactory2::RegisterStereoStatusWindow"))
    }
    fn RegisterStereoStatusEvent(
        &self,
        hevent: windows::Win32::Foundation::HANDLE,
    ) -> windows::core::Result<u32> {
        let _lock = lock();
        Err(missing("IDXGIFactory2::RegisterStereoStatusEvent"))
    }
    fn UnregisterStereoStatus(&self, dwcookie: u32) {
        let _lock = lock();
        missing_void("IDXGIFactory2::UnregisterStereoStatus")
    }
    fn RegisterOcclusionStatusWindow(
        &self,
        windowhandle: windows::Win32::Foundation::HWND,
        wmsg: u32,
    ) -> windows::core::Result<u32> {
        let _lock = lock();
        Err(missing("IDXGIFactory2::RegisterOcclusionStatusWindow"))
    }
    fn RegisterOcclusionStatusEvent(
        &self,
        hevent: windows::Win32::Foundation::HANDLE,
    ) -> windows::core::Result<u32> {
        let _lock = lock();
        Err(missing("IDXGIFactory2::RegisterOcclusionStatusEvent"))
    }
    fn UnregisterOcclusionStatus(&self, dwcookie: u32) {
        let _lock = lock();
        missing_void("IDXGIFactory2::UnregisterOcclusionStatus")
    }
    fn CreateSwapChainForComposition(
        &self,
        pdevice: windows::core::Ref<windows::core::IUnknown>,
        pdesc: *const DXGI_SWAP_CHAIN_DESC1,
        prestricttooutput: windows::core::Ref<IDXGIOutput>,
    ) -> windows::core::Result<IDXGISwapChain1> {
        let _lock = lock();
        Err(missing("IDXGIFactory2::CreateSwapChainForComposition"))
    }
}

impl IDXGIFactory1_Impl for Factory_Impl {
    fn EnumAdapters1(&self, adapter: u32) -> windows::core::Result<IDXGIAdapter1> {
        let _lock = lock();
        self.enum_adapters1(adapter)
    }
    fn IsCurrent(&self) -> windows::core::BOOL {
        let _lock = lock();
        self.is_current()
    }
}

impl IDXGIFactory_Impl for Factory_Impl {
    fn EnumAdapters(&self, adapter: u32) -> windows::core::Result<IDXGIAdapter> {
        let _lock = lock();
        self.enum_adapters(adapter)
    }
    fn MakeWindowAssociation(
        &self,
        windowhandle: windows::Win32::Foundation::HWND,
        flags: DXGI_MWA_FLAGS,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.make_window_association(windowhandle, flags)
    }
    fn GetWindowAssociation(&self) -> windows::core::Result<windows::Win32::Foundation::HWND> {
        let _lock = lock();
        self.get_window_association()
    }
    fn CreateSwapChain(
        &self,
        pdevice: windows::core::Ref<windows::core::IUnknown>,
        pdesc: *const DXGI_SWAP_CHAIN_DESC,
        ppswapchain: windows::core::OutRef<IDXGISwapChain>,
    ) -> windows::core::HRESULT {
        let _lock = lock();
        self.create_swap_chain(pdevice, pdesc, ppswapchain)
    }
    fn CreateSoftwareAdapter(
        &self,
        module: windows::Win32::Foundation::HMODULE,
    ) -> windows::core::Result<IDXGIAdapter> {
        let _lock = lock();
        Err(missing("IDXGIFactory::CreateSoftwareAdapter"))
    }
}

impl IDXGIObject_Impl for Factory_Impl {
    fn SetPrivateData(
        &self,
        name: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.private.set_private_data(name, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        name: *const windows::core::GUID,
        punknown: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.private.set_private_data_interface(name, punknown)
    }
    fn GetPrivateData(
        &self,
        name: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.private.get_private_data(name, pdatasize, pdata)
    }
    fn GetParent(
        &self,
        riid: *const windows::core::GUID,
        ppparent: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.get_parent(riid, ppparent)
    }
}

impl IDXGISwapChain4_Impl for SwapChain_Impl {
    fn SetHDRMetaData(
        &self,
        r#type: DXGI_HDR_METADATA_TYPE,
        size: u32,
        pmetadata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.set_hdr_meta_data(r#type, size, pmetadata)
    }
}

impl IDXGISwapChain3_Impl for SwapChain_Impl {
    fn GetCurrentBackBufferIndex(&self) -> u32 {
        let _lock = lock();
        self.get_current_back_buffer_index()
    }
    fn CheckColorSpaceSupport(
        &self,
        colorspace: windows::Win32::Graphics::Dxgi::Common::DXGI_COLOR_SPACE_TYPE,
    ) -> windows::core::Result<u32> {
        let _lock = lock();
        self.check_color_space_support(colorspace)
    }
    fn SetColorSpace1(
        &self,
        colorspace: windows::Win32::Graphics::Dxgi::Common::DXGI_COLOR_SPACE_TYPE,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.set_color_space1(colorspace)
    }
    fn ResizeBuffers1(
        &self,
        buffercount: u32,
        width: u32,
        height: u32,
        format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
        swapchainflags: &DXGI_SWAP_CHAIN_FLAG,
        pcreationnodemask: *const u32,
        pppresentqueue: *const Option<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.resize_buffers1(
            buffercount,
            width,
            height,
            format,
            swapchainflags,
            pcreationnodemask,
            pppresentqueue,
        )
    }
}

impl IDXGISwapChain2_Impl for SwapChain_Impl {
    fn SetSourceSize(&self, width: u32, height: u32) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISwapChain2::SetSourceSize"))
    }
    fn GetSourceSize(&self, pwidth: *mut u32, pheight: *mut u32) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISwapChain2::GetSourceSize"))
    }
    fn SetMaximumFrameLatency(&self, maxlatency: u32) -> windows::core::Result<()> {
        let _lock = lock();
        self.set_maximum_frame_latency(maxlatency)
    }
    fn GetMaximumFrameLatency(&self) -> windows::core::Result<u32> {
        let _lock = lock();
        self.get_maximum_frame_latency()
    }
    fn GetFrameLatencyWaitableObject(&self) -> windows::Win32::Foundation::HANDLE {
        let _lock = lock();
        missing_void("IDXGISwapChain2::GetFrameLatencyWaitableObject");
        Default::default()
    }
    fn SetMatrixTransform(&self, pmatrix: *const DXGI_MATRIX_3X2_F) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISwapChain2::SetMatrixTransform"))
    }
    fn GetMatrixTransform(&self, pmatrix: *mut DXGI_MATRIX_3X2_F) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISwapChain2::GetMatrixTransform"))
    }
}

impl IDXGISwapChain1_Impl for SwapChain_Impl {
    fn GetDesc1(&self) -> windows::core::Result<DXGI_SWAP_CHAIN_DESC1> {
        let _lock = lock();
        self.get_desc1()
    }
    fn GetFullscreenDesc(&self) -> windows::core::Result<DXGI_SWAP_CHAIN_FULLSCREEN_DESC> {
        let _lock = lock();
        Err(missing("IDXGISwapChain1::GetFullscreenDesc"))
    }
    fn GetHwnd(&self) -> windows::core::Result<windows::Win32::Foundation::HWND> {
        let _lock = lock();
        self.get_hwnd()
    }
    fn GetCoreWindow(
        &self,
        refiid: *const windows::core::GUID,
        ppunk: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISwapChain1::GetCoreWindow"))
    }
    fn Present1(
        &self,
        syncinterval: u32,
        presentflags: DXGI_PRESENT,
        ppresentparameters: *const DXGI_PRESENT_PARAMETERS,
    ) -> windows::core::HRESULT {
        let _lock = lock();
        self.present1(syncinterval, presentflags, ppresentparameters)
    }
    fn IsTemporaryMonoSupported(&self) -> windows::core::BOOL {
        let _lock = lock();
        missing_void("IDXGISwapChain1::IsTemporaryMonoSupported");
        Default::default()
    }
    fn GetRestrictToOutput(&self) -> windows::core::Result<IDXGIOutput> {
        let _lock = lock();
        Err(missing("IDXGISwapChain1::GetRestrictToOutput"))
    }
    fn SetBackgroundColor(&self, pcolor: *const DXGI_RGBA) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISwapChain1::SetBackgroundColor"))
    }
    fn GetBackgroundColor(&self) -> windows::core::Result<DXGI_RGBA> {
        let _lock = lock();
        Err(missing("IDXGISwapChain1::GetBackgroundColor"))
    }
    fn SetRotation(
        &self,
        rotation: windows::Win32::Graphics::Dxgi::Common::DXGI_MODE_ROTATION,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.set_rotation(rotation)
    }
    fn GetRotation(
        &self,
    ) -> windows::core::Result<windows::Win32::Graphics::Dxgi::Common::DXGI_MODE_ROTATION> {
        let _lock = lock();
        self.get_rotation()
    }
}

impl IDXGISwapChain_Impl for SwapChain_Impl {
    fn Present(&self, syncinterval: u32, flags: DXGI_PRESENT) -> windows::core::HRESULT {
        let _lock = lock();
        self.present(syncinterval, flags)
    }
    fn GetBuffer(
        &self,
        buffer: u32,
        riid: *const windows::core::GUID,
        ppsurface: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.get_buffer(buffer, riid, ppsurface)
    }
    fn SetFullscreenState(
        &self,
        fullscreen: windows::core::BOOL,
        ptarget: windows::core::Ref<IDXGIOutput>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.set_fullscreen_state(fullscreen, ptarget)
    }
    fn GetFullscreenState(
        &self,
        pfullscreen: *mut windows::core::BOOL,
        pptarget: windows::core::OutRef<IDXGIOutput>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.get_fullscreen_state(pfullscreen, pptarget)
    }
    fn GetDesc(&self) -> windows::core::Result<DXGI_SWAP_CHAIN_DESC> {
        let _lock = lock();
        self.get_desc()
    }
    fn ResizeBuffers(
        &self,
        buffercount: u32,
        width: u32,
        height: u32,
        newformat: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
        swapchainflags: &DXGI_SWAP_CHAIN_FLAG,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.resize_buffers(buffercount, width, height, newformat, swapchainflags)
    }
    fn ResizeTarget(
        &self,
        pnewtargetparameters: *const windows::Win32::Graphics::Dxgi::Common::DXGI_MODE_DESC,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISwapChain::ResizeTarget"))
    }
    fn GetContainingOutput(&self) -> windows::core::Result<IDXGIOutput> {
        let _lock = lock();
        Err(missing("IDXGISwapChain::GetContainingOutput"))
    }
    fn GetFrameStatistics(&self, pstats: *mut DXGI_FRAME_STATISTICS) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISwapChain::GetFrameStatistics"))
    }
    fn GetLastPresentCount(&self) -> windows::core::Result<u32> {
        let _lock = lock();
        self.get_last_present_count()
    }
}

impl IDXGIDeviceSubObject_Impl for SwapChain_Impl {
    fn GetDevice(
        &self,
        riid: *const windows::core::GUID,
        ppdevice: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.get_device(riid, ppdevice)
    }
}

impl IDXGIObject_Impl for SwapChain_Impl {
    fn SetPrivateData(
        &self,
        name: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.private.set_private_data(name, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        name: *const windows::core::GUID,
        punknown: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.private.set_private_data_interface(name, punknown)
    }
    fn GetPrivateData(
        &self,
        name: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.private.get_private_data(name, pdatasize, pdata)
    }
    fn GetParent(
        &self,
        riid: *const windows::core::GUID,
        ppparent: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.get_parent(riid, ppparent)
    }
}

impl ID3D11Buffer_Impl for Buffer_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_BUFFER_DESC) {
        let _lock = lock();
        self.get_desc(pdesc)
    }
}

impl ID3D11Resource_Impl for Buffer_Impl {
    fn GetType(&self, presourcedimension: *mut D3D11_RESOURCE_DIMENSION) {
        let _lock = lock();
        self.get_type(presourcedimension)
    }
    fn SetEvictionPriority(&self, evictionpriority: u32) {
        let _lock = lock();
        self.set_eviction_priority(evictionpriority)
    }
    fn GetEvictionPriority(&self) -> u32 {
        let _lock = lock();
        self.get_eviction_priority()
    }
}

impl ID3D11DeviceChild_Impl for Buffer_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl IDXGIResource1_Impl for Buffer_Impl {
    fn CreateSubresourceSurface(&self, index: u32) -> windows::core::Result<IDXGISurface2> {
        let _lock = lock();
        Err(missing("IDXGIResource1::CreateSubresourceSurface"))
    }
    fn CreateSharedHandle(
        &self,
        pattributes: *const windows::Win32::Security::SECURITY_ATTRIBUTES,
        dwaccess: u32,
        lpname: &windows::core::PCWSTR,
    ) -> windows::core::Result<windows::Win32::Foundation::HANDLE> {
        let _lock = lock();
        Err(missing("IDXGIResource1::CreateSharedHandle"))
    }
}

impl IDXGIResource_Impl for Buffer_Impl {
    fn GetSharedHandle(&self) -> windows::core::Result<windows::Win32::Foundation::HANDLE> {
        let _lock = lock();
        Err(missing("IDXGIResource::GetSharedHandle"))
    }
    fn GetUsage(&self) -> windows::core::Result<DXGI_USAGE> {
        let _lock = lock();
        self.get_usage()
    }
    fn SetEvictionPriority(
        &self,
        evictionpriority: DXGI_RESOURCE_PRIORITY,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        {
            self.set_eviction_priority(evictionpriority.0);
            Ok(())
        }
    }
    fn GetEvictionPriority(&self) -> windows::core::Result<DXGI_RESOURCE_PRIORITY> {
        let _lock = lock();
        Ok(DXGI_RESOURCE_PRIORITY(self.get_eviction_priority()))
    }
}

impl IDXGIDeviceSubObject_Impl for Buffer_Impl {
    fn GetDevice(
        &self,
        riid: *const windows::core::GUID,
        ppdevice: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.query_device(riid, ppdevice)
    }
}

impl IDXGIObject_Impl for Buffer_Impl {
    fn SetPrivateData(
        &self,
        name: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.private.set_private_data(name, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        name: *const windows::core::GUID,
        punknown: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child
            .private
            .set_private_data_interface(name, punknown)
    }
    fn GetPrivateData(
        &self,
        name: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.private.get_private_data(name, pdatasize, pdata)
    }
    fn GetParent(
        &self,
        riid: *const windows::core::GUID,
        ppparent: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.query_device(riid, ppparent)
    }
}

impl IDXGISurface1_Impl for Buffer_Impl {
    fn GetDC(
        &self,
        discard: windows::core::BOOL,
    ) -> windows::core::Result<windows::Win32::Graphics::Gdi::HDC> {
        let _lock = lock();
        Err(missing("IDXGISurface1::GetDC"))
    }
    fn ReleaseDC(
        &self,
        pdirtyrect: *const windows::Win32::Foundation::RECT,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISurface1::ReleaseDC"))
    }
}

impl IDXGISurface_Impl for Buffer_Impl {
    fn GetDesc(&self) -> windows::core::Result<DXGI_SURFACE_DESC> {
        let _lock = lock();
        self.get_desc_dxgi()
    }
    fn Map(
        &self,
        plockedrect: *mut DXGI_MAPPED_RECT,
        mapflags: DXGI_MAP_FLAGS,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISurface::Map"))
    }
    fn Unmap(&self) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISurface::Unmap"))
    }
}

impl ID3D10Buffer_Impl for Buffer_Impl {
    fn Map(
        &self,
        maptype: D3D10_MAP,
        mapflags: u32,
        ppdata: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("ID3D10Buffer::Map"))
    }
    fn Unmap(&self) {
        let _lock = lock();
        missing_void("ID3D10Buffer::Unmap")
    }
    fn GetDesc(&self, pdesc: *mut D3D10_BUFFER_DESC) {
        let _lock = lock();
        self.get_desc10(pdesc)
    }
}

impl ID3D10Resource_Impl for Buffer_Impl {
    fn GetType(&self, rtype: *mut D3D10_RESOURCE_DIMENSION) {
        let _lock = lock();
        self.get_type10(rtype)
    }
    fn SetEvictionPriority(&self, evictionpriority: u32) {
        let _lock = lock();
        self.set_eviction_priority(evictionpriority)
    }
    fn GetEvictionPriority(&self) -> u32 {
        let _lock = lock();
        self.get_eviction_priority()
    }
}

impl ID3D10DeviceChild_Impl for Buffer_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D10Device>) {
        let _lock = lock();
        self.child.get_device10(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11Texture1D_Impl for Texture1D_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_TEXTURE1D_DESC) {
        let _lock = lock();
        self.get_desc(pdesc)
    }
}

impl ID3D11Resource_Impl for Texture1D_Impl {
    fn GetType(&self, presourcedimension: *mut D3D11_RESOURCE_DIMENSION) {
        let _lock = lock();
        self.get_type(presourcedimension)
    }
    fn SetEvictionPriority(&self, evictionpriority: u32) {
        let _lock = lock();
        self.set_eviction_priority(evictionpriority)
    }
    fn GetEvictionPriority(&self) -> u32 {
        let _lock = lock();
        self.get_eviction_priority()
    }
}

impl ID3D11DeviceChild_Impl for Texture1D_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl IDXGIResource1_Impl for Texture1D_Impl {
    fn CreateSubresourceSurface(&self, index: u32) -> windows::core::Result<IDXGISurface2> {
        let _lock = lock();
        Err(missing("IDXGIResource1::CreateSubresourceSurface"))
    }
    fn CreateSharedHandle(
        &self,
        pattributes: *const windows::Win32::Security::SECURITY_ATTRIBUTES,
        dwaccess: u32,
        lpname: &windows::core::PCWSTR,
    ) -> windows::core::Result<windows::Win32::Foundation::HANDLE> {
        let _lock = lock();
        Err(missing("IDXGIResource1::CreateSharedHandle"))
    }
}

impl IDXGIResource_Impl for Texture1D_Impl {
    fn GetSharedHandle(&self) -> windows::core::Result<windows::Win32::Foundation::HANDLE> {
        let _lock = lock();
        Err(missing("IDXGIResource::GetSharedHandle"))
    }
    fn GetUsage(&self) -> windows::core::Result<DXGI_USAGE> {
        let _lock = lock();
        self.get_usage()
    }
    fn SetEvictionPriority(
        &self,
        evictionpriority: DXGI_RESOURCE_PRIORITY,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        {
            self.set_eviction_priority(evictionpriority.0);
            Ok(())
        }
    }
    fn GetEvictionPriority(&self) -> windows::core::Result<DXGI_RESOURCE_PRIORITY> {
        let _lock = lock();
        Ok(DXGI_RESOURCE_PRIORITY(self.get_eviction_priority()))
    }
}

impl IDXGIDeviceSubObject_Impl for Texture1D_Impl {
    fn GetDevice(
        &self,
        riid: *const windows::core::GUID,
        ppdevice: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.query_device(riid, ppdevice)
    }
}

impl IDXGIObject_Impl for Texture1D_Impl {
    fn SetPrivateData(
        &self,
        name: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.private.set_private_data(name, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        name: *const windows::core::GUID,
        punknown: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child
            .private
            .set_private_data_interface(name, punknown)
    }
    fn GetPrivateData(
        &self,
        name: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.private.get_private_data(name, pdatasize, pdata)
    }
    fn GetParent(
        &self,
        riid: *const windows::core::GUID,
        ppparent: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.query_device(riid, ppparent)
    }
}

impl IDXGISurface1_Impl for Texture1D_Impl {
    fn GetDC(
        &self,
        discard: windows::core::BOOL,
    ) -> windows::core::Result<windows::Win32::Graphics::Gdi::HDC> {
        let _lock = lock();
        Err(missing("IDXGISurface1::GetDC"))
    }
    fn ReleaseDC(
        &self,
        pdirtyrect: *const windows::Win32::Foundation::RECT,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISurface1::ReleaseDC"))
    }
}

impl IDXGISurface_Impl for Texture1D_Impl {
    fn GetDesc(&self) -> windows::core::Result<DXGI_SURFACE_DESC> {
        let _lock = lock();
        self.get_desc_dxgi()
    }
    fn Map(
        &self,
        plockedrect: *mut DXGI_MAPPED_RECT,
        mapflags: DXGI_MAP_FLAGS,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISurface::Map"))
    }
    fn Unmap(&self) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISurface::Unmap"))
    }
}

impl ID3D10Texture1D_Impl for Texture1D_Impl {
    fn Map(
        &self,
        subresource: u32,
        maptype: D3D10_MAP,
        mapflags: u32,
        ppdata: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("ID3D10Texture1D::Map"))
    }
    fn Unmap(&self, subresource: u32) {
        let _lock = lock();
        missing_void("ID3D10Texture1D::Unmap")
    }
    fn GetDesc(&self, pdesc: *mut D3D10_TEXTURE1D_DESC) {
        let _lock = lock();
        self.get_desc10(pdesc)
    }
}

impl ID3D10Resource_Impl for Texture1D_Impl {
    fn GetType(&self, rtype: *mut D3D10_RESOURCE_DIMENSION) {
        let _lock = lock();
        self.get_type10(rtype)
    }
    fn SetEvictionPriority(&self, evictionpriority: u32) {
        let _lock = lock();
        self.set_eviction_priority(evictionpriority)
    }
    fn GetEvictionPriority(&self) -> u32 {
        let _lock = lock();
        self.get_eviction_priority()
    }
}

impl ID3D10DeviceChild_Impl for Texture1D_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D10Device>) {
        let _lock = lock();
        self.child.get_device10(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11Texture2D1_Impl for Texture2D_Impl {
    fn GetDesc1(&self, pdesc: *mut D3D11_TEXTURE2D_DESC1) {
        let _lock = lock();
        self.get_desc1(pdesc)
    }
}

impl ID3D11Texture2D_Impl for Texture2D_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_TEXTURE2D_DESC) {
        let _lock = lock();
        self.get_desc(pdesc)
    }
}

impl ID3D11Resource_Impl for Texture2D_Impl {
    fn GetType(&self, presourcedimension: *mut D3D11_RESOURCE_DIMENSION) {
        let _lock = lock();
        self.get_type(presourcedimension)
    }
    fn SetEvictionPriority(&self, evictionpriority: u32) {
        let _lock = lock();
        self.set_eviction_priority(evictionpriority)
    }
    fn GetEvictionPriority(&self) -> u32 {
        let _lock = lock();
        self.get_eviction_priority()
    }
}

impl ID3D11DeviceChild_Impl for Texture2D_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl IDXGIResource1_Impl for Texture2D_Impl {
    fn CreateSubresourceSurface(&self, index: u32) -> windows::core::Result<IDXGISurface2> {
        let _lock = lock();
        Err(missing("IDXGIResource1::CreateSubresourceSurface"))
    }
    fn CreateSharedHandle(
        &self,
        pattributes: *const windows::Win32::Security::SECURITY_ATTRIBUTES,
        dwaccess: u32,
        lpname: &windows::core::PCWSTR,
    ) -> windows::core::Result<windows::Win32::Foundation::HANDLE> {
        let _lock = lock();
        Err(missing("IDXGIResource1::CreateSharedHandle"))
    }
}

impl IDXGIResource_Impl for Texture2D_Impl {
    fn GetSharedHandle(&self) -> windows::core::Result<windows::Win32::Foundation::HANDLE> {
        let _lock = lock();
        Err(missing("IDXGIResource::GetSharedHandle"))
    }
    fn GetUsage(&self) -> windows::core::Result<DXGI_USAGE> {
        let _lock = lock();
        self.get_usage()
    }
    fn SetEvictionPriority(
        &self,
        evictionpriority: DXGI_RESOURCE_PRIORITY,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        {
            self.set_eviction_priority(evictionpriority.0);
            Ok(())
        }
    }
    fn GetEvictionPriority(&self) -> windows::core::Result<DXGI_RESOURCE_PRIORITY> {
        let _lock = lock();
        Ok(DXGI_RESOURCE_PRIORITY(self.get_eviction_priority()))
    }
}

impl IDXGIDeviceSubObject_Impl for Texture2D_Impl {
    fn GetDevice(
        &self,
        riid: *const windows::core::GUID,
        ppdevice: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.query_device(riid, ppdevice)
    }
}

impl IDXGIObject_Impl for Texture2D_Impl {
    fn SetPrivateData(
        &self,
        name: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.private.set_private_data(name, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        name: *const windows::core::GUID,
        punknown: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child
            .private
            .set_private_data_interface(name, punknown)
    }
    fn GetPrivateData(
        &self,
        name: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.private.get_private_data(name, pdatasize, pdata)
    }
    fn GetParent(
        &self,
        riid: *const windows::core::GUID,
        ppparent: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.query_device(riid, ppparent)
    }
}

impl IDXGISurface1_Impl for Texture2D_Impl {
    fn GetDC(
        &self,
        discard: windows::core::BOOL,
    ) -> windows::core::Result<windows::Win32::Graphics::Gdi::HDC> {
        let _lock = lock();
        Err(missing("IDXGISurface1::GetDC"))
    }
    fn ReleaseDC(
        &self,
        pdirtyrect: *const windows::Win32::Foundation::RECT,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISurface1::ReleaseDC"))
    }
}

impl IDXGISurface_Impl for Texture2D_Impl {
    fn GetDesc(&self) -> windows::core::Result<DXGI_SURFACE_DESC> {
        let _lock = lock();
        self.get_desc_dxgi()
    }
    fn Map(
        &self,
        plockedrect: *mut DXGI_MAPPED_RECT,
        mapflags: DXGI_MAP_FLAGS,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISurface::Map"))
    }
    fn Unmap(&self) -> windows::core::Result<()> {
        let _lock = lock();
        Err(missing("IDXGISurface::Unmap"))
    }
}

impl ID3D10Texture2D_Impl for Texture2D_Impl {
    fn Map(
        &self,
        subresource: u32,
        maptype: D3D10_MAP,
        mapflags: u32,
    ) -> windows::core::Result<D3D10_MAPPED_TEXTURE2D> {
        let _lock = lock();
        Err(missing("ID3D10Texture2D::Map"))
    }
    fn Unmap(&self, subresource: u32) {
        let _lock = lock();
        missing_void("ID3D10Texture2D::Unmap")
    }
    fn GetDesc(&self, pdesc: *mut D3D10_TEXTURE2D_DESC) {
        let _lock = lock();
        self.get_desc10(pdesc)
    }
}

impl ID3D10Resource_Impl for Texture2D_Impl {
    fn GetType(&self, rtype: *mut D3D10_RESOURCE_DIMENSION) {
        let _lock = lock();
        self.get_type10(rtype)
    }
    fn SetEvictionPriority(&self, evictionpriority: u32) {
        let _lock = lock();
        self.set_eviction_priority(evictionpriority)
    }
    fn GetEvictionPriority(&self) -> u32 {
        let _lock = lock();
        self.get_eviction_priority()
    }
}

impl ID3D10DeviceChild_Impl for Texture2D_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D10Device>) {
        let _lock = lock();
        self.child.get_device10(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11Texture3D1_Impl for Texture3D_Impl {
    fn GetDesc1(&self, pdesc: *mut D3D11_TEXTURE3D_DESC1) {
        let _lock = lock();
        self.get_desc1(pdesc)
    }
}

impl ID3D11Texture3D_Impl for Texture3D_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_TEXTURE3D_DESC) {
        let _lock = lock();
        self.get_desc(pdesc)
    }
}

impl ID3D11Resource_Impl for Texture3D_Impl {
    fn GetType(&self, presourcedimension: *mut D3D11_RESOURCE_DIMENSION) {
        let _lock = lock();
        self.get_type(presourcedimension)
    }
    fn SetEvictionPriority(&self, evictionpriority: u32) {
        let _lock = lock();
        self.set_eviction_priority(evictionpriority)
    }
    fn GetEvictionPriority(&self) -> u32 {
        let _lock = lock();
        self.get_eviction_priority()
    }
}

impl ID3D11DeviceChild_Impl for Texture3D_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl IDXGIResource1_Impl for Texture3D_Impl {
    fn CreateSubresourceSurface(&self, index: u32) -> windows::core::Result<IDXGISurface2> {
        let _lock = lock();
        Err(missing("IDXGIResource1::CreateSubresourceSurface"))
    }
    fn CreateSharedHandle(
        &self,
        pattributes: *const windows::Win32::Security::SECURITY_ATTRIBUTES,
        dwaccess: u32,
        lpname: &windows::core::PCWSTR,
    ) -> windows::core::Result<windows::Win32::Foundation::HANDLE> {
        let _lock = lock();
        Err(missing("IDXGIResource1::CreateSharedHandle"))
    }
}

impl IDXGIResource_Impl for Texture3D_Impl {
    fn GetSharedHandle(&self) -> windows::core::Result<windows::Win32::Foundation::HANDLE> {
        let _lock = lock();
        Err(missing("IDXGIResource::GetSharedHandle"))
    }
    fn GetUsage(&self) -> windows::core::Result<DXGI_USAGE> {
        let _lock = lock();
        self.get_usage()
    }
    fn SetEvictionPriority(
        &self,
        evictionpriority: DXGI_RESOURCE_PRIORITY,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        {
            self.set_eviction_priority(evictionpriority.0);
            Ok(())
        }
    }
    fn GetEvictionPriority(&self) -> windows::core::Result<DXGI_RESOURCE_PRIORITY> {
        let _lock = lock();
        Ok(DXGI_RESOURCE_PRIORITY(self.get_eviction_priority()))
    }
}

impl IDXGIDeviceSubObject_Impl for Texture3D_Impl {
    fn GetDevice(
        &self,
        riid: *const windows::core::GUID,
        ppdevice: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.query_device(riid, ppdevice)
    }
}

impl IDXGIObject_Impl for Texture3D_Impl {
    fn SetPrivateData(
        &self,
        name: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.private.set_private_data(name, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        name: *const windows::core::GUID,
        punknown: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child
            .private
            .set_private_data_interface(name, punknown)
    }
    fn GetPrivateData(
        &self,
        name: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.private.get_private_data(name, pdatasize, pdata)
    }
    fn GetParent(
        &self,
        riid: *const windows::core::GUID,
        ppparent: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.query_device(riid, ppparent)
    }
}

impl ID3D10Texture3D_Impl for Texture3D_Impl {
    fn Map(
        &self,
        subresource: u32,
        maptype: D3D10_MAP,
        mapflags: u32,
    ) -> windows::core::Result<D3D10_MAPPED_TEXTURE3D> {
        let _lock = lock();
        Err(missing("ID3D10Texture3D::Map"))
    }
    fn Unmap(&self, subresource: u32) {
        let _lock = lock();
        missing_void("ID3D10Texture3D::Unmap")
    }
    fn GetDesc(&self, pdesc: *mut D3D10_TEXTURE3D_DESC) {
        let _lock = lock();
        self.get_desc10(pdesc)
    }
}

impl ID3D10Resource_Impl for Texture3D_Impl {
    fn GetType(&self, rtype: *mut D3D10_RESOURCE_DIMENSION) {
        let _lock = lock();
        self.get_type10(rtype)
    }
    fn SetEvictionPriority(&self, evictionpriority: u32) {
        let _lock = lock();
        self.set_eviction_priority(evictionpriority)
    }
    fn GetEvictionPriority(&self) -> u32 {
        let _lock = lock();
        self.get_eviction_priority()
    }
}

impl ID3D10DeviceChild_Impl for Texture3D_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D10Device>) {
        let _lock = lock();
        self.child.get_device10(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11RenderTargetView1_Impl for RenderTargetView_Impl {
    fn GetDesc1(&self, pdesc1: *mut D3D11_RENDER_TARGET_VIEW_DESC1) {
        let _lock = lock();
        self.get_desc1(pdesc1)
    }
}

impl ID3D11RenderTargetView_Impl for RenderTargetView_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_RENDER_TARGET_VIEW_DESC) {
        let _lock = lock();
        self.get_desc(pdesc)
    }
}

impl ID3D11View_Impl for RenderTargetView_Impl {
    fn GetResource(&self, ppresource: windows::core::OutRef<ID3D11Resource>) {
        let _lock = lock();
        self.get_resource(ppresource)
    }
}

impl ID3D11DeviceChild_Impl for RenderTargetView_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D10RenderTargetView_Impl for RenderTargetView_Impl {
    fn GetDesc(&self, pdesc: *mut D3D10_RENDER_TARGET_VIEW_DESC) {
        let _lock = lock();
        self.get_desc10(pdesc)
    }
}

impl ID3D10View_Impl for RenderTargetView_Impl {
    fn GetResource(&self, ppresource: windows::core::OutRef<ID3D10Resource>) {
        let _lock = lock();
        self.get_resource10(ppresource)
    }
}

impl ID3D10DeviceChild_Impl for RenderTargetView_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D10Device>) {
        let _lock = lock();
        self.child.get_device10(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11DepthStencilView_Impl for DepthStencilView_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_DEPTH_STENCIL_VIEW_DESC) {
        let _lock = lock();
        self.get_desc(pdesc)
    }
}

impl ID3D11View_Impl for DepthStencilView_Impl {
    fn GetResource(&self, ppresource: windows::core::OutRef<ID3D11Resource>) {
        let _lock = lock();
        self.get_resource(ppresource)
    }
}

impl ID3D11DeviceChild_Impl for DepthStencilView_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D10DepthStencilView_Impl for DepthStencilView_Impl {
    fn GetDesc(&self, pdesc: *mut D3D10_DEPTH_STENCIL_VIEW_DESC) {
        let _lock = lock();
        self.get_desc10(pdesc)
    }
}

impl ID3D10View_Impl for DepthStencilView_Impl {
    fn GetResource(&self, ppresource: windows::core::OutRef<ID3D10Resource>) {
        let _lock = lock();
        self.get_resource10(ppresource)
    }
}

impl ID3D10DeviceChild_Impl for DepthStencilView_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D10Device>) {
        let _lock = lock();
        self.child.get_device10(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11ShaderResourceView1_Impl for ShaderResourceView_Impl {
    fn GetDesc1(&self, pdesc1: *mut D3D11_SHADER_RESOURCE_VIEW_DESC1) {
        let _lock = lock();
        self.get_desc1(pdesc1)
    }
}

impl ID3D11ShaderResourceView_Impl for ShaderResourceView_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_SHADER_RESOURCE_VIEW_DESC) {
        let _lock = lock();
        self.get_desc(pdesc)
    }
}

impl ID3D11View_Impl for ShaderResourceView_Impl {
    fn GetResource(&self, ppresource: windows::core::OutRef<ID3D11Resource>) {
        let _lock = lock();
        self.get_resource(ppresource)
    }
}

impl ID3D11DeviceChild_Impl for ShaderResourceView_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D10ShaderResourceView1_Impl for ShaderResourceView_Impl {
    fn GetDesc1(&self, pdesc: *mut D3D10_SHADER_RESOURCE_VIEW_DESC1) {
        let _lock = lock();
        self.get_desc110(pdesc)
    }
}

impl ID3D10ShaderResourceView_Impl for ShaderResourceView_Impl {
    fn GetDesc(&self, pdesc: *mut D3D10_SHADER_RESOURCE_VIEW_DESC) {
        let _lock = lock();
        self.get_desc10(pdesc)
    }
}

impl ID3D10View_Impl for ShaderResourceView_Impl {
    fn GetResource(&self, ppresource: windows::core::OutRef<ID3D10Resource>) {
        let _lock = lock();
        self.get_resource10(ppresource)
    }
}

impl ID3D10DeviceChild_Impl for ShaderResourceView_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D10Device>) {
        let _lock = lock();
        self.child.get_device10(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11UnorderedAccessView1_Impl for UnorderedAccessView_Impl {
    fn GetDesc1(&self, pdesc1: *mut D3D11_UNORDERED_ACCESS_VIEW_DESC1) {
        let _lock = lock();
        self.get_desc1(pdesc1)
    }
}

impl ID3D11UnorderedAccessView_Impl for UnorderedAccessView_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_UNORDERED_ACCESS_VIEW_DESC) {
        let _lock = lock();
        self.get_desc(pdesc)
    }
}

impl ID3D11View_Impl for UnorderedAccessView_Impl {
    fn GetResource(&self, ppresource: windows::core::OutRef<ID3D11Resource>) {
        let _lock = lock();
        self.get_resource(ppresource)
    }
}

impl ID3D11DeviceChild_Impl for UnorderedAccessView_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11SamplerState_Impl for SamplerState_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_SAMPLER_DESC) {
        let _lock = lock();
        self.get_desc(pdesc)
    }
}

impl ID3D11DeviceChild_Impl for SamplerState_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D10SamplerState_Impl for SamplerState_Impl {
    fn GetDesc(&self, pdesc: *mut D3D10_SAMPLER_DESC) {
        let _lock = lock();
        self.get_desc10(pdesc)
    }
}

impl ID3D10DeviceChild_Impl for SamplerState_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D10Device>) {
        let _lock = lock();
        self.child.get_device10(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11BlendState1_Impl for BlendState_Impl {
    fn GetDesc1(&self, pdesc: *mut D3D11_BLEND_DESC1) {
        let _lock = lock();
        self.get_desc1(pdesc)
    }
}

impl ID3D11BlendState_Impl for BlendState_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_BLEND_DESC) {
        let _lock = lock();
        self.get_desc(pdesc)
    }
}

impl ID3D11DeviceChild_Impl for BlendState_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D10BlendState1_Impl for BlendState_Impl {
    fn GetDesc1(&self, pdesc: *mut D3D10_BLEND_DESC1) {
        let _lock = lock();
        self.get_desc110(pdesc)
    }
}

impl ID3D10BlendState_Impl for BlendState_Impl {
    fn GetDesc(&self, pdesc: *mut D3D10_BLEND_DESC) {
        let _lock = lock();
        self.get_desc10(pdesc)
    }
}

impl ID3D10DeviceChild_Impl for BlendState_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D10Device>) {
        let _lock = lock();
        self.child.get_device10(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11DepthStencilState_Impl for DepthStencilState_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_DEPTH_STENCIL_DESC) {
        let _lock = lock();
        self.get_desc(pdesc)
    }
}

impl ID3D11DeviceChild_Impl for DepthStencilState_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D10DepthStencilState_Impl for DepthStencilState_Impl {
    fn GetDesc(&self, pdesc: *mut D3D10_DEPTH_STENCIL_DESC) {
        let _lock = lock();
        self.get_desc10(pdesc)
    }
}

impl ID3D10DeviceChild_Impl for DepthStencilState_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D10Device>) {
        let _lock = lock();
        self.child.get_device10(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11RasterizerState2_Impl for RasterizerState_Impl {
    fn GetDesc2(&self, pdesc: *mut D3D11_RASTERIZER_DESC2) {
        let _lock = lock();
        self.get_desc2(pdesc)
    }
}

impl ID3D11RasterizerState1_Impl for RasterizerState_Impl {
    fn GetDesc1(&self, pdesc: *mut D3D11_RASTERIZER_DESC1) {
        let _lock = lock();
        self.get_desc1(pdesc)
    }
}

impl ID3D11RasterizerState_Impl for RasterizerState_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_RASTERIZER_DESC) {
        let _lock = lock();
        self.get_desc(pdesc)
    }
}

impl ID3D11DeviceChild_Impl for RasterizerState_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D10RasterizerState_Impl for RasterizerState_Impl {
    fn GetDesc(&self, pdesc: *mut D3D10_RASTERIZER_DESC) {
        let _lock = lock();
        self.get_desc10(pdesc)
    }
}

impl ID3D10DeviceChild_Impl for RasterizerState_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D10Device>) {
        let _lock = lock();
        self.child.get_device10(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11InputLayout_Impl for InputLayout_Impl {}

impl ID3D11DeviceChild_Impl for InputLayout_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11Query1_Impl for Query_Impl {
    fn GetDesc1(&self, pdesc1: *mut D3D11_QUERY_DESC1) {
        let _lock = lock();
        self.get_desc1(pdesc1)
    }
}

impl ID3D11Query_Impl for Query_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_QUERY_DESC) {
        let _lock = lock();
        self.get_desc(pdesc)
    }
}

impl ID3D11Asynchronous_Impl for Query_Impl {
    fn GetDataSize(&self) -> u32 {
        let _lock = lock();
        self.get_data_size()
    }
}

impl ID3D11DeviceChild_Impl for Query_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11Predicate_Impl for Predicate_Impl {}

impl ID3D11Query_Impl for Predicate_Impl {
    fn GetDesc(&self, pdesc: *mut D3D11_QUERY_DESC) {
        let _lock = lock();
        self.get_desc(pdesc)
    }
}

impl ID3D11Asynchronous_Impl for Predicate_Impl {
    fn GetDataSize(&self) -> u32 {
        let _lock = lock();
        self.get_data_size()
    }
}

impl ID3D11DeviceChild_Impl for Predicate_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11VertexShader_Impl for VertexShader_Impl {}

impl ID3D11DeviceChild_Impl for VertexShader_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11PixelShader_Impl for PixelShader_Impl {}

impl ID3D11DeviceChild_Impl for PixelShader_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11GeometryShader_Impl for GeometryShader_Impl {}

impl ID3D11DeviceChild_Impl for GeometryShader_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11ComputeShader_Impl for ComputeShader_Impl {}

impl ID3D11DeviceChild_Impl for ComputeShader_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3D11CommandList_Impl for CommandList_Impl {
    fn GetContextFlags(&self) -> u32 {
        let _lock = lock();
        self.get_context_flags()
    }
}

impl ID3D11DeviceChild_Impl for CommandList_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}

impl ID3DDeviceContextState_Impl for DeviceContextState_Impl {}

impl ID3D11DeviceChild_Impl for DeviceContextState_Impl {
    fn GetDevice(&self, ppdevice: windows::core::OutRef<ID3D11Device>) {
        let _lock = lock();
        self.child.get_device(ppdevice)
    }
    fn GetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        pdatasize: *mut u32,
        pdata: *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.get_private_data(guid, pdatasize, pdata)
    }
    fn SetPrivateData(
        &self,
        guid: *const windows::core::GUID,
        datasize: u32,
        pdata: *const core::ffi::c_void,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data(guid, datasize, pdata)
    }
    fn SetPrivateDataInterface(
        &self,
        guid: *const windows::core::GUID,
        pdata: windows::core::Ref<windows::core::IUnknown>,
    ) -> windows::core::Result<()> {
        let _lock = lock();
        self.child.set_private_data_interface(guid, pdata)
    }
}
