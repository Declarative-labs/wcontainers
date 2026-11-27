//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The library d3d11.dll, and the debug layer's interfaces its objects answer.

use wcontainers_component_object_model as component_object_model;
use wcontainers_direct3d_common as direct3d_common;
use wcontainers_directx_graphics_infrastructure as directx_graphics_infrastructure;

use wcontainers_component_object_model::structures::HResult;

pub mod constants;
pub mod enumerations;
pub mod structures;

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The library d3d11.dll, and the debug layer's interfaces its objects answer.
pub trait Direct3D11:
    Sized
    + direct3d_common::Direct3DCommon
    + directx_graphics_infrastructure::DirectXGraphicsInfrastructure
{
    type AuthenticatedChannel: AuthenticatedChannel<Api = Self>
        + direct3d_common::DestructionNotifier;
    type BlendState: BlendState1<Api = Self> + direct3d_common::DestructionNotifier;
    type Buffer: Buffer<Api = Self>
        + directx_graphics_infrastructure::Resource1<Api = Self>
        + direct3d_common::DestructionNotifier;
    type ClassInstance: ClassInstance<Api = Self> + direct3d_common::DestructionNotifier;
    type ClassLinkage: ClassLinkage<Api = Self> + direct3d_common::DestructionNotifier;
    type CommandList: CommandList<Api = Self> + direct3d_common::DestructionNotifier;
    type ComputeShader: ComputeShader<Api = Self> + direct3d_common::DestructionNotifier;
    type Counter: Counter<Api = Self> + direct3d_common::DestructionNotifier;
    type CryptoSession: CryptoSession<Api = Self> + direct3d_common::DestructionNotifier;
    type Debug: Debug<Api = Self> + direct3d_common::DestructionNotifier;
    type DepthStencilState: DepthStencilState<Api = Self> + direct3d_common::DestructionNotifier;
    type DepthStencilView: DepthStencilView<Api = Self> + direct3d_common::DestructionNotifier;
    type Device: Device5<Api = Self>
        + directx_graphics_infrastructure::Device4<Api = Self>
        + Multithread
        + direct3d_common::DestructionNotifier;
    type DeviceContext: DeviceContext4<Api = Self>
        + Multithread
        + UserDefinedAnnotation
        + direct3d_common::DestructionNotifier;
    type DomainShader: DomainShader<Api = Self> + direct3d_common::DestructionNotifier;
    type Fence: Fence<Api = Self> + direct3d_common::DestructionNotifier;
    type GeometryShader: GeometryShader<Api = Self> + direct3d_common::DestructionNotifier;
    type HullShader: HullShader<Api = Self> + direct3d_common::DestructionNotifier;
    type InfoQueue: InfoQueue + direct3d_common::DestructionNotifier;
    type InputLayout: InputLayout<Api = Self> + direct3d_common::DestructionNotifier;
    type Multithread: Multithread + direct3d_common::DestructionNotifier;
    type PixelShader: PixelShader<Api = Self> + direct3d_common::DestructionNotifier;
    type Predicate: Predicate<Api = Self> + direct3d_common::DestructionNotifier;
    type Query: Query1<Api = Self> + direct3d_common::DestructionNotifier;
    type RasterizerState: RasterizerState2<Api = Self> + direct3d_common::DestructionNotifier;
    type RefDefaultTrackingOptions: RefDefaultTrackingOptions + direct3d_common::DestructionNotifier;
    type RefTrackingOptions: RefTrackingOptions + direct3d_common::DestructionNotifier;
    type RenderTargetView: RenderTargetView1<Api = Self> + direct3d_common::DestructionNotifier;
    type SamplerState: SamplerState<Api = Self> + direct3d_common::DestructionNotifier;
    type ShaderResourceView: ShaderResourceView1<Api = Self> + direct3d_common::DestructionNotifier;
    type ShaderTrace: ShaderTrace + direct3d_common::DestructionNotifier;
    type ShaderTraceFactory: ShaderTraceFactory<Api = Self> + direct3d_common::DestructionNotifier;
    type SwitchToRef: SwitchToRef + direct3d_common::DestructionNotifier;
    type Texture1D: Texture1D<Api = Self>
        + directx_graphics_infrastructure::Resource1<Api = Self>
        + direct3d_common::DestructionNotifier;
    type Texture2D: Texture2D1<Api = Self>
        + directx_graphics_infrastructure::Resource1<Api = Self>
        + directx_graphics_infrastructure::Surface2<Api = Self>
        + direct3d_common::DestructionNotifier;
    type Texture3D: Texture3D1<Api = Self>
        + directx_graphics_infrastructure::Resource1<Api = Self>
        + direct3d_common::DestructionNotifier;
    type TracingDevice: TracingDevice<Api = Self> + direct3d_common::DestructionNotifier;
    type UnorderedAccessView: UnorderedAccessView1<Api = Self>
        + direct3d_common::DestructionNotifier;
    type VertexShader: VertexShader<Api = Self> + direct3d_common::DestructionNotifier;
    type VideoContext: VideoContext3<Api = Self> + direct3d_common::DestructionNotifier;
    type VideoDecoder: VideoDecoder<Api = Self> + direct3d_common::DestructionNotifier;
    type VideoDecoderOutputView: VideoDecoderOutputView<Api = Self>
        + direct3d_common::DestructionNotifier;
    type VideoDevice: VideoDevice2<Api = Self> + direct3d_common::DestructionNotifier;
    type VideoProcessor: VideoProcessor<Api = Self> + direct3d_common::DestructionNotifier;
    type VideoProcessorEnumerator: VideoProcessorEnumerator1<Api = Self>
        + direct3d_common::DestructionNotifier;
    type VideoProcessorInputView: VideoProcessorInputView<Api = Self>
        + direct3d_common::DestructionNotifier;
    type VideoProcessorOutputView: VideoProcessorOutputView<Api = Self>
        + direct3d_common::DestructionNotifier;
    type DeviceContextState: DeviceContextState<Api = Self> + direct3d_common::DestructionNotifier;
    type UserDefinedAnnotation: UserDefinedAnnotation + direct3d_common::DestructionNotifier;
    /// Memory a Map lends until its Unmap, which the caller may read and write meanwhile (an exception line of
    /// src/api.rs).
    type Mapped: wcontainers_volatile_bytes::VolatileBytes + Send + Sync + 'static;

    /// The header's `CreateDevice`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-d3d11createdevice).
    fn create_device(
        &self,
        adapter: Option<
            &<Self as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        >,
        driver_type: direct3d_common::enumerations::DriverType,
        software: component_object_model::structures::Hmodule,
        flags: enumerations::CreateDeviceFlag,
        feature_levels: &[direct3d_common::enumerations::FeatureLevel],
        sdk_version: u32,
    ) -> Result<
        (
            <Self as Direct3D11>::Device,
            direct3d_common::enumerations::FeatureLevel,
            <Self as Direct3D11>::DeviceContext,
        ),
        HResult,
    >;
    /// The header's `CreateDeviceAndSwapChain`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-d3d11createdeviceandswapchain).
    #[allow(
        clippy::too_many_arguments,
        reason = "the method's own parameters, one to one (MAPPING.md)"
    )]
    fn create_device_and_swap_chain(
        &self,
        adapter: Option<
            &<Self as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        >,
        driver_type: direct3d_common::enumerations::DriverType,
        software: component_object_model::structures::Hmodule,
        flags: enumerations::CreateDeviceFlag,
        feature_levels: &[direct3d_common::enumerations::FeatureLevel],
        sdk_version: u32,
        swap_chain_desc: Option<&directx_graphics_infrastructure::structures::SwapChainDesc>,
    ) -> Result<
        (
            <Self as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
            <Self as Direct3D11>::Device,
            direct3d_common::enumerations::FeatureLevel,
            <Self as Direct3D11>::DeviceContext,
        ),
        HResult,
    >;
}

/// The header's `ID3D11Asynchronous`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11asynchronous).
pub trait Asynchronous: DeviceChild {
    /// The header's `GetDataSize`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11asynchronous-getdatasize).
    fn get_data_size(&self) -> u32;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11AuthenticatedChannel`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11authenticatedchannel).
pub trait AuthenticatedChannel: DeviceChild {
    /// The header's `GetCertificateSize`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11authenticatedchannel-getcertificatesize).
    fn get_certificate_size(&self) -> Result<u32, HResult>;
    /// The header's `GetCertificate`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11authenticatedchannel-getcertificate).
    fn get_certificate(
        &self,
        certificate: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), HResult>;
    /// The header's `GetChannelHandle`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11authenticatedchannel-getchannelhandle).
    fn get_channel_handle(&self) -> component_object_model::structures::Handle;
}

/// The header's `ID3D11BlendState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11blendstate).
pub trait BlendState: DeviceChild {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11blendstate-getdesc).
    fn get_desc(&self) -> structures::BlendDesc;
}

/// The header's `ID3D11BlendState1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3d11blendstate1).
pub trait BlendState1: BlendState {
    /// The header's `GetDesc1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11blendstate1-getdesc1).
    fn get_desc1(&self) -> structures::BlendDesc1;
}

/// The header's `ID3D11Buffer`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11buffer).
pub trait Buffer: Resource {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11buffer-getdesc).
    fn get_desc(&self) -> structures::BufferDesc;
}

/// The header's `ID3D11ClassInstance`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11classinstance).
pub trait ClassInstance: DeviceChild {
    /// The header's `GetClassLinkage`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11classinstance-getclasslinkage).
    fn get_class_linkage(&self) -> Option<<Self::Api as Direct3D11>::ClassLinkage>;
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11classinstance-getdesc).
    fn get_desc(&self) -> structures::ClassInstanceDesc;
    /// The header's `GetInstanceName`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11classinstance-getinstancename).
    fn get_instance_name(&self, instance_name: Option<&std::ffi::CStr>);
    /// The header's `GetTypeName`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11classinstance-gettypename).
    fn get_type_name(&self, type_name: Option<&std::ffi::CStr>);
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11ClassLinkage`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11classlinkage).
pub trait ClassLinkage: DeviceChild {
    /// The header's `GetClassInstance`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11classlinkage-getclassinstance).
    fn get_class_instance(
        &self,
        class_instance_name: &std::ffi::CStr,
        instance_index: u32,
    ) -> Result<<Self::Api as Direct3D11>::ClassInstance, HResult>;
    /// The header's `CreateClassInstance`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11classlinkage-createclassinstance).
    fn create_class_instance(
        &self,
        class_type_name: &std::ffi::CStr,
        constant_buffer_offset: u32,
        constant_vector_offset: u32,
        texture_offset: u32,
        sampler_offset: u32,
    ) -> Result<<Self::Api as Direct3D11>::ClassInstance, HResult>;
}

/// The header's `ID3D11CommandList`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11commandlist).
pub trait CommandList: DeviceChild {
    /// The header's `GetContextFlags`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11commandlist-getcontextflags).
    fn get_context_flags(&self) -> u32;
}

/// The header's `ID3D11ComputeShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11computeshader).
pub trait ComputeShader: DeviceChild {}

/// The header's `ID3D11Counter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11counter).
pub trait Counter: Asynchronous {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11counter-getdesc).
    fn get_desc(&self) -> structures::CounterDesc;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11CryptoSession`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11cryptosession).
pub trait CryptoSession: DeviceChild {
    /// The header's `GetCryptoType`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11cryptosession-getcryptotype).
    fn get_crypto_type(&self) -> component_object_model::structures::Guid;
    /// The header's `GetDecoderProfile`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11cryptosession-getdecoderprofile).
    fn get_decoder_profile(&self) -> component_object_model::structures::Guid;
    /// The header's `GetCertificateSize`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11cryptosession-getcertificatesize).
    fn get_certificate_size(&self) -> Result<u32, HResult>;
    /// The header's `GetCertificate`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11cryptosession-getcertificate).
    fn get_certificate(
        &self,
        certificate: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), HResult>;
    /// The header's `GetCryptoSessionHandle`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11cryptosession-getcryptosessionhandle).
    fn get_crypto_session_handle(&self) -> component_object_model::structures::Handle;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11Debug`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nn-d3d11sdklayers-id3d11debug).
pub trait Debug: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    type Api: Direct3D11;

    /// The header's `SetFeatureMask`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11debug-setfeaturemask).
    fn set_feature_mask(&self, mask: u32) -> Result<(), HResult>;
    /// The header's `GetFeatureMask`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11debug-getfeaturemask).
    fn get_feature_mask(&self) -> u32;
    /// The header's `SetPresentPerRenderOpDelay`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11debug-setpresentperrenderopdelay).
    fn set_present_per_render_op_delay(&self, milliseconds: u32) -> Result<(), HResult>;
    /// The header's `GetPresentPerRenderOpDelay`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11debug-getpresentperrenderopdelay).
    fn get_present_per_render_op_delay(&self) -> u32;
    /// The header's `SetSwapChain`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11debug-setswapchain).
    fn set_swap_chain(
        &self,
        swap_chain: Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>,
    ) -> Result<(), HResult>;
    /// The header's `GetSwapChain`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11debug-getswapchain).
    fn get_swap_chain(&self) -> Result<Option<<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain>, HResult>;
    /// The header's `ValidateContext`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11debug-validatecontext).
    fn validate_context(
        &self,
        context: &<Self::Api as Direct3D11>::DeviceContext,
    ) -> Result<(), HResult>;
    /// The header's `ReportLiveDeviceObjects`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11debug-reportlivedeviceobjects).
    fn report_live_device_objects(&self, flags: enumerations::RldoFlags) -> Result<(), HResult>;
    /// The header's `ValidateContextForDispatch`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11debug-validatecontextfordispatch).
    fn validate_context_for_dispatch(
        &self,
        context: &<Self::Api as Direct3D11>::DeviceContext,
    ) -> Result<(), HResult>;
}

/// The header's `ID3D11DepthStencilState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11depthstencilstate).
pub trait DepthStencilState: DeviceChild {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11depthstencilstate-getdesc).
    fn get_desc(&self) -> structures::DepthStencilDesc;
}

/// The header's `ID3D11DepthStencilView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11depthstencilview).
pub trait DepthStencilView: View {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11depthstencilview-getdesc).
    fn get_desc(&self) -> structures::DepthStencilViewDesc;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11Device`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11device).
pub trait Device: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    type Api: Direct3D11;

    /// The header's `CreateBuffer`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createbuffer).
    fn create_buffer(
        &self,
        desc: &structures::BufferDesc,
        initial_data: Option<&structures::SubresourceData>,
    ) -> Result<<Self::Api as Direct3D11>::Buffer, HResult>;
    /// The header's `CreateTexture1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createtexture1d).
    fn create_texture_1d(
        &self,
        desc: &structures::Texture1DDesc,
        initial_data: &[structures::SubresourceData],
    ) -> Result<<Self::Api as Direct3D11>::Texture1D, HResult>;
    /// The header's `CreateTexture2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createtexture2d).
    fn create_texture_2d(
        &self,
        desc: &structures::Texture2DDesc,
        initial_data: &[structures::SubresourceData],
    ) -> Result<<Self::Api as Direct3D11>::Texture2D, HResult>;
    /// The header's `CreateTexture3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createtexture3d).
    fn create_texture_3d(
        &self,
        desc: &structures::Texture3DDesc,
        initial_data: &[structures::SubresourceData],
    ) -> Result<<Self::Api as Direct3D11>::Texture3D, HResult>;
    /// The header's `CreateShaderResourceView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createshaderresourceview).
    fn create_shader_resource_view(
        &self,
        resource: ResourceRef<'_, Self::Api>,
        desc: Option<&structures::ShaderResourceViewDesc>,
    ) -> Result<<Self::Api as Direct3D11>::ShaderResourceView, HResult>;
    /// The header's `CreateUnorderedAccessView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createunorderedaccessview).
    fn create_unordered_access_view(
        &self,
        resource: ResourceRef<'_, Self::Api>,
        desc: Option<&structures::UnorderedAccessViewDesc>,
    ) -> Result<<Self::Api as Direct3D11>::UnorderedAccessView, HResult>;
    /// The header's `CreateRenderTargetView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createrendertargetview).
    fn create_render_target_view(
        &self,
        resource: ResourceRef<'_, Self::Api>,
        desc: Option<&structures::RenderTargetViewDesc>,
    ) -> Result<<Self::Api as Direct3D11>::RenderTargetView, HResult>;
    /// The header's `CreateDepthStencilView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createdepthstencilview).
    fn create_depth_stencil_view(
        &self,
        resource: ResourceRef<'_, Self::Api>,
        desc: Option<&structures::DepthStencilViewDesc>,
    ) -> Result<<Self::Api as Direct3D11>::DepthStencilView, HResult>;
    /// The header's `CreateInputLayout`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createinputlayout).
    fn create_input_layout(
        &self,
        input_element_descs: &[structures::InputElementDesc],
        shader_bytecode_with_input_signature: &[u8],
    ) -> Result<<Self::Api as Direct3D11>::InputLayout, HResult>;
    /// The header's `CreateVertexShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createvertexshader).
    fn create_vertex_shader(
        &self,
        shader_bytecode: &[u8],
        class_linkage: Option<&<Self::Api as Direct3D11>::ClassLinkage>,
    ) -> Result<<Self::Api as Direct3D11>::VertexShader, HResult>;
    /// The header's `CreateGeometryShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-creategeometryshader).
    fn create_geometry_shader(
        &self,
        shader_bytecode: &[u8],
        class_linkage: Option<&<Self::Api as Direct3D11>::ClassLinkage>,
    ) -> Result<<Self::Api as Direct3D11>::GeometryShader, HResult>;
    /// The header's `CreateGeometryShaderWithStreamOutput`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-creategeometryshaderwithstreamoutput).
    fn create_geometry_shader_with_stream_output(
        &self,
        shader_bytecode: &[u8],
        so_declaration: &[structures::SoDeclarationEntry],
        buffer_strides: &[u32],
        rasterized_stream: u32,
        class_linkage: Option<&<Self::Api as Direct3D11>::ClassLinkage>,
    ) -> Result<<Self::Api as Direct3D11>::GeometryShader, HResult>;
    /// The header's `CreatePixelShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createpixelshader).
    fn create_pixel_shader(
        &self,
        shader_bytecode: &[u8],
        class_linkage: Option<&<Self::Api as Direct3D11>::ClassLinkage>,
    ) -> Result<<Self::Api as Direct3D11>::PixelShader, HResult>;
    /// The header's `CreateHullShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createhullshader).
    fn create_hull_shader(
        &self,
        shader_bytecode: &[u8],
        class_linkage: Option<&<Self::Api as Direct3D11>::ClassLinkage>,
    ) -> Result<<Self::Api as Direct3D11>::HullShader, HResult>;
    /// The header's `CreateDomainShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createdomainshader).
    fn create_domain_shader(
        &self,
        shader_bytecode: &[u8],
        class_linkage: Option<&<Self::Api as Direct3D11>::ClassLinkage>,
    ) -> Result<<Self::Api as Direct3D11>::DomainShader, HResult>;
    /// The header's `CreateComputeShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createcomputeshader).
    fn create_compute_shader(
        &self,
        shader_bytecode: &[u8],
        class_linkage: Option<&<Self::Api as Direct3D11>::ClassLinkage>,
    ) -> Result<<Self::Api as Direct3D11>::ComputeShader, HResult>;
    /// The header's `CreateClassLinkage`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createclasslinkage).
    fn create_class_linkage(&self) -> Result<<Self::Api as Direct3D11>::ClassLinkage, HResult>;
    /// The header's `CreateBlendState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createblendstate).
    fn create_blend_state(
        &self,
        blend_state_desc: &structures::BlendDesc,
    ) -> Result<<Self::Api as Direct3D11>::BlendState, HResult>;
    /// The header's `CreateDepthStencilState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createdepthstencilstate).
    fn create_depth_stencil_state(
        &self,
        depth_stencil_desc: &structures::DepthStencilDesc,
    ) -> Result<<Self::Api as Direct3D11>::DepthStencilState, HResult>;
    /// The header's `CreateRasterizerState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createrasterizerstate).
    fn create_rasterizer_state(
        &self,
        rasterizer_desc: &structures::RasterizerDesc,
    ) -> Result<<Self::Api as Direct3D11>::RasterizerState, HResult>;
    /// The header's `CreateSamplerState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createsamplerstate).
    fn create_sampler_state(
        &self,
        sampler_desc: &structures::SamplerDesc,
    ) -> Result<<Self::Api as Direct3D11>::SamplerState, HResult>;
    /// The header's `CreateQuery`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createquery).
    fn create_query(
        &self,
        query_desc: &structures::QueryDesc,
    ) -> Result<QueryObject<Self::Api>, HResult>;
    /// The header's `CreatePredicate`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createpredicate).
    fn create_predicate(
        &self,
        predicate_desc: &structures::QueryDesc,
    ) -> Result<<Self::Api as Direct3D11>::Predicate, HResult>;
    /// The header's `CreateCounter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createcounter).
    fn create_counter(
        &self,
        counter_desc: &structures::CounterDesc,
    ) -> Result<<Self::Api as Direct3D11>::Counter, HResult>;
    /// The header's `CreateDeferredContext`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-createdeferredcontext).
    fn create_deferred_context(
        &self,
        context_flags: u32,
    ) -> Result<<Self::Api as Direct3D11>::DeviceContext, HResult>;
    /// The header's `OpenSharedResource`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-opensharedresource).
    fn open_shared_resource(
        &self,
        h_resource: component_object_model::structures::Handle,
        returned_interface: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
    /// The header's `CheckFormatSupport`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-checkformatsupport).
    fn check_format_support(
        &self,
        format: directx_graphics_infrastructure::enumerations::Format,
    ) -> Result<u32, HResult>;
    /// The header's `CheckMultisampleQualityLevels`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-checkmultisamplequalitylevels).
    fn check_multisample_quality_levels(
        &self,
        format: directx_graphics_infrastructure::enumerations::Format,
        sample_count: u32,
    ) -> Result<u32, HResult>;
    /// The header's `CheckCounterInfo`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-checkcounterinfo).
    fn check_counter_info(&self) -> structures::CounterInfo;
    /// The header's `CheckCounter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-checkcounter).
    fn check_counter(
        &self,
        desc: &structures::CounterDesc,
        sz_name: Option<&std::ffi::CStr>,
        sz_units: Option<&std::ffi::CStr>,
        sz_description: Option<&std::ffi::CStr>,
    ) -> Result<(enumerations::CounterType, u32), HResult>;
    /// The header's `CheckFeatureSupport`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-checkfeaturesupport).
    fn check_feature_support(
        &self,
        feature: enumerations::Feature,
        feature_support_data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), HResult>;
    /// The header's `GetPrivateData`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-getprivatedata).
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, HResult>;
    /// The header's `SetPrivateData`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-setprivatedata).
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), HResult>;
    /// The header's `SetPrivateDataInterface`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-setprivatedatainterface).
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<UnknownRef<'_, Self::Api>>,
    ) -> Result<(), HResult>;
    /// The header's `GetFeatureLevel`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-getfeaturelevel).
    fn get_feature_level(&self) -> direct3d_common::enumerations::FeatureLevel;
    /// The header's `GetCreationFlags`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-getcreationflags).
    fn get_creation_flags(&self) -> u32;
    /// The header's `GetDeviceRemovedReason`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-getdeviceremovedreason).
    fn get_device_removed_reason(&self) -> Result<(), HResult>;
    /// The header's `GetImmediateContext`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-getimmediatecontext).
    fn get_immediate_context(&self) -> Option<<Self::Api as Direct3D11>::DeviceContext>;
    /// The header's `SetExceptionMode`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-setexceptionmode).
    fn set_exception_mode(&self, raise_flags: u32) -> Result<(), HResult>;
    /// The header's `GetExceptionMode`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11device-getexceptionmode).
    fn get_exception_mode(&self) -> u32;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11Device1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3d11device1).
pub trait Device1: Device {
    /// The header's `GetImmediateContext1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11device1-getimmediatecontext1).
    fn get_immediate_context1(&self) -> Option<<Self::Api as Direct3D11>::DeviceContext>;
    /// The header's `CreateDeferredContext1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11device1-createdeferredcontext1).
    fn create_deferred_context1(
        &self,
        context_flags: u32,
    ) -> Result<<Self::Api as Direct3D11>::DeviceContext, HResult>;
    /// The header's `CreateBlendState1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11device1-createblendstate1).
    fn create_blend_state1(
        &self,
        blend_state_desc: &structures::BlendDesc1,
    ) -> Result<<Self::Api as Direct3D11>::BlendState, HResult>;
    /// The header's `CreateRasterizerState1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11device1-createrasterizerstate1).
    fn create_rasterizer_state1(
        &self,
        rasterizer_desc: &structures::RasterizerDesc1,
    ) -> Result<<Self::Api as Direct3D11>::RasterizerState, HResult>;
    /// The header's `CreateDeviceContextState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11device1-createdevicecontextstate).
    fn create_device_context_state(
        &self,
        flags: u32,
        feature_levels: &[direct3d_common::enumerations::FeatureLevel],
        sdk_version: u32,
        emulated_interface: &component_object_model::structures::Guid,
    ) -> Result<
        (
            direct3d_common::enumerations::FeatureLevel,
            Option<<Self::Api as Direct3D11>::DeviceContextState>,
        ),
        HResult,
    >;
    /// The header's `OpenSharedResource1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11device1-opensharedresource1).
    fn open_shared_resource1(
        &self,
        h_resource: component_object_model::structures::Handle,
        returned_interface: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
    /// The header's `OpenSharedResourceByName`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11device1-opensharedresourcebyname).
    fn open_shared_resource_by_name(
        &self,
        lp_name: &[u16],
        dw_desired_access: u32,
        returned_interface: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11Device2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nn-d3d11_2-id3d11device2).
pub trait Device2: Device1 {
    /// The header's `GetImmediateContext2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nf-d3d11_2-id3d11device2-getimmediatecontext2).
    fn get_immediate_context2(&self) -> Option<<Self::Api as Direct3D11>::DeviceContext>;
    /// The header's `CreateDeferredContext2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nf-d3d11_2-id3d11device2-createdeferredcontext2).
    fn create_deferred_context2(
        &self,
        context_flags: u32,
    ) -> Result<<Self::Api as Direct3D11>::DeviceContext, HResult>;
    /// The header's `GetResourceTiling`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nf-d3d11_2-id3d11device2-getresourcetiling).
    fn get_resource_tiling(
        &self,
        tiled_resource: ResourceRef<'_, Self::Api>,
        first_subresource_tiling_to_get: u32,
    ) -> (
        u32,
        structures::PackedMipDesc,
        structures::TileShape,
        Vec<structures::SubresourceTiling>,
    );
    /// The header's `CheckMultisampleQualityLevels1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nf-d3d11_2-id3d11device2-checkmultisamplequalitylevels1).
    fn check_multisample_quality_levels1(
        &self,
        format: directx_graphics_infrastructure::enumerations::Format,
        sample_count: u32,
        flags: u32,
    ) -> Result<u32, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11Device3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11device3).
pub trait Device3: Device2 {
    /// The header's `CreateTexture2D1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11device3-createtexture2d1).
    fn create_texture_2d1(
        &self,
        desc1: &structures::Texture2DDesc1,
        initial_data: Option<&structures::SubresourceData>,
    ) -> Result<<Self::Api as Direct3D11>::Texture2D, HResult>;
    /// The header's `CreateTexture3D1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11device3-createtexture3d1).
    fn create_texture_3d1(
        &self,
        desc1: &structures::Texture3DDesc1,
        initial_data: Option<&structures::SubresourceData>,
    ) -> Result<<Self::Api as Direct3D11>::Texture3D, HResult>;
    /// The header's `CreateRasterizerState2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11device3-createrasterizerstate2).
    fn create_rasterizer_state2(
        &self,
        rasterizer_desc: &structures::RasterizerDesc2,
    ) -> Result<<Self::Api as Direct3D11>::RasterizerState, HResult>;
    /// The header's `CreateShaderResourceView1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11device3-createshaderresourceview1).
    fn create_shader_resource_view1(
        &self,
        resource: ResourceRef<'_, Self::Api>,
        desc1: Option<&structures::ShaderResourceViewDesc1>,
    ) -> Result<<Self::Api as Direct3D11>::ShaderResourceView, HResult>;
    /// The header's `CreateUnorderedAccessView1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11device3-createunorderedaccessview1).
    fn create_unordered_access_view1(
        &self,
        resource: ResourceRef<'_, Self::Api>,
        desc1: Option<&structures::UnorderedAccessViewDesc1>,
    ) -> Result<<Self::Api as Direct3D11>::UnorderedAccessView, HResult>;
    /// The header's `CreateRenderTargetView1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11device3-createrendertargetview1).
    fn create_render_target_view1(
        &self,
        resource: ResourceRef<'_, Self::Api>,
        desc1: Option<&structures::RenderTargetViewDesc1>,
    ) -> Result<<Self::Api as Direct3D11>::RenderTargetView, HResult>;
    /// The header's `CreateQuery1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11device3-createquery1).
    fn create_query1(
        &self,
        query_desc1: &structures::QueryDesc1,
    ) -> Result<<Self::Api as Direct3D11>::Query, HResult>;
    /// The header's `GetImmediateContext3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11device3-getimmediatecontext3).
    fn get_immediate_context3(&self) -> Option<<Self::Api as Direct3D11>::DeviceContext>;
    /// The header's `CreateDeferredContext3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11device3-createdeferredcontext3).
    fn create_deferred_context3(
        &self,
        context_flags: u32,
    ) -> Result<<Self::Api as Direct3D11>::DeviceContext, HResult>;
    /// The header's `WriteToSubresource`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11device3-writetosubresource).
    fn write_to_subresource(
        &self,
        dst_resource: ResourceRef<'_, Self::Api>,
        dst_subresource: u32,
        dst_box: Option<&structures::Box>,
        src_data: &impl wcontainers_volatile_bytes::VolatileBytes,
        src_row_pitch: u32,
        src_depth_pitch: u32,
    );
    /// The header's `ReadFromSubresource`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11device3-readfromsubresource).
    fn read_from_subresource(
        &self,
        dst_data: &impl wcontainers_volatile_bytes::VolatileBytes,
        dst_row_pitch: u32,
        dst_depth_pitch: u32,
        src_resource: ResourceRef<'_, Self::Api>,
        src_subresource: u32,
        src_box: Option<&structures::Box>,
    );
}

/// The header's `ID3D11Device4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11device4).
pub trait Device4: Device3 {
    /// The header's `RegisterDeviceRemovedEvent`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nf-d3d11_4-id3d11device4-registerdeviceremovedevent).
    fn register_device_removed_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, HResult>;
    /// The header's `UnregisterDeviceRemoved`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nf-d3d11_4-id3d11device4-unregisterdeviceremoved).
    fn unregister_device_removed(&self, dw_cookie: u32);
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11Device5`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11device5).
pub trait Device5: Device4 {
    /// The header's `OpenSharedFence`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nf-d3d11_4-id3d11device5-opensharedfence).
    fn open_shared_fence(
        &self,
        h_fence: component_object_model::structures::Handle,
        returned_interface: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
    /// The header's `CreateFence`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nf-d3d11_4-id3d11device5-createfence).
    fn create_fence(
        &self,
        initial_value: u64,
        flags: enumerations::FenceFlag,
        returned_interface: &component_object_model::structures::Guid,
    ) -> Result<UnknownObject<Self::Api>, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11DeviceChild`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11devicechild).
pub trait DeviceChild: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    type Api: Direct3D11;

    /// The header's `GetDevice`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicechild-getdevice).
    fn get_device(&self) -> Option<<Self::Api as Direct3D11>::Device>;
    /// The header's `GetPrivateData`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicechild-getprivatedata).
    fn get_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, HResult>;
    /// The header's `SetPrivateData`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicechild-setprivatedata).
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), HResult>;
    /// The header's `SetPrivateDataInterface`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicechild-setprivatedatainterface).
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<UnknownRef<'_, Self::Api>>,
    ) -> Result<(), HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11DeviceContext`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11devicecontext).
pub trait DeviceContext: DeviceChild {
    /// The header's `VSSetConstantBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-vssetconstantbuffers).
    fn vs_set_constant_buffers(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as Direct3D11>::Buffer>],
    );
    /// The header's `PSSetShaderResources`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-pssetshaderresources).
    fn ps_set_shader_resources(
        &self,
        start_slot: u32,
        shader_resource_views: &[Option<&<Self::Api as Direct3D11>::ShaderResourceView>],
    );
    /// The header's `PSSetShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-pssetshader).
    fn ps_set_shader(
        &self,
        pixel_shader: Option<&<Self::Api as Direct3D11>::PixelShader>,
        class_instances: &[Option<&<Self::Api as Direct3D11>::ClassInstance>],
    );
    /// The header's `PSSetSamplers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-pssetsamplers).
    fn ps_set_samplers(
        &self,
        start_slot: u32,
        samplers: &[Option<&<Self::Api as Direct3D11>::SamplerState>],
    );
    /// The header's `VSSetShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-vssetshader).
    fn vs_set_shader(
        &self,
        vertex_shader: Option<&<Self::Api as Direct3D11>::VertexShader>,
        class_instances: &[Option<&<Self::Api as Direct3D11>::ClassInstance>],
    );
    /// The header's `DrawIndexed`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-drawindexed).
    fn draw_indexed(&self, index_count: u32, start_index_location: u32, base_vertex_location: i32);
    /// The header's `Draw`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-draw).
    fn draw(&self, vertex_count: u32, start_vertex_location: u32);
    /// The header's `Map`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-map).
    fn map(
        &self,
        resource: ResourceRef<'_, Self::Api>,
        subresource: u32,
        map_type: enumerations::Map,
        map_flags: u32,
    ) -> Result<structures::MappedSubresource<<Self::Api as Direct3D11>::Mapped>, HResult>;
    /// The header's `Unmap`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-unmap).
    fn unmap(&self, resource: ResourceRef<'_, Self::Api>, subresource: u32);
    /// The header's `PSSetConstantBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-pssetconstantbuffers).
    fn ps_set_constant_buffers(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as Direct3D11>::Buffer>],
    );
    /// The header's `IASetInputLayout`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-iasetinputlayout).
    fn ia_set_input_layout(&self, input_layout: Option<&<Self::Api as Direct3D11>::InputLayout>);
    /// The header's `IASetVertexBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-iasetvertexbuffers).
    fn ia_set_vertex_buffers(
        &self,
        start_slot: u32,
        vertex_buffers: &[Option<&<Self::Api as Direct3D11>::Buffer>],
        strides: &[u32],
        offsets: &[u32],
    );
    /// The header's `IASetIndexBuffer`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-iasetindexbuffer).
    fn ia_set_index_buffer(
        &self,
        index_buffer: Option<&<Self::Api as Direct3D11>::Buffer>,
        format: directx_graphics_infrastructure::enumerations::Format,
        offset: u32,
    );
    /// The header's `DrawIndexedInstanced`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-drawindexedinstanced).
    fn draw_indexed_instanced(
        &self,
        index_count_per_instance: u32,
        instance_count: u32,
        start_index_location: u32,
        base_vertex_location: i32,
        start_instance_location: u32,
    );
    /// The header's `DrawInstanced`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-drawinstanced).
    fn draw_instanced(
        &self,
        vertex_count_per_instance: u32,
        instance_count: u32,
        start_vertex_location: u32,
        start_instance_location: u32,
    );
    /// The header's `GSSetConstantBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-gssetconstantbuffers).
    fn gs_set_constant_buffers(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as Direct3D11>::Buffer>],
    );
    /// The header's `GSSetShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-gssetshader).
    fn gs_set_shader(
        &self,
        shader: Option<&<Self::Api as Direct3D11>::GeometryShader>,
        class_instances: &[Option<&<Self::Api as Direct3D11>::ClassInstance>],
    );
    /// The header's `IASetPrimitiveTopology`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-iasetprimitivetopology).
    fn ia_set_primitive_topology(&self, topology: direct3d_common::enumerations::PrimitiveTopology);
    /// The header's `VSSetShaderResources`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-vssetshaderresources).
    fn vs_set_shader_resources(
        &self,
        start_slot: u32,
        shader_resource_views: &[Option<&<Self::Api as Direct3D11>::ShaderResourceView>],
    );
    /// The header's `VSSetSamplers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-vssetsamplers).
    fn vs_set_samplers(
        &self,
        start_slot: u32,
        samplers: &[Option<&<Self::Api as Direct3D11>::SamplerState>],
    );
    /// The header's `Begin`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-begin).
    fn begin(&self, async_: AsynchronousRef<'_, Self::Api>);
    /// The header's `End`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-end).
    fn end(&self, async_: AsynchronousRef<'_, Self::Api>);
    /// The header's `GetData`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-getdata).
    fn get_data(
        &self,
        async_: AsynchronousRef<'_, Self::Api>,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
        get_data_flags: u32,
    ) -> Result<(), HResult>;
    /// The header's `SetPredication`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-setpredication).
    fn set_predication(
        &self,
        predicate: Option<&<Self::Api as Direct3D11>::Predicate>,
        predicate_value: bool,
    );
    /// The header's `GSSetShaderResources`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-gssetshaderresources).
    fn gs_set_shader_resources(
        &self,
        start_slot: u32,
        shader_resource_views: &[Option<&<Self::Api as Direct3D11>::ShaderResourceView>],
    );
    /// The header's `GSSetSamplers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-gssetsamplers).
    fn gs_set_samplers(
        &self,
        start_slot: u32,
        samplers: &[Option<&<Self::Api as Direct3D11>::SamplerState>],
    );
    /// The header's `OMSetRenderTargets`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-omsetrendertargets).
    fn om_set_render_targets(
        &self,
        render_target_views: &[Option<&<Self::Api as Direct3D11>::RenderTargetView>],
        depth_stencil_view: Option<&<Self::Api as Direct3D11>::DepthStencilView>,
    );
    /// The header's `OMSetRenderTargetsAndUnorderedAccessViews`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-omsetrendertargetsandunorderedaccessviews).
    fn om_set_render_targets_and_unordered_access_views(
        &self,
        render_target_views: Option<&[Option<&<Self::Api as Direct3D11>::RenderTargetView>]>,
        depth_stencil_view: Option<&<Self::Api as Direct3D11>::DepthStencilView>,
        uav_start_slot: u32,
        unordered_access_views: Option<&[Option<&<Self::Api as Direct3D11>::UnorderedAccessView>]>,
        uav_initial_counts: Option<&[u32]>,
    );
    /// The header's `OMSetBlendState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-omsetblendstate).
    fn om_set_blend_state(
        &self,
        blend_state: Option<&<Self::Api as Direct3D11>::BlendState>,
        blend_factor: Option<
            &[f32; constants::DEVICE_CONTEXT_OM_SET_BLEND_STATE_BLEND_FACTOR_LENGTH],
        >,
        sample_mask: u32,
    );
    /// The header's `OMSetDepthStencilState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-omsetdepthstencilstate).
    fn om_set_depth_stencil_state(
        &self,
        depth_stencil_state: Option<&<Self::Api as Direct3D11>::DepthStencilState>,
        stencil_ref: u32,
    );
    /// The header's `SOSetTargets`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-sosettargets).
    fn so_set_targets(
        &self,
        so_targets: &[Option<&<Self::Api as Direct3D11>::Buffer>],
        offsets: &[u32],
    );
    /// The header's `DrawAuto`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-drawauto).
    fn draw_auto(&self);
    /// The header's `DrawIndexedInstancedIndirect`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-drawindexedinstancedindirect).
    fn draw_indexed_instanced_indirect(
        &self,
        buffer_for_args: &<Self::Api as Direct3D11>::Buffer,
        aligned_byte_offset_for_args: u32,
    );
    /// The header's `DrawInstancedIndirect`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-drawinstancedindirect).
    fn draw_instanced_indirect(
        &self,
        buffer_for_args: &<Self::Api as Direct3D11>::Buffer,
        aligned_byte_offset_for_args: u32,
    );
    /// The header's `Dispatch`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-dispatch).
    fn dispatch(
        &self,
        thread_group_count_x: u32,
        thread_group_count_y: u32,
        thread_group_count_z: u32,
    );
    /// The header's `DispatchIndirect`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-dispatchindirect).
    fn dispatch_indirect(
        &self,
        buffer_for_args: &<Self::Api as Direct3D11>::Buffer,
        aligned_byte_offset_for_args: u32,
    );
    /// The header's `RSSetState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-rssetstate).
    fn rs_set_state(&self, rasterizer_state: Option<&<Self::Api as Direct3D11>::RasterizerState>);
    /// The header's `RSSetViewports`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-rssetviewports).
    fn rs_set_viewports(&self, viewports: &[structures::Viewport]);
    /// The header's `RSSetScissorRects`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-rssetscissorrects).
    fn rs_set_scissor_rects(&self, rects: &[component_object_model::structures::Rect]);
    /// The header's `CopySubresourceRegion`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-copysubresourceregion).
    #[allow(
        clippy::too_many_arguments,
        reason = "the method's own parameters, one to one (MAPPING.md)"
    )]
    fn copy_subresource_region(
        &self,
        dst_resource: ResourceRef<'_, Self::Api>,
        dst_subresource: u32,
        dst_x: u32,
        dst_y: u32,
        dst_z: u32,
        src_resource: ResourceRef<'_, Self::Api>,
        src_subresource: u32,
        src_box: Option<&structures::Box>,
    );
    /// The header's `CopyResource`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-copyresource).
    fn copy_resource(
        &self,
        dst_resource: ResourceRef<'_, Self::Api>,
        src_resource: ResourceRef<'_, Self::Api>,
    );
    /// The header's `UpdateSubresource`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-updatesubresource).
    fn update_subresource(
        &self,
        dst_resource: ResourceRef<'_, Self::Api>,
        dst_subresource: u32,
        dst_box: Option<&structures::Box>,
        src_data: &impl wcontainers_volatile_bytes::VolatileBytes,
        src_row_pitch: u32,
        src_depth_pitch: u32,
    );
    /// The header's `CopyStructureCount`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-copystructurecount).
    fn copy_structure_count(
        &self,
        dst_buffer: &<Self::Api as Direct3D11>::Buffer,
        dst_aligned_byte_offset: u32,
        src_view: &<Self::Api as Direct3D11>::UnorderedAccessView,
    );
    /// The header's `ClearRenderTargetView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-clearrendertargetview).
    fn clear_render_target_view(
        &self,
        render_target_view: &<Self::Api as Direct3D11>::RenderTargetView,
        color_rgba: &[f32; constants::DEVICE_CONTEXT_CLEAR_RENDER_TARGET_VIEW_COLOR_RGBA_LENGTH],
    );
    /// The header's `ClearUnorderedAccessViewUint`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-clearunorderedaccessviewuint).
    fn clear_unordered_access_view_uint(
        &self,
        unordered_access_view: &<Self::Api as Direct3D11>::UnorderedAccessView,
        values: &[u32; constants::DEVICE_CONTEXT_CLEAR_UNORDERED_ACCESS_VIEW_UINT_VALUES_LENGTH],
    );
    /// The header's `ClearUnorderedAccessViewFloat`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-clearunorderedaccessviewfloat).
    fn clear_unordered_access_view_float(
        &self,
        unordered_access_view: &<Self::Api as Direct3D11>::UnorderedAccessView,
        values: &[f32; constants::DEVICE_CONTEXT_CLEAR_UNORDERED_ACCESS_VIEW_FLOAT_VALUES_LENGTH],
    );
    /// The header's `ClearDepthStencilView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-cleardepthstencilview).
    fn clear_depth_stencil_view(
        &self,
        depth_stencil_view: &<Self::Api as Direct3D11>::DepthStencilView,
        clear_flags: u32,
        depth: f32,
        stencil: u8,
    );
    /// The header's `GenerateMips`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-generatemips).
    fn generate_mips(&self, shader_resource_view: &<Self::Api as Direct3D11>::ShaderResourceView);
    /// The header's `SetResourceMinLOD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-setresourceminlod).
    fn set_resource_min_lod(&self, resource: ResourceRef<'_, Self::Api>, min_lod: f32);
    /// The header's `GetResourceMinLOD`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-getresourceminlod).
    fn get_resource_min_lod(&self, resource: ResourceRef<'_, Self::Api>) -> f32;
    /// The header's `ResolveSubresource`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-resolvesubresource).
    fn resolve_subresource(
        &self,
        dst_resource: ResourceRef<'_, Self::Api>,
        dst_subresource: u32,
        src_resource: ResourceRef<'_, Self::Api>,
        src_subresource: u32,
        format: directx_graphics_infrastructure::enumerations::Format,
    );
    /// The header's `ExecuteCommandList`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-executecommandlist).
    fn execute_command_list(
        &self,
        command_list: &<Self::Api as Direct3D11>::CommandList,
        restore_context_state: bool,
    );
    /// The header's `HSSetShaderResources`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-hssetshaderresources).
    fn hs_set_shader_resources(
        &self,
        start_slot: u32,
        shader_resource_views: &[Option<&<Self::Api as Direct3D11>::ShaderResourceView>],
    );
    /// The header's `HSSetShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-hssetshader).
    fn hs_set_shader(
        &self,
        hull_shader: Option<&<Self::Api as Direct3D11>::HullShader>,
        class_instances: &[Option<&<Self::Api as Direct3D11>::ClassInstance>],
    );
    /// The header's `HSSetSamplers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-hssetsamplers).
    fn hs_set_samplers(
        &self,
        start_slot: u32,
        samplers: &[Option<&<Self::Api as Direct3D11>::SamplerState>],
    );
    /// The header's `HSSetConstantBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-hssetconstantbuffers).
    fn hs_set_constant_buffers(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as Direct3D11>::Buffer>],
    );
    /// The header's `DSSetShaderResources`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-dssetshaderresources).
    fn ds_set_shader_resources(
        &self,
        start_slot: u32,
        shader_resource_views: &[Option<&<Self::Api as Direct3D11>::ShaderResourceView>],
    );
    /// The header's `DSSetShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-dssetshader).
    fn ds_set_shader(
        &self,
        domain_shader: Option<&<Self::Api as Direct3D11>::DomainShader>,
        class_instances: &[Option<&<Self::Api as Direct3D11>::ClassInstance>],
    );
    /// The header's `DSSetSamplers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-dssetsamplers).
    fn ds_set_samplers(
        &self,
        start_slot: u32,
        samplers: &[Option<&<Self::Api as Direct3D11>::SamplerState>],
    );
    /// The header's `DSSetConstantBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-dssetconstantbuffers).
    fn ds_set_constant_buffers(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as Direct3D11>::Buffer>],
    );
    /// The header's `CSSetShaderResources`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-cssetshaderresources).
    fn cs_set_shader_resources(
        &self,
        start_slot: u32,
        shader_resource_views: &[Option<&<Self::Api as Direct3D11>::ShaderResourceView>],
    );
    /// The header's `CSSetUnorderedAccessViews`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-cssetunorderedaccessviews).
    fn cs_set_unordered_access_views(
        &self,
        start_slot: u32,
        unordered_access_views: &[Option<&<Self::Api as Direct3D11>::UnorderedAccessView>],
        uav_initial_counts: &[u32],
    );
    /// The header's `CSSetShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-cssetshader).
    fn cs_set_shader(
        &self,
        compute_shader: Option<&<Self::Api as Direct3D11>::ComputeShader>,
        class_instances: &[Option<&<Self::Api as Direct3D11>::ClassInstance>],
    );
    /// The header's `CSSetSamplers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-cssetsamplers).
    fn cs_set_samplers(
        &self,
        start_slot: u32,
        samplers: &[Option<&<Self::Api as Direct3D11>::SamplerState>],
    );
    /// The header's `CSSetConstantBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-cssetconstantbuffers).
    fn cs_set_constant_buffers(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as Direct3D11>::Buffer>],
    );
    /// The header's `VSGetConstantBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-vsgetconstantbuffers).
    fn vs_get_constant_buffers(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::Buffer>>;
    /// The header's `PSGetShaderResources`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-psgetshaderresources).
    fn ps_get_shader_resources(
        &self,
        start_slot: u32,
        num_views: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::ShaderResourceView>>;
    /// The header's `PSGetShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-psgetshader).
    fn ps_get_shader(
        &self,
    ) -> (
        Option<<Self::Api as Direct3D11>::PixelShader>,
        Vec<Option<<Self::Api as Direct3D11>::ClassInstance>>,
    );
    /// The header's `PSGetSamplers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-psgetsamplers).
    fn ps_get_samplers(
        &self,
        start_slot: u32,
        num_samplers: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::SamplerState>>;
    /// The header's `VSGetShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-vsgetshader).
    fn vs_get_shader(
        &self,
    ) -> (
        Option<<Self::Api as Direct3D11>::VertexShader>,
        Vec<Option<<Self::Api as Direct3D11>::ClassInstance>>,
    );
    /// The header's `PSGetConstantBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-psgetconstantbuffers).
    fn ps_get_constant_buffers(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::Buffer>>;
    /// The header's `IAGetInputLayout`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-iagetinputlayout).
    fn ia_get_input_layout(&self) -> Option<<Self::Api as Direct3D11>::InputLayout>;
    /// The header's `IAGetVertexBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-iagetvertexbuffers).
    fn ia_get_vertex_buffers(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> (
        Vec<Option<<Self::Api as Direct3D11>::Buffer>>,
        Vec<u32>,
        Vec<u32>,
    );
    /// The header's `IAGetIndexBuffer`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-iagetindexbuffer).
    fn ia_get_index_buffer(
        &self,
    ) -> (
        Option<<Self::Api as Direct3D11>::Buffer>,
        directx_graphics_infrastructure::enumerations::Format,
        u32,
    );
    /// The header's `GSGetConstantBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-gsgetconstantbuffers).
    fn gs_get_constant_buffers(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::Buffer>>;
    /// The header's `GSGetShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-gsgetshader).
    fn gs_get_shader(
        &self,
    ) -> (
        Option<<Self::Api as Direct3D11>::GeometryShader>,
        Vec<Option<<Self::Api as Direct3D11>::ClassInstance>>,
    );
    /// The header's `IAGetPrimitiveTopology`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-iagetprimitivetopology).
    fn ia_get_primitive_topology(&self) -> direct3d_common::enumerations::PrimitiveTopology;
    /// The header's `VSGetShaderResources`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-vsgetshaderresources).
    fn vs_get_shader_resources(
        &self,
        start_slot: u32,
        num_views: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::ShaderResourceView>>;
    /// The header's `VSGetSamplers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-vsgetsamplers).
    fn vs_get_samplers(
        &self,
        start_slot: u32,
        num_samplers: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::SamplerState>>;
    /// The header's `GetPredication`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-getpredication).
    fn get_predication(&self) -> (Option<<Self::Api as Direct3D11>::Predicate>, bool);
    /// The header's `GSGetShaderResources`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-gsgetshaderresources).
    fn gs_get_shader_resources(
        &self,
        start_slot: u32,
        num_views: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::ShaderResourceView>>;
    /// The header's `GSGetSamplers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-gsgetsamplers).
    fn gs_get_samplers(
        &self,
        start_slot: u32,
        num_samplers: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::SamplerState>>;
    /// The header's `OMGetRenderTargets`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-omgetrendertargets).
    fn om_get_render_targets(
        &self,
        num_views: u32,
    ) -> (
        Vec<Option<<Self::Api as Direct3D11>::RenderTargetView>>,
        Option<<Self::Api as Direct3D11>::DepthStencilView>,
    );
    /// The header's `OMGetRenderTargetsAndUnorderedAccessViews`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-omgetrendertargetsandunorderedaccessviews).
    fn om_get_render_targets_and_unordered_access_views(
        &self,
        num_rt_vs: u32,
        uav_start_slot: u32,
        num_ua_vs: u32,
    ) -> (
        Vec<Option<<Self::Api as Direct3D11>::RenderTargetView>>,
        Option<<Self::Api as Direct3D11>::DepthStencilView>,
        Vec<Option<<Self::Api as Direct3D11>::UnorderedAccessView>>,
    );
    /// The header's `OMGetBlendState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-omgetblendstate).
    fn om_get_blend_state(
        &self,
    ) -> (
        Option<<Self::Api as Direct3D11>::BlendState>,
        [f32; constants::DEVICE_CONTEXT_OM_GET_BLEND_STATE_BLEND_FACTOR_LENGTH],
        u32,
    );
    /// The header's `OMGetDepthStencilState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-omgetdepthstencilstate).
    fn om_get_depth_stencil_state(
        &self,
    ) -> (Option<<Self::Api as Direct3D11>::DepthStencilState>, u32);
    /// The header's `SOGetTargets`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-sogettargets).
    fn so_get_targets(&self, num_buffers: u32) -> Vec<Option<<Self::Api as Direct3D11>::Buffer>>;
    /// The header's `RSGetState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-rsgetstate).
    fn rs_get_state(&self) -> Option<<Self::Api as Direct3D11>::RasterizerState>;
    /// The header's `RSGetViewports`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-rsgetviewports).
    fn rs_get_viewports(&self) -> Vec<structures::Viewport>;
    /// The header's `RSGetScissorRects`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-rsgetscissorrects).
    fn rs_get_scissor_rects(&self) -> Vec<component_object_model::structures::Rect>;
    /// The header's `HSGetShaderResources`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-hsgetshaderresources).
    fn hs_get_shader_resources(
        &self,
        start_slot: u32,
        num_views: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::ShaderResourceView>>;
    /// The header's `HSGetShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-hsgetshader).
    fn hs_get_shader(
        &self,
    ) -> (
        Option<<Self::Api as Direct3D11>::HullShader>,
        Vec<Option<<Self::Api as Direct3D11>::ClassInstance>>,
    );
    /// The header's `HSGetSamplers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-hsgetsamplers).
    fn hs_get_samplers(
        &self,
        start_slot: u32,
        num_samplers: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::SamplerState>>;
    /// The header's `HSGetConstantBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-hsgetconstantbuffers).
    fn hs_get_constant_buffers(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::Buffer>>;
    /// The header's `DSGetShaderResources`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-dsgetshaderresources).
    fn ds_get_shader_resources(
        &self,
        start_slot: u32,
        num_views: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::ShaderResourceView>>;
    /// The header's `DSGetShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-dsgetshader).
    fn ds_get_shader(
        &self,
    ) -> (
        Option<<Self::Api as Direct3D11>::DomainShader>,
        Vec<Option<<Self::Api as Direct3D11>::ClassInstance>>,
    );
    /// The header's `DSGetSamplers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-dsgetsamplers).
    fn ds_get_samplers(
        &self,
        start_slot: u32,
        num_samplers: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::SamplerState>>;
    /// The header's `DSGetConstantBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-dsgetconstantbuffers).
    fn ds_get_constant_buffers(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::Buffer>>;
    /// The header's `CSGetShaderResources`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-csgetshaderresources).
    fn cs_get_shader_resources(
        &self,
        start_slot: u32,
        num_views: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::ShaderResourceView>>;
    /// The header's `CSGetUnorderedAccessViews`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-csgetunorderedaccessviews).
    fn cs_get_unordered_access_views(
        &self,
        start_slot: u32,
        num_ua_vs: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::UnorderedAccessView>>;
    /// The header's `CSGetShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-csgetshader).
    fn cs_get_shader(
        &self,
    ) -> (
        Option<<Self::Api as Direct3D11>::ComputeShader>,
        Vec<Option<<Self::Api as Direct3D11>::ClassInstance>>,
    );
    /// The header's `CSGetSamplers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-csgetsamplers).
    fn cs_get_samplers(
        &self,
        start_slot: u32,
        num_samplers: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::SamplerState>>;
    /// The header's `CSGetConstantBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-csgetconstantbuffers).
    fn cs_get_constant_buffers(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> Vec<Option<<Self::Api as Direct3D11>::Buffer>>;
    /// The header's `ClearState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-clearstate).
    fn clear_state(&self);
    /// The header's `Flush`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-flush).
    fn flush(&self);
    /// The header's `GetType`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-gettype).
    fn get_type(&self) -> enumerations::DeviceContextType;
    /// The header's `GetContextFlags`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-getcontextflags).
    fn get_context_flags(&self) -> u32;
    /// The header's `FinishCommandList`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-finishcommandlist).
    fn finish_command_list(
        &self,
        restore_deferred_context_state: bool,
    ) -> Result<<Self::Api as Direct3D11>::CommandList, HResult>;
}

/// The header's `ID3D11DeviceContext1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3d11devicecontext1).
pub trait DeviceContext1: DeviceContext {
    /// The header's `CopySubresourceRegion1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-copysubresourceregion1).
    #[allow(
        clippy::too_many_arguments,
        reason = "the method's own parameters, one to one (MAPPING.md)"
    )]
    fn copy_subresource_region1(
        &self,
        dst_resource: ResourceRef<'_, Self::Api>,
        dst_subresource: u32,
        dst_x: u32,
        dst_y: u32,
        dst_z: u32,
        src_resource: ResourceRef<'_, Self::Api>,
        src_subresource: u32,
        src_box: Option<&structures::Box>,
        copy_flags: u32,
    );
    /// The header's `UpdateSubresource1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-updatesubresource1).
    #[allow(
        clippy::too_many_arguments,
        reason = "the method's own parameters, one to one (MAPPING.md)"
    )]
    fn update_subresource1(
        &self,
        dst_resource: ResourceRef<'_, Self::Api>,
        dst_subresource: u32,
        dst_box: Option<&structures::Box>,
        src_data: &impl wcontainers_volatile_bytes::VolatileBytes,
        src_row_pitch: u32,
        src_depth_pitch: u32,
        copy_flags: u32,
    );
    /// The header's `DiscardResource`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-discardresource).
    fn discard_resource(&self, resource: ResourceRef<'_, Self::Api>);
    /// The header's `DiscardView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-discardview).
    fn discard_view(&self, resource_view: ViewRef<'_, Self::Api>);
    /// The header's `VSSetConstantBuffers1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-vssetconstantbuffers1).
    fn vs_set_constant_buffers1(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as Direct3D11>::Buffer>],
        first_constant: &[u32],
        num_constants: &[u32],
    );
    /// The header's `HSSetConstantBuffers1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-hssetconstantbuffers1).
    fn hs_set_constant_buffers1(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as Direct3D11>::Buffer>],
        first_constant: &[u32],
        num_constants: &[u32],
    );
    /// The header's `DSSetConstantBuffers1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-dssetconstantbuffers1).
    fn ds_set_constant_buffers1(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as Direct3D11>::Buffer>],
        first_constant: &[u32],
        num_constants: &[u32],
    );
    /// The header's `GSSetConstantBuffers1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-gssetconstantbuffers1).
    fn gs_set_constant_buffers1(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as Direct3D11>::Buffer>],
        first_constant: &[u32],
        num_constants: &[u32],
    );
    /// The header's `PSSetConstantBuffers1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-pssetconstantbuffers1).
    fn ps_set_constant_buffers1(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as Direct3D11>::Buffer>],
        first_constant: &[u32],
        num_constants: &[u32],
    );
    /// The header's `CSSetConstantBuffers1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-cssetconstantbuffers1).
    fn cs_set_constant_buffers1(
        &self,
        start_slot: u32,
        constant_buffers: &[Option<&<Self::Api as Direct3D11>::Buffer>],
        first_constant: &[u32],
        num_constants: &[u32],
    );
    /// The header's `VSGetConstantBuffers1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-vsgetconstantbuffers1).
    fn vs_get_constant_buffers1(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> (
        Vec<Option<<Self::Api as Direct3D11>::Buffer>>,
        Vec<u32>,
        Vec<u32>,
    );
    /// The header's `HSGetConstantBuffers1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-hsgetconstantbuffers1).
    fn hs_get_constant_buffers1(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> (
        Vec<Option<<Self::Api as Direct3D11>::Buffer>>,
        Vec<u32>,
        Vec<u32>,
    );
    /// The header's `DSGetConstantBuffers1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-dsgetconstantbuffers1).
    fn ds_get_constant_buffers1(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> (
        Vec<Option<<Self::Api as Direct3D11>::Buffer>>,
        Vec<u32>,
        Vec<u32>,
    );
    /// The header's `GSGetConstantBuffers1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-gsgetconstantbuffers1).
    fn gs_get_constant_buffers1(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> (
        Vec<Option<<Self::Api as Direct3D11>::Buffer>>,
        Vec<u32>,
        Vec<u32>,
    );
    /// The header's `PSGetConstantBuffers1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-psgetconstantbuffers1).
    fn ps_get_constant_buffers1(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> (
        Vec<Option<<Self::Api as Direct3D11>::Buffer>>,
        Vec<u32>,
        Vec<u32>,
    );
    /// The header's `CSGetConstantBuffers1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-csgetconstantbuffers1).
    fn cs_get_constant_buffers1(
        &self,
        start_slot: u32,
        num_buffers: u32,
    ) -> (
        Vec<Option<<Self::Api as Direct3D11>::Buffer>>,
        Vec<u32>,
        Vec<u32>,
    );
    /// The header's `SwapDeviceContextState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-swapdevicecontextstate).
    fn swap_device_context_state(
        &self,
        state: &<Self::Api as Direct3D11>::DeviceContextState,
    ) -> Option<<Self::Api as Direct3D11>::DeviceContextState>;
    /// The header's `ClearView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-clearview).
    fn clear_view(
        &self,
        view: ViewRef<'_, Self::Api>,
        color: &[f32; constants::DEVICE_CONTEXT1_CLEAR_VIEW_COLOR_LENGTH],
        rect: &[component_object_model::structures::Rect],
    );
    /// The header's `DiscardView1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11devicecontext1-discardview1).
    fn discard_view1(
        &self,
        resource_view: ViewRef<'_, Self::Api>,
        rects: &[component_object_model::structures::Rect],
    );
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11DeviceContext2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nn-d3d11_2-id3d11devicecontext2).
pub trait DeviceContext2: DeviceContext1 {
    /// The header's `UpdateTileMappings`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nf-d3d11_2-id3d11devicecontext2-updatetilemappings).
    #[allow(
        clippy::too_many_arguments,
        reason = "the method's own parameters, one to one (MAPPING.md)"
    )]
    fn update_tile_mappings(
        &self,
        tiled_resource: ResourceRef<'_, Self::Api>,
        tiled_resource_region_start_coordinates: &[structures::TiledResourceCoordinate],
        tiled_resource_region_sizes: &[structures::TileRegionSize],
        tile_pool: Option<&<Self::Api as Direct3D11>::Buffer>,
        range_flags: &[u32],
        tile_pool_start_offsets: &[u32],
        range_tile_counts: &[u32],
        flags: u32,
    ) -> Result<(), HResult>;
    /// The header's `CopyTileMappings`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nf-d3d11_2-id3d11devicecontext2-copytilemappings).
    fn copy_tile_mappings(
        &self,
        dest_tiled_resource: ResourceRef<'_, Self::Api>,
        dest_region_start_coordinate: &structures::TiledResourceCoordinate,
        source_tiled_resource: ResourceRef<'_, Self::Api>,
        source_region_start_coordinate: &structures::TiledResourceCoordinate,
        tile_region_size: &structures::TileRegionSize,
        flags: u32,
    ) -> Result<(), HResult>;
    /// The header's `CopyTiles`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nf-d3d11_2-id3d11devicecontext2-copytiles).
    fn copy_tiles(
        &self,
        tiled_resource: ResourceRef<'_, Self::Api>,
        tile_region_start_coordinate: &structures::TiledResourceCoordinate,
        tile_region_size: &structures::TileRegionSize,
        buffer: &<Self::Api as Direct3D11>::Buffer,
        buffer_start_offset_in_bytes: u64,
        flags: u32,
    );
    /// The header's `UpdateTiles`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nf-d3d11_2-id3d11devicecontext2-updatetiles).
    fn update_tiles(
        &self,
        dest_tiled_resource: ResourceRef<'_, Self::Api>,
        dest_tile_region_start_coordinate: &structures::TiledResourceCoordinate,
        dest_tile_region_size: &structures::TileRegionSize,
        source_tile_data: &impl wcontainers_volatile_bytes::VolatileBytes,
        flags: u32,
    );
    /// The header's `ResizeTilePool`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nf-d3d11_2-id3d11devicecontext2-resizetilepool).
    fn resize_tile_pool(
        &self,
        tile_pool: &<Self::Api as Direct3D11>::Buffer,
        new_size_in_bytes: u64,
    ) -> Result<(), HResult>;
    /// The header's `TiledResourceBarrier`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nf-d3d11_2-id3d11devicecontext2-tiledresourcebarrier).
    fn tiled_resource_barrier(
        &self,
        tiled_resource_or_view_access_before_barrier: Option<DeviceChildRef<'_, Self::Api>>,
        tiled_resource_or_view_access_after_barrier: Option<DeviceChildRef<'_, Self::Api>>,
    );
    /// The header's `IsAnnotationEnabled`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nf-d3d11_2-id3d11devicecontext2-isannotationenabled).
    fn is_annotation_enabled(&self) -> bool;
    /// The header's `SetMarkerInt`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nf-d3d11_2-id3d11devicecontext2-setmarkerint).
    fn set_marker_int(&self, label: &[u16], data: i32);
    /// The header's `BeginEventInt`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nf-d3d11_2-id3d11devicecontext2-begineventint).
    fn begin_event_int(&self, label: &[u16], data: i32);
    /// The header's `EndEvent`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_2/nf-d3d11_2-id3d11devicecontext2-endevent).
    fn end_event(&self);
}

/// The header's `ID3D11DeviceContext3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11devicecontext3).
pub trait DeviceContext3: DeviceContext2 {
    /// The header's `Flush1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11devicecontext3-flush1).
    fn flush1(
        &self,
        context_type: enumerations::ContextType,
        h_event: component_object_model::structures::Handle,
    );
    /// The header's `SetHardwareProtectionState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11devicecontext3-sethardwareprotectionstate).
    fn set_hardware_protection_state(&self, hw_protection_enable: bool);
    /// The header's `GetHardwareProtectionState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11devicecontext3-gethardwareprotectionstate).
    fn get_hardware_protection_state(&self) -> bool;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11DeviceContext4`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11devicecontext4).
pub trait DeviceContext4: DeviceContext3 {
    /// The header's `Signal`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11devicecontext4-signal).
    fn signal(&self, fence: &<Self::Api as Direct3D11>::Fence, value: u64) -> Result<(), HResult>;
    /// The header's `Wait`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11devicecontext4-wait).
    fn wait(&self, fence: &<Self::Api as Direct3D11>::Fence, value: u64) -> Result<(), HResult>;
}

/// The header's `ID3DDeviceContextState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3ddevicecontextstate).
pub trait DeviceContextState: DeviceChild {}

/// The header's `ID3D11DomainShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11domainshader).
pub trait DomainShader: DeviceChild {}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11Fence`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11fence).
pub trait Fence: DeviceChild {
    /// The header's `CreateSharedHandle`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11fence-createsharedhandle).
    fn create_shared_handle(
        &self,
        attributes: Option<&component_object_model::structures::SecurityAttributes>,
        dw_access: u32,
        lp_name: Option<&[u16]>,
    ) -> Result<component_object_model::structures::Handle, HResult>;
    /// The header's `GetCompletedValue`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11fence-getcompletedvalue).
    fn get_completed_value(&self) -> u64;
    /// The header's `SetEventOnCompletion`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11fence-seteventoncompletion).
    fn set_event_on_completion(
        &self,
        value: u64,
        h_event: component_object_model::structures::Handle,
    ) -> Result<(), HResult>;
}

/// The header's `ID3D11GeometryShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11geometryshader).
pub trait GeometryShader: DeviceChild {}

/// The header's `ID3D11HullShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11hullshader).
pub trait HullShader: DeviceChild {}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11InfoQueue`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nn-d3d11sdklayers-id3d11infoqueue).
pub trait InfoQueue: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    /// The header's `SetMessageCountLimit`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-setmessagecountlimit).
    fn set_message_count_limit(&self, message_count_limit: u64) -> Result<(), HResult>;
    /// The header's `ClearStoredMessages`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-clearstoredmessages).
    fn clear_stored_messages(&self);
    /// The header's `GetMessage`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-getmessage).
    fn get_message(
        &self,
        message_index: u64,
        message: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, HResult>;
    /// The header's `GetNumMessagesAllowedByStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-getnummessagesallowedbystoragefilter).
    fn get_num_messages_allowed_by_storage_filter(&self) -> u64;
    /// The header's `GetNumMessagesDeniedByStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-getnummessagesdeniedbystoragefilter).
    fn get_num_messages_denied_by_storage_filter(&self) -> u64;
    /// The header's `GetNumStoredMessages`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-getnumstoredmessages).
    fn get_num_stored_messages(&self) -> u64;
    /// The header's `GetNumStoredMessagesAllowedByRetrievalFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-getnumstoredmessagesallowedbyretrievalfilter).
    fn get_num_stored_messages_allowed_by_retrieval_filter(&self) -> u64;
    /// The header's `GetNumMessagesDiscardedByMessageCountLimit`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-getnummessagesdiscardedbymessagecountlimit).
    fn get_num_messages_discarded_by_message_count_limit(&self) -> u64;
    /// The header's `GetMessageCountLimit`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-getmessagecountlimit).
    fn get_message_count_limit(&self) -> u64;
    /// The header's `AddStorageFilterEntries`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-addstoragefilterentries).
    fn add_storage_filter_entries(
        &self,
        filter: &structures::InfoQueueFilter,
    ) -> Result<(), HResult>;
    /// The header's `GetStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-getstoragefilter).
    fn get_storage_filter(
        &self,
        filter: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, HResult>;
    /// The header's `ClearStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-clearstoragefilter).
    fn clear_storage_filter(&self);
    /// The header's `PushEmptyStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-pushemptystoragefilter).
    fn push_empty_storage_filter(&self) -> Result<(), HResult>;
    /// The header's `PushCopyOfStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-pushcopyofstoragefilter).
    fn push_copy_of_storage_filter(&self) -> Result<(), HResult>;
    /// The header's `PushStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-pushstoragefilter).
    fn push_storage_filter(&self, filter: &structures::InfoQueueFilter) -> Result<(), HResult>;
    /// The header's `PopStorageFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-popstoragefilter).
    fn pop_storage_filter(&self);
    /// The header's `GetStorageFilterStackSize`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-getstoragefilterstacksize).
    fn get_storage_filter_stack_size(&self) -> u32;
    /// The header's `AddRetrievalFilterEntries`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-addretrievalfilterentries).
    fn add_retrieval_filter_entries(
        &self,
        filter: &structures::InfoQueueFilter,
    ) -> Result<(), HResult>;
    /// The header's `GetRetrievalFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-getretrievalfilter).
    fn get_retrieval_filter(
        &self,
        filter: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, HResult>;
    /// The header's `ClearRetrievalFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-clearretrievalfilter).
    fn clear_retrieval_filter(&self);
    /// The header's `PushEmptyRetrievalFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-pushemptyretrievalfilter).
    fn push_empty_retrieval_filter(&self) -> Result<(), HResult>;
    /// The header's `PushCopyOfRetrievalFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-pushcopyofretrievalfilter).
    fn push_copy_of_retrieval_filter(&self) -> Result<(), HResult>;
    /// The header's `PushRetrievalFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-pushretrievalfilter).
    fn push_retrieval_filter(&self, filter: &structures::InfoQueueFilter) -> Result<(), HResult>;
    /// The header's `PopRetrievalFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-popretrievalfilter).
    fn pop_retrieval_filter(&self);
    /// The header's `GetRetrievalFilterStackSize`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-getretrievalfilterstacksize).
    fn get_retrieval_filter_stack_size(&self) -> u32;
    /// The header's `AddMessage`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-addmessage).
    fn add_message(
        &self,
        category: enumerations::MessageCategory,
        severity: enumerations::MessageSeverity,
        id: enumerations::MessageId,
        description: &std::ffi::CStr,
    ) -> Result<(), HResult>;
    /// The header's `AddApplicationMessage`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-addapplicationmessage).
    fn add_application_message(
        &self,
        severity: enumerations::MessageSeverity,
        description: &std::ffi::CStr,
    ) -> Result<(), HResult>;
    /// The header's `SetBreakOnCategory`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-setbreakoncategory).
    fn set_break_on_category(
        &self,
        category: enumerations::MessageCategory,
        b_enable: bool,
    ) -> Result<(), HResult>;
    /// The header's `SetBreakOnSeverity`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-setbreakonseverity).
    fn set_break_on_severity(
        &self,
        severity: enumerations::MessageSeverity,
        b_enable: bool,
    ) -> Result<(), HResult>;
    /// The header's `SetBreakOnID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-setbreakonid).
    fn set_break_on_id(&self, id: enumerations::MessageId, b_enable: bool) -> Result<(), HResult>;
    /// The header's `GetBreakOnCategory`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-getbreakoncategory).
    fn get_break_on_category(&self, category: enumerations::MessageCategory) -> bool;
    /// The header's `GetBreakOnSeverity`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-getbreakonseverity).
    fn get_break_on_severity(&self, severity: enumerations::MessageSeverity) -> bool;
    /// The header's `GetBreakOnID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-getbreakonid).
    fn get_break_on_id(&self, id: enumerations::MessageId) -> bool;
    /// The header's `SetMuteDebugOutput`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-setmutedebugoutput).
    fn set_mute_debug_output(&self, b_mute: bool);
    /// The header's `GetMuteDebugOutput`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11infoqueue-getmutedebugoutput).
    fn get_mute_debug_output(&self) -> bool;
}

/// The header's `ID3D11InputLayout`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11inputlayout).
pub trait InputLayout: DeviceChild {}

/// The header's `ID3D11Multithread`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11multithread).
pub trait Multithread: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    /// The header's `Enter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nf-d3d11_4-id3d11multithread-enter).
    fn enter(&self);
    /// The header's `Leave`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nf-d3d11_4-id3d11multithread-leave).
    fn leave(&self);
    /// The header's `SetMultithreadProtected`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nf-d3d11_4-id3d11multithread-setmultithreadprotected).
    fn set_multithread_protected(&self, b_mt_protect: bool) -> bool;
    /// The header's `GetMultithreadProtected`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nf-d3d11_4-id3d11multithread-getmultithreadprotected).
    fn get_multithread_protected(&self) -> bool;
}

/// The header's `ID3D11PixelShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11pixelshader).
pub trait PixelShader: DeviceChild {}

/// The header's `ID3D11Predicate`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11predicate).
pub trait Predicate: Query {}

/// The header's `ID3D11Query`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11query).
pub trait Query: Asynchronous {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11query-getdesc).
    fn get_desc(&self) -> structures::QueryDesc;
}

/// The header's `ID3D11Query1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11query1).
pub trait Query1: Query {
    /// The header's `GetDesc1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11query1-getdesc1).
    fn get_desc1(&self) -> structures::QueryDesc1;
}

/// The header's `ID3D11RasterizerState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11rasterizerstate).
pub trait RasterizerState: DeviceChild {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11rasterizerstate-getdesc).
    fn get_desc(&self) -> structures::RasterizerDesc;
}

/// The header's `ID3D11RasterizerState1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3d11rasterizerstate1).
pub trait RasterizerState1: RasterizerState {
    /// The header's `GetDesc1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11rasterizerstate1-getdesc1).
    fn get_desc1(&self) -> structures::RasterizerDesc1;
}

/// The header's `ID3D11RasterizerState2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11rasterizerstate2).
pub trait RasterizerState2: RasterizerState1 {
    /// The header's `GetDesc2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11rasterizerstate2-getdesc2).
    fn get_desc2(&self) -> structures::RasterizerDesc2;
}

/// The header's `ID3D11RefDefaultTrackingOptions`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nn-d3d11sdklayers-id3d11refdefaulttrackingoptions).
pub trait RefDefaultTrackingOptions: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    /// The header's `SetTrackingOptions`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11refdefaulttrackingoptions-settrackingoptions).
    fn set_tracking_options(&self, resource_type_flags: u32, options: u32) -> Result<(), HResult>;
}

/// The header's `ID3D11RefTrackingOptions`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nn-d3d11sdklayers-id3d11reftrackingoptions).
pub trait RefTrackingOptions: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    /// The header's `SetTrackingOptions`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11reftrackingoptions-settrackingoptions).
    fn set_tracking_options(&self, u_options: u32) -> Result<(), HResult>;
}

/// The header's `ID3D11RenderTargetView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11rendertargetview).
pub trait RenderTargetView: View {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11rendertargetview-getdesc).
    fn get_desc(&self) -> structures::RenderTargetViewDesc;
}

/// The header's `ID3D11RenderTargetView1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11rendertargetview1).
pub trait RenderTargetView1: RenderTargetView {
    /// The header's `GetDesc1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11rendertargetview1-getdesc1).
    fn get_desc1(&self) -> structures::RenderTargetViewDesc1;
}

/// The header's `ID3D11Resource`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11resource).
pub trait Resource: DeviceChild {
    /// The header's `GetType`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11resource-gettype).
    fn get_type(&self) -> enumerations::ResourceDimension;
    /// The header's `SetEvictionPriority`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11resource-setevictionpriority).
    fn set_eviction_priority(&self, eviction_priority: u32);
    /// The header's `GetEvictionPriority`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11resource-getevictionpriority).
    fn get_eviction_priority(&self) -> u32;
}

/// The header's `ID3D11SamplerState`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11samplerstate).
pub trait SamplerState: DeviceChild {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11samplerstate-getdesc).
    fn get_desc(&self) -> structures::SamplerDesc;
}

/// The header's `ID3D11ShaderResourceView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11shaderresourceview).
pub trait ShaderResourceView: View {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11shaderresourceview-getdesc).
    fn get_desc(&self) -> structures::ShaderResourceViewDesc;
}

/// The header's `ID3D11ShaderResourceView1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11shaderresourceview1).
pub trait ShaderResourceView1: ShaderResourceView {
    /// The header's `GetDesc1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11shaderresourceview1-getdesc1).
    fn get_desc1(&self) -> structures::ShaderResourceViewDesc1;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11ShaderTrace`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/nn-d3d11shadertracing-id3d11shadertrace).
pub trait ShaderTrace: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    /// The header's `TraceReady`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/nf-d3d11shadertracing-id3d11shadertrace-traceready).
    fn trace_ready(&self) -> Result<u64, HResult>;
    /// The header's `ResetTrace`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/nf-d3d11shadertracing-id3d11shadertrace-resettrace).
    fn reset_trace(&self);
    /// The header's `GetTraceStats`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/nf-d3d11shadertracing-id3d11shadertrace-gettracestats).
    fn get_trace_stats(&self) -> Result<structures::TraceStats, HResult>;
    /// The header's `PSSelectStamp`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/nf-d3d11shadertracing-id3d11shadertrace-psselectstamp).
    fn ps_select_stamp(&self, stamp_index: u32) -> Result<(), HResult>;
    /// The header's `GetInitialRegisterContents`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/nf-d3d11shadertracing-id3d11shadertrace-getinitialregistercontents).
    fn get_initial_register_contents(
        &self,
        register: &structures::TraceRegister,
    ) -> Result<structures::TraceValue, HResult>;
    /// The header's `GetStep`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/nf-d3d11shadertracing-id3d11shadertrace-getstep).
    fn get_step(&self, step_index: u32) -> Result<structures::TraceStep, HResult>;
    /// The header's `GetWrittenRegister`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/nf-d3d11shadertracing-id3d11shadertrace-getwrittenregister).
    fn get_written_register(
        &self,
        step_index: u32,
        written_register_index: u32,
    ) -> Result<(structures::TraceRegister, structures::TraceValue), HResult>;
    /// The header's `GetReadRegister`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/nf-d3d11shadertracing-id3d11shadertrace-getreadregister).
    fn get_read_register(
        &self,
        step_index: u32,
        read_register_index: u32,
    ) -> Result<(structures::TraceRegister, structures::TraceValue), HResult>;
}

/// The header's `ID3D11ShaderTraceFactory`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/nn-d3d11shadertracing-id3d11shadertracefactory).
pub trait ShaderTraceFactory: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    type Api: Direct3D11;

    /// The header's `CreateShaderTrace`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11shadertracing/nf-d3d11shadertracing-id3d11shadertracefactory-createshadertrace).
    fn create_shader_trace(
        &self,
        shader: UnknownRef<'_, Self::Api>,
        trace_desc: &structures::ShaderTraceDesc,
    ) -> Result<<Self::Api as Direct3D11>::ShaderTrace, HResult>;
}

/// The header's `ID3D11SwitchToRef`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nn-d3d11sdklayers-id3d11switchtoref).
pub trait SwitchToRef: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    /// The header's `SetUseRef`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11switchtoref-setuseref).
    fn set_use_ref(&self, use_ref: bool) -> bool;
    /// The header's `GetUseRef`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11switchtoref-getuseref).
    fn get_use_ref(&self) -> bool;
}

/// The header's `ID3D11Texture1D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11texture1d).
pub trait Texture1D: Resource {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11texture1d-getdesc).
    fn get_desc(&self) -> structures::Texture1DDesc;
}

/// The header's `ID3D11Texture2D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11texture2d).
pub trait Texture2D: Resource {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11texture2d-getdesc).
    fn get_desc(&self) -> structures::Texture2DDesc;
}

/// The header's `ID3D11Texture2D1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11texture2d1).
pub trait Texture2D1: Texture2D {
    /// The header's `GetDesc1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11texture2d1-getdesc1).
    fn get_desc1(&self) -> structures::Texture2DDesc1;
}

/// The header's `ID3D11Texture3D`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11texture3d).
pub trait Texture3D: Resource {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11texture3d-getdesc).
    fn get_desc(&self) -> structures::Texture3DDesc;
}

/// The header's `ID3D11Texture3D1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11texture3d1).
pub trait Texture3D1: Texture3D {
    /// The header's `GetDesc1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11texture3d1-getdesc1).
    fn get_desc1(&self) -> structures::Texture3DDesc1;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11TracingDevice`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nn-d3d11sdklayers-id3d11tracingdevice).
pub trait TracingDevice: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    type Api: Direct3D11;

    /// The header's `SetShaderTrackingOptionsByType`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11tracingdevice-setshadertrackingoptionsbytype).
    fn set_shader_tracking_options_by_type(
        &self,
        resource_type_flags: u32,
        options: u32,
    ) -> Result<(), HResult>;
    /// The header's `SetShaderTrackingOptions`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11sdklayers/nf-d3d11sdklayers-id3d11tracingdevice-setshadertrackingoptions).
    fn set_shader_tracking_options(
        &self,
        shader: UnknownRef<'_, Self::Api>,
        options: u32,
    ) -> Result<(), HResult>;
}

/// The header's `ID3D11UnorderedAccessView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11unorderedaccessview).
pub trait UnorderedAccessView: View {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11unorderedaccessview-getdesc).
    fn get_desc(&self) -> structures::UnorderedAccessViewDesc;
}

/// The header's `ID3D11UnorderedAccessView1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nn-d3d11_3-id3d11unorderedaccessview1).
pub trait UnorderedAccessView1: UnorderedAccessView {
    /// The header's `GetDesc1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_3/nf-d3d11_3-id3d11unorderedaccessview1-getdesc1).
    fn get_desc1(&self) -> structures::UnorderedAccessViewDesc1;
}

/// The header's `ID3DUserDefinedAnnotation`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3duserdefinedannotation).
pub trait UserDefinedAnnotation: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    /// The header's `BeginEvent`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3duserdefinedannotation-beginevent).
    fn begin_event(&self, name: &[u16]) -> i32;
    /// The header's `EndEvent`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3duserdefinedannotation-endevent).
    fn end_event(&self) -> i32;
    /// The header's `SetMarker`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3duserdefinedannotation-setmarker).
    fn set_marker(&self, name: &[u16]);
    /// The header's `GetStatus`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3duserdefinedannotation-getstatus).
    fn get_status(&self) -> bool;
}

/// The header's `ID3D11VertexShader`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11vertexshader).
pub trait VertexShader: DeviceChild {}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11VideoContext`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videocontext).
pub trait VideoContext: DeviceChild {
    /// The header's `GetDecoderBuffer`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-getdecoderbuffer).
    fn get_decoder_buffer(
        &self,
        decoder: &<Self::Api as Direct3D11>::VideoDecoder,
        type_: enumerations::VideoDecoderBufferType,
        buffer: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<u32, HResult>;
    /// The header's `ReleaseDecoderBuffer`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-releasedecoderbuffer).
    fn release_decoder_buffer(
        &self,
        decoder: &<Self::Api as Direct3D11>::VideoDecoder,
        type_: enumerations::VideoDecoderBufferType,
    ) -> Result<(), HResult>;
    /// The header's `DecoderBeginFrame`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-decoderbeginframe).
    fn decoder_begin_frame(
        &self,
        decoder: &<Self::Api as Direct3D11>::VideoDecoder,
        view: &<Self::Api as Direct3D11>::VideoDecoderOutputView,
        content_key: Option<&[u8]>,
    ) -> Result<(), HResult>;
    /// The header's `DecoderEndFrame`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-decoderendframe).
    fn decoder_end_frame(
        &self,
        decoder: &<Self::Api as Direct3D11>::VideoDecoder,
    ) -> Result<(), HResult>;
    /// The header's `SubmitDecoderBuffers`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-submitdecoderbuffers).
    fn submit_decoder_buffers(
        &self,
        decoder: &<Self::Api as Direct3D11>::VideoDecoder,
        buffer_desc: &[structures::VideoDecoderBufferDesc],
    ) -> Result<(), HResult>;
    /// The header's `DecoderExtension`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-decoderextension).
    fn decoder_extension(
        &self,
        decoder: &<Self::Api as Direct3D11>::VideoDecoder,
        extension_data: &structures::VideoDecoderExtension<Self::Api>,
    ) -> i32;
    /// The header's `VideoProcessorSetOutputTargetRect`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetoutputtargetrect).
    fn video_processor_set_output_target_rect(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        enable: bool,
        rect: Option<&component_object_model::structures::Rect>,
    );
    /// The header's `VideoProcessorSetOutputBackgroundColor`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetoutputbackgroundcolor).
    fn video_processor_set_output_background_color(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        y_cb_cr: bool,
        color: &structures::VideoColor,
    );
    /// The header's `VideoProcessorSetOutputColorSpace`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetoutputcolorspace).
    fn video_processor_set_output_color_space(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        color_space: &structures::VideoProcessorColorSpace,
    );
    /// The header's `VideoProcessorSetOutputAlphaFillMode`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetoutputalphafillmode).
    fn video_processor_set_output_alpha_fill_mode(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        alpha_fill_mode: enumerations::VideoProcessorAlphaFillMode,
        stream_index: u32,
    );
    /// The header's `VideoProcessorSetOutputConstriction`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetoutputconstriction).
    fn video_processor_set_output_constriction(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        enable: bool,
        size: component_object_model::structures::Size,
    );
    /// The header's `VideoProcessorSetOutputStereoMode`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetoutputstereomode).
    fn video_processor_set_output_stereo_mode(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        enable: bool,
    );
    /// The header's `VideoProcessorSetOutputExtension`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetoutputextension).
    fn video_processor_set_output_extension(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        extension_guid: &component_object_model::structures::Guid,
        data_size: u32,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> i32;
    /// The header's `VideoProcessorGetOutputTargetRect`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetoutputtargetrect).
    fn video_processor_get_output_target_rect(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
    ) -> (bool, component_object_model::structures::Rect);
    /// The header's `VideoProcessorGetOutputBackgroundColor`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetoutputbackgroundcolor).
    fn video_processor_get_output_background_color(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
    ) -> (bool, structures::VideoColor);
    /// The header's `VideoProcessorGetOutputColorSpace`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetoutputcolorspace).
    fn video_processor_get_output_color_space(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
    ) -> structures::VideoProcessorColorSpace;
    /// The header's `VideoProcessorGetOutputAlphaFillMode`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetoutputalphafillmode).
    fn video_processor_get_output_alpha_fill_mode(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
    ) -> (enumerations::VideoProcessorAlphaFillMode, u32);
    /// The header's `VideoProcessorGetOutputConstriction`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetoutputconstriction).
    fn video_processor_get_output_constriction(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
    ) -> (bool, component_object_model::structures::Size);
    /// The header's `VideoProcessorGetOutputStereoMode`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetoutputstereomode).
    fn video_processor_get_output_stereo_mode(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
    ) -> bool;
    /// The header's `VideoProcessorGetOutputExtension`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetoutputextension).
    fn video_processor_get_output_extension(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        extension_guid: &component_object_model::structures::Guid,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> i32;
    /// The header's `VideoProcessorSetStreamFrameFormat`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetstreamframeformat).
    fn video_processor_set_stream_frame_format(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        frame_format: enumerations::VideoFrameFormat,
    );
    /// The header's `VideoProcessorSetStreamColorSpace`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetstreamcolorspace).
    fn video_processor_set_stream_color_space(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        color_space: &structures::VideoProcessorColorSpace,
    );
    /// The header's `VideoProcessorSetStreamOutputRate`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetstreamoutputrate).
    fn video_processor_set_stream_output_rate(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        output_rate: enumerations::VideoProcessorOutputRate,
        repeat_frame: bool,
        custom_rate: Option<&directx_graphics_infrastructure::structures::Rational>,
    );
    /// The header's `VideoProcessorSetStreamSourceRect`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetstreamsourcerect).
    fn video_processor_set_stream_source_rect(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        rect: Option<&component_object_model::structures::Rect>,
    );
    /// The header's `VideoProcessorSetStreamDestRect`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetstreamdestrect).
    fn video_processor_set_stream_dest_rect(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        rect: Option<&component_object_model::structures::Rect>,
    );
    /// The header's `VideoProcessorSetStreamAlpha`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetstreamalpha).
    fn video_processor_set_stream_alpha(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        alpha: f32,
    );
    /// The header's `VideoProcessorSetStreamPalette`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetstreampalette).
    fn video_processor_set_stream_palette(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        entries: &[u32],
    );
    /// The header's `VideoProcessorSetStreamPixelAspectRatio`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetstreampixelaspectratio).
    fn video_processor_set_stream_pixel_aspect_ratio(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        source_aspect_ratio: Option<&directx_graphics_infrastructure::structures::Rational>,
        destination_aspect_ratio: Option<&directx_graphics_infrastructure::structures::Rational>,
    );
    /// The header's `VideoProcessorSetStreamLumaKey`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetstreamlumakey).
    fn video_processor_set_stream_luma_key(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        lower: f32,
        upper: f32,
    );
    /// The header's `VideoProcessorSetStreamStereoFormat`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetstreamstereoformat).
    #[allow(
        clippy::too_many_arguments,
        reason = "the method's own parameters, one to one (MAPPING.md)"
    )]
    fn video_processor_set_stream_stereo_format(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        format: enumerations::VideoProcessorStereoFormat,
        left_view_frame0: bool,
        base_view_frame0: bool,
        flip_mode: enumerations::VideoProcessorStereoFlipMode,
        mono_offset: i32,
    );
    /// The header's `VideoProcessorSetStreamAutoProcessingMode`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetstreamautoprocessingmode).
    fn video_processor_set_stream_auto_processing_mode(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
    );
    /// The header's `VideoProcessorSetStreamFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetstreamfilter).
    fn video_processor_set_stream_filter(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        filter: enumerations::VideoProcessorFilter,
        enable: bool,
        level: i32,
    );
    /// The header's `VideoProcessorSetStreamExtension`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetstreamextension).
    fn video_processor_set_stream_extension(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        extension_guid: &component_object_model::structures::Guid,
        data_size: u32,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> i32;
    /// The header's `VideoProcessorGetStreamFrameFormat`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetstreamframeformat).
    fn video_processor_get_stream_frame_format(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> enumerations::VideoFrameFormat;
    /// The header's `VideoProcessorGetStreamColorSpace`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetstreamcolorspace).
    fn video_processor_get_stream_color_space(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> structures::VideoProcessorColorSpace;
    /// The header's `VideoProcessorGetStreamOutputRate`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetstreamoutputrate).
    fn video_processor_get_stream_output_rate(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (
        enumerations::VideoProcessorOutputRate,
        bool,
        directx_graphics_infrastructure::structures::Rational,
    );
    /// The header's `VideoProcessorGetStreamSourceRect`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetstreamsourcerect).
    fn video_processor_get_stream_source_rect(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (bool, component_object_model::structures::Rect);
    /// The header's `VideoProcessorGetStreamDestRect`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetstreamdestrect).
    fn video_processor_get_stream_dest_rect(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (bool, component_object_model::structures::Rect);
    /// The header's `VideoProcessorGetStreamAlpha`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetstreamalpha).
    fn video_processor_get_stream_alpha(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (bool, f32);
    /// The header's `VideoProcessorGetStreamPalette`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetstreampalette).
    fn video_processor_get_stream_palette(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        count: u32,
    ) -> Vec<u32>;
    /// The header's `VideoProcessorGetStreamPixelAspectRatio`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetstreampixelaspectratio).
    fn video_processor_get_stream_pixel_aspect_ratio(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (
        bool,
        directx_graphics_infrastructure::structures::Rational,
        directx_graphics_infrastructure::structures::Rational,
    );
    /// The header's `VideoProcessorGetStreamLumaKey`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetstreamlumakey).
    fn video_processor_get_stream_luma_key(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (bool, f32, f32);
    /// The header's `VideoProcessorGetStreamStereoFormat`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetstreamstereoformat).
    fn video_processor_get_stream_stereo_format(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (
        bool,
        enumerations::VideoProcessorStereoFormat,
        bool,
        bool,
        enumerations::VideoProcessorStereoFlipMode,
        i32,
    );
    /// The header's `VideoProcessorGetStreamAutoProcessingMode`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetstreamautoprocessingmode).
    fn video_processor_get_stream_auto_processing_mode(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> bool;
    /// The header's `VideoProcessorGetStreamFilter`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetstreamfilter).
    fn video_processor_get_stream_filter(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        filter: enumerations::VideoProcessorFilter,
    ) -> (bool, i32);
    /// The header's `VideoProcessorGetStreamExtension`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetstreamextension).
    fn video_processor_get_stream_extension(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        extension_guid: &component_object_model::structures::Guid,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> i32;
    /// The header's `VideoProcessorBlt`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorblt).
    fn video_processor_blt(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        view: &<Self::Api as Direct3D11>::VideoProcessorOutputView,
        output_frame: u32,
        streams: &[structures::VideoProcessorStream<Self::Api>],
    ) -> Result<(), HResult>;
    /// The header's `NegotiateCryptoSessionKeyExchange`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-negotiatecryptosessionkeyexchange).
    fn negotiate_crypto_session_key_exchange(
        &self,
        crypto_session: &<Self::Api as Direct3D11>::CryptoSession,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), HResult>;
    /// The header's `EncryptionBlt`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-encryptionblt).
    fn encryption_blt(
        &self,
        crypto_session: &<Self::Api as Direct3D11>::CryptoSession,
        src_surface: &<Self::Api as Direct3D11>::Texture2D,
        dst_surface: &<Self::Api as Direct3D11>::Texture2D,
        iv: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    );
    /// The header's `DecryptionBlt`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-decryptionblt).
    fn decryption_blt(
        &self,
        crypto_session: &<Self::Api as Direct3D11>::CryptoSession,
        src_surface: &<Self::Api as Direct3D11>::Texture2D,
        dst_surface: &<Self::Api as Direct3D11>::Texture2D,
        encrypted_block_info: Option<&structures::EncryptedBlockInfo>,
        content_key: Option<&[u8]>,
        iv: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    );
    /// The header's `StartSessionKeyRefresh`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-startsessionkeyrefresh).
    fn start_session_key_refresh(
        &self,
        crypto_session: &<Self::Api as Direct3D11>::CryptoSession,
        random_number: &impl wcontainers_volatile_bytes::VolatileBytes,
    );
    /// The header's `FinishSessionKeyRefresh`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-finishsessionkeyrefresh).
    fn finish_session_key_refresh(&self, crypto_session: &<Self::Api as Direct3D11>::CryptoSession);
    /// The header's `GetEncryptionBltKey`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-getencryptionbltkey).
    fn get_encryption_blt_key(
        &self,
        crypto_session: &<Self::Api as Direct3D11>::CryptoSession,
        readback_key: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), HResult>;
    /// The header's `NegotiateAuthenticatedChannelKeyExchange`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-negotiateauthenticatedchannelkeyexchange).
    fn negotiate_authenticated_channel_key_exchange(
        &self,
        channel: &<Self::Api as Direct3D11>::AuthenticatedChannel,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), HResult>;
    /// The header's `QueryAuthenticatedChannel`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-queryauthenticatedchannel).
    fn query_authenticated_channel(
        &self,
        channel: &<Self::Api as Direct3D11>::AuthenticatedChannel,
        input: &[u8],
        output: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), HResult>;
    /// The header's `ConfigureAuthenticatedChannel`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-configureauthenticatedchannel).
    fn configure_authenticated_channel(
        &self,
        channel: &<Self::Api as Direct3D11>::AuthenticatedChannel,
        input: &[u8],
    ) -> Result<structures::AuthenticatedConfigureOutput, HResult>;
    /// The header's `VideoProcessorSetStreamRotation`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetstreamrotation).
    fn video_processor_set_stream_rotation(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        rotation: enumerations::VideoProcessorRotation,
    );
    /// The header's `VideoProcessorGetStreamRotation`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorgetstreamrotation).
    fn video_processor_get_stream_rotation(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (bool, enumerations::VideoProcessorRotation);
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11VideoContext1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3d11videocontext1).
pub trait VideoContext1: VideoContext {
    /// The header's `SubmitDecoderBuffers1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videocontext1-submitdecoderbuffers1).
    fn submit_decoder_buffers1(
        &self,
        decoder: &<Self::Api as Direct3D11>::VideoDecoder,
        buffer_desc: &[structures::VideoDecoderBufferDesc1],
    ) -> Result<(), HResult>;
    /// The header's `GetDataForNewHardwareKey`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videocontext1-getdatafornewhardwarekey).
    fn get_data_for_new_hardware_key(
        &self,
        crypto_session: &<Self::Api as Direct3D11>::CryptoSession,
        privat_input_data: &[u8],
    ) -> Result<u64, HResult>;
    /// The header's `CheckCryptoSessionStatus`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videocontext1-checkcryptosessionstatus).
    fn check_crypto_session_status(
        &self,
        crypto_session: &<Self::Api as Direct3D11>::CryptoSession,
    ) -> Result<enumerations::CryptoSessionStatus, HResult>;
    /// The header's `DecoderEnableDownsampling`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videocontext1-decoderenabledownsampling).
    fn decoder_enable_downsampling(
        &self,
        decoder: &<Self::Api as Direct3D11>::VideoDecoder,
        input_color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
        output_desc: &structures::VideoSampleDesc,
        reference_frame_count: u32,
    ) -> Result<(), HResult>;
    /// The header's `DecoderUpdateDownsampling`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videocontext1-decoderupdatedownsampling).
    fn decoder_update_downsampling(
        &self,
        decoder: &<Self::Api as Direct3D11>::VideoDecoder,
        output_desc: &structures::VideoSampleDesc,
    ) -> Result<(), HResult>;
    /// The header's `VideoProcessorSetOutputColorSpace1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videocontext1-videoprocessorsetoutputcolorspace1).
    fn video_processor_set_output_color_space1(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
    );
    /// The header's `VideoProcessorSetOutputShaderUsage`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videocontext1-videoprocessorsetoutputshaderusage).
    fn video_processor_set_output_shader_usage(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        shader_usage: bool,
    );
    /// The header's `VideoProcessorGetOutputColorSpace1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videocontext1-videoprocessorgetoutputcolorspace1).
    fn video_processor_get_output_color_space1(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
    ) -> directx_graphics_infrastructure::enumerations::ColorSpaceType;
    /// The header's `VideoProcessorGetOutputShaderUsage`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videocontext1-videoprocessorgetoutputshaderusage).
    fn video_processor_get_output_shader_usage(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
    ) -> bool;
    /// The header's `VideoProcessorSetStreamColorSpace1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videocontext1-videoprocessorsetstreamcolorspace1).
    fn video_processor_set_stream_color_space1(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
    );
    /// The header's `VideoProcessorSetStreamMirror`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videocontext1-videoprocessorsetstreammirror).
    fn video_processor_set_stream_mirror(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        enable: bool,
        flip_horizontal: bool,
        flip_vertical: bool,
    );
    /// The header's `VideoProcessorGetStreamColorSpace1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videocontext1-videoprocessorgetstreamcolorspace1).
    fn video_processor_get_stream_color_space1(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> directx_graphics_infrastructure::enumerations::ColorSpaceType;
    /// The header's `VideoProcessorGetStreamMirror`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videocontext1-videoprocessorgetstreammirror).
    fn video_processor_get_stream_mirror(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
    ) -> (bool, bool, bool);
    /// The header's `VideoProcessorGetBehaviorHints`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videocontext1-videoprocessorgetbehaviorhints).
    fn video_processor_get_behavior_hints(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        output_width: u32,
        output_height: u32,
        output_format: directx_graphics_infrastructure::enumerations::Format,
        streams: &[structures::VideoProcessorStreamBehaviorHint],
    ) -> Result<u32, HResult>;
}

/// The header's `ID3D11VideoContext2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11videocontext2).
pub trait VideoContext2: VideoContext1 {
    /// The header's `VideoProcessorSetOutputHDRMetaData`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nf-d3d11_4-id3d11videocontext2-videoprocessorsetoutputhdrmetadata).
    fn video_processor_set_output_hdr_meta_data(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        type_: directx_graphics_infrastructure::enumerations::HdrMetadataType,
        hdr_meta_data: Option<&[u8]>,
    );
    /// The header's `VideoProcessorGetOutputHDRMetaData`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nf-d3d11_4-id3d11videocontext2-videoprocessorgetoutputhdrmetadata).
    fn video_processor_get_output_hdr_meta_data(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        meta_data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> directx_graphics_infrastructure::enumerations::HdrMetadataType;
    /// The header's `VideoProcessorSetStreamHDRMetaData`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nf-d3d11_4-id3d11videocontext2-videoprocessorsetstreamhdrmetadata).
    fn video_processor_set_stream_hdr_meta_data(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        type_: directx_graphics_infrastructure::enumerations::HdrMetadataType,
        hdr_meta_data: Option<&[u8]>,
    );
    /// The header's `VideoProcessorGetStreamHDRMetaData`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nf-d3d11_4-id3d11videocontext2-videoprocessorgetstreamhdrmetadata).
    fn video_processor_get_stream_hdr_meta_data(
        &self,
        video_processor: &<Self::Api as Direct3D11>::VideoProcessor,
        stream_index: u32,
        meta_data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> directx_graphics_infrastructure::enumerations::HdrMetadataType;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11VideoContext3`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11videocontext3).
pub trait VideoContext3: VideoContext2 {
    /// The header's `DecoderBeginFrame1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nf-d3d11_4-id3d11videocontext3-decoderbeginframe1).
    fn decoder_begin_frame1(
        &self,
        decoder: &<Self::Api as Direct3D11>::VideoDecoder,
        view: &<Self::Api as Direct3D11>::VideoDecoderOutputView,
        content_key: Option<&[u8]>,
        histogram_offsets: &[u32],
        histogram_buffers: &[Option<&<Self::Api as Direct3D11>::Buffer>],
    ) -> Result<(), HResult>;
    fn submit_decoder_buffers2(
        &self,
        decoder: &<Self::Api as Direct3D11>::VideoDecoder,
        buffer_desc: &[structures::VideoDecoderBufferDesc2],
    ) -> Result<(), HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11VideoDecoder`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videodecoder).
pub trait VideoDecoder: DeviceChild {
    /// The header's `GetCreationParameters`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodecoder-getcreationparameters).
    fn get_creation_parameters(
        &self,
    ) -> Result<(structures::VideoDecoderDesc, structures::VideoDecoderConfig), HResult>;
    /// The header's `GetDriverHandle`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodecoder-getdriverhandle).
    fn get_driver_handle(&self) -> Result<component_object_model::structures::Handle, HResult>;
}

/// The header's `ID3D11VideoDecoderOutputView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videodecoderoutputview).
pub trait VideoDecoderOutputView: View {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodecoderoutputview-getdesc).
    fn get_desc(&self) -> structures::VideoDecoderOutputViewDesc;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11VideoDevice`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videodevice).
pub trait VideoDevice: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    type Api: Direct3D11;

    /// The header's `CreateVideoDecoder`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-createvideodecoder).
    fn create_video_decoder(
        &self,
        video_desc: &structures::VideoDecoderDesc,
        config: &structures::VideoDecoderConfig,
    ) -> Result<<Self::Api as Direct3D11>::VideoDecoder, HResult>;
    /// The header's `CreateVideoProcessor`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-createvideoprocessor).
    fn create_video_processor(
        &self,
        enum_: &<Self::Api as Direct3D11>::VideoProcessorEnumerator,
        rate_conversion_index: u32,
    ) -> Result<<Self::Api as Direct3D11>::VideoProcessor, HResult>;
    /// The header's `CreateAuthenticatedChannel`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-createauthenticatedchannel).
    fn create_authenticated_channel(
        &self,
        channel_type: enumerations::AuthenticatedChannelType,
    ) -> Result<<Self::Api as Direct3D11>::AuthenticatedChannel, HResult>;
    /// The header's `CreateCryptoSession`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-createcryptosession).
    fn create_crypto_session(
        &self,
        crypto_type: &component_object_model::structures::Guid,
        decoder_profile: Option<&component_object_model::structures::Guid>,
        key_exchange_type: &component_object_model::structures::Guid,
    ) -> Result<<Self::Api as Direct3D11>::CryptoSession, HResult>;
    /// The header's `CreateVideoDecoderOutputView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-createvideodecoderoutputview).
    fn create_video_decoder_output_view(
        &self,
        resource: ResourceRef<'_, Self::Api>,
        desc: &structures::VideoDecoderOutputViewDesc,
    ) -> Result<<Self::Api as Direct3D11>::VideoDecoderOutputView, HResult>;
    /// The header's `CreateVideoProcessorInputView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-createvideoprocessorinputview).
    fn create_video_processor_input_view(
        &self,
        resource: ResourceRef<'_, Self::Api>,
        enum_: &<Self::Api as Direct3D11>::VideoProcessorEnumerator,
        desc: &structures::VideoProcessorInputViewDesc,
    ) -> Result<<Self::Api as Direct3D11>::VideoProcessorInputView, HResult>;
    /// The header's `CreateVideoProcessorOutputView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-createvideoprocessoroutputview).
    fn create_video_processor_output_view(
        &self,
        resource: ResourceRef<'_, Self::Api>,
        enum_: &<Self::Api as Direct3D11>::VideoProcessorEnumerator,
        desc: &structures::VideoProcessorOutputViewDesc,
    ) -> Result<<Self::Api as Direct3D11>::VideoProcessorOutputView, HResult>;
    /// The header's `CreateVideoProcessorEnumerator`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-createvideoprocessorenumerator).
    fn create_video_processor_enumerator(
        &self,
        desc: &structures::VideoProcessorContentDesc,
    ) -> Result<<Self::Api as Direct3D11>::VideoProcessorEnumerator, HResult>;
    /// The header's `GetVideoDecoderProfileCount`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-getvideodecoderprofilecount).
    fn get_video_decoder_profile_count(&self) -> u32;
    /// The header's `GetVideoDecoderProfile`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-getvideodecoderprofile).
    fn get_video_decoder_profile(
        &self,
        index: u32,
    ) -> Result<component_object_model::structures::Guid, HResult>;
    /// The header's `CheckVideoDecoderFormat`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-checkvideodecoderformat).
    fn check_video_decoder_format(
        &self,
        decoder_profile: &component_object_model::structures::Guid,
        format: directx_graphics_infrastructure::enumerations::Format,
    ) -> Result<bool, HResult>;
    /// The header's `GetVideoDecoderConfigCount`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-getvideodecoderconfigcount).
    fn get_video_decoder_config_count(
        &self,
        desc: &structures::VideoDecoderDesc,
    ) -> Result<u32, HResult>;
    /// The header's `GetVideoDecoderConfig`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-getvideodecoderconfig).
    fn get_video_decoder_config(
        &self,
        desc: &structures::VideoDecoderDesc,
        index: u32,
    ) -> Result<structures::VideoDecoderConfig, HResult>;
    /// The header's `GetContentProtectionCaps`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-getcontentprotectioncaps).
    fn get_content_protection_caps(
        &self,
        crypto_type: Option<&component_object_model::structures::Guid>,
        decoder_profile: Option<&component_object_model::structures::Guid>,
    ) -> Result<structures::VideoContentProtectionCaps, HResult>;
    /// The header's `CheckCryptoKeyExchange`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-checkcryptokeyexchange).
    fn check_crypto_key_exchange(
        &self,
        crypto_type: &component_object_model::structures::Guid,
        decoder_profile: Option<&component_object_model::structures::Guid>,
        index: u32,
    ) -> Result<component_object_model::structures::Guid, HResult>;
    /// The header's `SetPrivateData`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-setprivatedata).
    fn set_private_data(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<&[u8]>,
    ) -> Result<(), HResult>;
    /// The header's `SetPrivateDataInterface`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videodevice-setprivatedatainterface).
    fn set_private_data_interface(
        &self,
        guid: &component_object_model::structures::Guid,
        data: Option<UnknownRef<'_, Self::Api>>,
    ) -> Result<(), HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11VideoDevice1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3d11videodevice1).
pub trait VideoDevice1: VideoDevice {
    /// The header's `GetCryptoSessionPrivateDataSize`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videodevice1-getcryptosessionprivatedatasize).
    fn get_crypto_session_private_data_size(
        &self,
        crypto_type: &component_object_model::structures::Guid,
        decoder_profile: Option<&component_object_model::structures::Guid>,
        key_exchange_type: &component_object_model::structures::Guid,
    ) -> Result<(u32, u32), HResult>;
    /// The header's `GetVideoDecoderCaps`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videodevice1-getvideodecodercaps).
    fn get_video_decoder_caps(
        &self,
        decoder_profile: &component_object_model::structures::Guid,
        sample_width: u32,
        sample_height: u32,
        frame_rate: &directx_graphics_infrastructure::structures::Rational,
        bit_rate: u32,
        crypto_type: Option<&component_object_model::structures::Guid>,
    ) -> Result<u32, HResult>;
    /// The header's `CheckVideoDecoderDownsampling`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videodevice1-checkvideodecoderdownsampling).
    fn check_video_decoder_downsampling(
        &self,
        input_desc: &structures::VideoDecoderDesc,
        input_color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
        input_config: &structures::VideoDecoderConfig,
        frame_rate: &directx_graphics_infrastructure::structures::Rational,
        output_desc: &structures::VideoSampleDesc,
    ) -> Result<(bool, bool), HResult>;
    /// The header's `RecommendVideoDecoderDownsampleParameters`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videodevice1-recommendvideodecoderdownsampleparameters).
    fn recommend_video_decoder_downsample_parameters(
        &self,
        input_desc: &structures::VideoDecoderDesc,
        input_color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
        input_config: &structures::VideoDecoderConfig,
        frame_rate: &directx_graphics_infrastructure::structures::Rational,
    ) -> Result<structures::VideoSampleDesc, HResult>;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11VideoDevice2`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11videodevice2).
pub trait VideoDevice2: VideoDevice1 {
    /// The header's `CheckFeatureSupport`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_4/nf-d3d11_4-id3d11videodevice2-checkfeaturesupport).
    fn check_feature_support(
        &self,
        feature: enumerations::FeatureVideo,
        feature_support_data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), HResult>;
    fn negotiate_crypto_session_key_exchange_mt(
        &self,
        crypto_session: &<Self::Api as Direct3D11>::CryptoSession,
        flags: enumerations::CryptoSessionKeyExchangeFlags,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), HResult>;
}

/// The header's `ID3D11VideoProcessor`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videoprocessor).
pub trait VideoProcessor: DeviceChild {
    /// The header's `GetContentDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videoprocessor-getcontentdesc).
    fn get_content_desc(&self) -> structures::VideoProcessorContentDesc;
    /// The header's `GetRateConversionCaps`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videoprocessor-getrateconversioncaps).
    fn get_rate_conversion_caps(&self) -> structures::VideoProcessorRateConversionCaps;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3D11VideoProcessorEnumerator`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videoprocessorenumerator).
pub trait VideoProcessorEnumerator: DeviceChild {
    /// The header's `GetVideoProcessorContentDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videoprocessorenumerator-getvideoprocessorcontentdesc).
    fn get_video_processor_content_desc(
        &self,
    ) -> Result<structures::VideoProcessorContentDesc, HResult>;
    /// The header's `CheckVideoProcessorFormat`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videoprocessorenumerator-checkvideoprocessorformat).
    fn check_video_processor_format(
        &self,
        format: directx_graphics_infrastructure::enumerations::Format,
    ) -> Result<u32, HResult>;
    /// The header's `GetVideoProcessorCaps`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videoprocessorenumerator-getvideoprocessorcaps).
    fn get_video_processor_caps(&self) -> Result<structures::VideoProcessorCaps, HResult>;
    /// The header's `GetVideoProcessorRateConversionCaps`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videoprocessorenumerator-getvideoprocessorrateconversioncaps).
    fn get_video_processor_rate_conversion_caps(
        &self,
        type_index: u32,
    ) -> Result<structures::VideoProcessorRateConversionCaps, HResult>;
    /// The header's `GetVideoProcessorCustomRate`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videoprocessorenumerator-getvideoprocessorcustomrate).
    fn get_video_processor_custom_rate(
        &self,
        type_index: u32,
        custom_rate_index: u32,
    ) -> Result<structures::VideoProcessorCustomRate, HResult>;
    /// The header's `GetVideoProcessorFilterRange`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videoprocessorenumerator-getvideoprocessorfilterrange).
    fn get_video_processor_filter_range(
        &self,
        filter: enumerations::VideoProcessorFilter,
    ) -> Result<structures::VideoProcessorFilterRange, HResult>;
}

/// The header's `ID3D11VideoProcessorEnumerator1`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nn-d3d11_1-id3d11videoprocessorenumerator1).
pub trait VideoProcessorEnumerator1: VideoProcessorEnumerator {
    /// The header's `CheckVideoProcessorFormatConversion`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11_1/nf-d3d11_1-id3d11videoprocessorenumerator1-checkvideoprocessorformatconversion).
    fn check_video_processor_format_conversion(
        &self,
        input_format: directx_graphics_infrastructure::enumerations::Format,
        input_color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
        output_format: directx_graphics_infrastructure::enumerations::Format,
        output_color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
    ) -> Result<bool, HResult>;
}

/// The header's `ID3D11VideoProcessorInputView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videoprocessorinputview).
pub trait VideoProcessorInputView: View {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videoprocessorinputview-getdesc).
    fn get_desc(&self) -> structures::VideoProcessorInputViewDesc;
}

/// The header's `ID3D11VideoProcessorOutputView`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11videoprocessoroutputview).
pub trait VideoProcessorOutputView: View {
    /// The header's `GetDesc`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11videoprocessoroutputview-getdesc).
    fn get_desc(&self) -> structures::VideoProcessorOutputViewDesc;
}

/// The header's `ID3D11View`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nn-d3d11-id3d11view).
pub trait View: DeviceChild {
    /// The header's `GetResource`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3d11/nf-d3d11-id3d11view-getresource).
    fn get_resource(&self) -> Option<ResourceObject<Self::Api>>;
}

/// An `ID3D11Asynchronous*`: one of the kinds of object that are one.
pub enum AsynchronousRef<'a, A: Direct3D11> {
    Counter(&'a <A as Direct3D11>::Counter),
    Predicate(&'a <A as Direct3D11>::Predicate),
    Query(&'a <A as Direct3D11>::Query),
}

/// An `ID3D11DeviceChild*`: one of the kinds of object that are one.
pub enum DeviceChildRef<'a, A: Direct3D11> {
    AuthenticatedChannel(&'a <A as Direct3D11>::AuthenticatedChannel),
    BlendState(&'a <A as Direct3D11>::BlendState),
    Buffer(&'a <A as Direct3D11>::Buffer),
    ClassInstance(&'a <A as Direct3D11>::ClassInstance),
    ClassLinkage(&'a <A as Direct3D11>::ClassLinkage),
    CommandList(&'a <A as Direct3D11>::CommandList),
    ComputeShader(&'a <A as Direct3D11>::ComputeShader),
    Counter(&'a <A as Direct3D11>::Counter),
    CryptoSession(&'a <A as Direct3D11>::CryptoSession),
    DepthStencilState(&'a <A as Direct3D11>::DepthStencilState),
    DepthStencilView(&'a <A as Direct3D11>::DepthStencilView),
    DeviceContext(&'a <A as Direct3D11>::DeviceContext),
    DomainShader(&'a <A as Direct3D11>::DomainShader),
    Fence(&'a <A as Direct3D11>::Fence),
    GeometryShader(&'a <A as Direct3D11>::GeometryShader),
    HullShader(&'a <A as Direct3D11>::HullShader),
    InputLayout(&'a <A as Direct3D11>::InputLayout),
    PixelShader(&'a <A as Direct3D11>::PixelShader),
    Predicate(&'a <A as Direct3D11>::Predicate),
    Query(&'a <A as Direct3D11>::Query),
    RasterizerState(&'a <A as Direct3D11>::RasterizerState),
    RenderTargetView(&'a <A as Direct3D11>::RenderTargetView),
    SamplerState(&'a <A as Direct3D11>::SamplerState),
    ShaderResourceView(&'a <A as Direct3D11>::ShaderResourceView),
    Texture1D(&'a <A as Direct3D11>::Texture1D),
    Texture2D(&'a <A as Direct3D11>::Texture2D),
    Texture3D(&'a <A as Direct3D11>::Texture3D),
    UnorderedAccessView(&'a <A as Direct3D11>::UnorderedAccessView),
    VertexShader(&'a <A as Direct3D11>::VertexShader),
    VideoContext(&'a <A as Direct3D11>::VideoContext),
    VideoDecoder(&'a <A as Direct3D11>::VideoDecoder),
    VideoDecoderOutputView(&'a <A as Direct3D11>::VideoDecoderOutputView),
    VideoProcessor(&'a <A as Direct3D11>::VideoProcessor),
    VideoProcessorEnumerator(&'a <A as Direct3D11>::VideoProcessorEnumerator),
    VideoProcessorInputView(&'a <A as Direct3D11>::VideoProcessorInputView),
    VideoProcessorOutputView(&'a <A as Direct3D11>::VideoProcessorOutputView),
    DeviceContextState(&'a <A as Direct3D11>::DeviceContextState),
}

/// An `ID3D11Resource*`: one of the kinds of object that are one.
pub enum ResourceRef<'a, A: Direct3D11> {
    Buffer(&'a <A as Direct3D11>::Buffer),
    Texture1D(&'a <A as Direct3D11>::Texture1D),
    Texture2D(&'a <A as Direct3D11>::Texture2D),
    Texture3D(&'a <A as Direct3D11>::Texture3D),
}

/// An `ID3D11View*`: one of the kinds of object that are one.
pub enum ViewRef<'a, A: Direct3D11> {
    DepthStencilView(&'a <A as Direct3D11>::DepthStencilView),
    RenderTargetView(&'a <A as Direct3D11>::RenderTargetView),
    ShaderResourceView(&'a <A as Direct3D11>::ShaderResourceView),
    UnorderedAccessView(&'a <A as Direct3D11>::UnorderedAccessView),
    VideoDecoderOutputView(&'a <A as Direct3D11>::VideoDecoderOutputView),
    VideoProcessorInputView(&'a <A as Direct3D11>::VideoProcessorInputView),
    VideoProcessorOutputView(&'a <A as Direct3D11>::VideoProcessorOutputView),
}

/// An `ID3D11Query*`: one of the kinds of object that are one.
pub enum QueryObject<A: Direct3D11> {
    Predicate(<A as Direct3D11>::Predicate),
    Query(<A as Direct3D11>::Query),
}

/// An `ID3D11Resource*`: one of the kinds of object that are one.
pub enum ResourceObject<A: Direct3D11> {
    Buffer(<A as Direct3D11>::Buffer),
    Texture1D(<A as Direct3D11>::Texture1D),
    Texture2D(<A as Direct3D11>::Texture2D),
    Texture3D(<A as Direct3D11>::Texture3D),
}

/// An `IUnknown*`: an object of one of this API's kinds, or of none.
pub enum UnknownRef<'a, A: Direct3D11> {
    AuthenticatedChannel(&'a <A as Direct3D11>::AuthenticatedChannel),
    BlendState(&'a <A as Direct3D11>::BlendState),
    Buffer(&'a <A as Direct3D11>::Buffer),
    ClassInstance(&'a <A as Direct3D11>::ClassInstance),
    ClassLinkage(&'a <A as Direct3D11>::ClassLinkage),
    CommandList(&'a <A as Direct3D11>::CommandList),
    ComputeShader(&'a <A as Direct3D11>::ComputeShader),
    Counter(&'a <A as Direct3D11>::Counter),
    CryptoSession(&'a <A as Direct3D11>::CryptoSession),
    Debug(&'a <A as Direct3D11>::Debug),
    DepthStencilState(&'a <A as Direct3D11>::DepthStencilState),
    DepthStencilView(&'a <A as Direct3D11>::DepthStencilView),
    Device(&'a <A as Direct3D11>::Device),
    DeviceContext(&'a <A as Direct3D11>::DeviceContext),
    DomainShader(&'a <A as Direct3D11>::DomainShader),
    Fence(&'a <A as Direct3D11>::Fence),
    GeometryShader(&'a <A as Direct3D11>::GeometryShader),
    HullShader(&'a <A as Direct3D11>::HullShader),
    InfoQueue(&'a <A as Direct3D11>::InfoQueue),
    InputLayout(&'a <A as Direct3D11>::InputLayout),
    Multithread(&'a <A as Direct3D11>::Multithread),
    PixelShader(&'a <A as Direct3D11>::PixelShader),
    Predicate(&'a <A as Direct3D11>::Predicate),
    Query(&'a <A as Direct3D11>::Query),
    RasterizerState(&'a <A as Direct3D11>::RasterizerState),
    RefDefaultTrackingOptions(&'a <A as Direct3D11>::RefDefaultTrackingOptions),
    RefTrackingOptions(&'a <A as Direct3D11>::RefTrackingOptions),
    RenderTargetView(&'a <A as Direct3D11>::RenderTargetView),
    SamplerState(&'a <A as Direct3D11>::SamplerState),
    ShaderResourceView(&'a <A as Direct3D11>::ShaderResourceView),
    ShaderTrace(&'a <A as Direct3D11>::ShaderTrace),
    ShaderTraceFactory(&'a <A as Direct3D11>::ShaderTraceFactory),
    SwitchToRef(&'a <A as Direct3D11>::SwitchToRef),
    Texture1D(&'a <A as Direct3D11>::Texture1D),
    Texture2D(&'a <A as Direct3D11>::Texture2D),
    Texture3D(&'a <A as Direct3D11>::Texture3D),
    TracingDevice(&'a <A as Direct3D11>::TracingDevice),
    UnorderedAccessView(&'a <A as Direct3D11>::UnorderedAccessView),
    VertexShader(&'a <A as Direct3D11>::VertexShader),
    VideoContext(&'a <A as Direct3D11>::VideoContext),
    VideoDecoder(&'a <A as Direct3D11>::VideoDecoder),
    VideoDecoderOutputView(&'a <A as Direct3D11>::VideoDecoderOutputView),
    VideoDevice(&'a <A as Direct3D11>::VideoDevice),
    VideoProcessor(&'a <A as Direct3D11>::VideoProcessor),
    VideoProcessorEnumerator(&'a <A as Direct3D11>::VideoProcessorEnumerator),
    VideoProcessorInputView(&'a <A as Direct3D11>::VideoProcessorInputView),
    VideoProcessorOutputView(&'a <A as Direct3D11>::VideoProcessorOutputView),
    DeviceContextState(&'a <A as Direct3D11>::DeviceContextState),
    UserDefinedAnnotation(&'a <A as Direct3D11>::UserDefinedAnnotation),
    Unknown(&'a component_object_model::structures::Foreign),
}

/// An `IUnknown*`: an object of one of this API's kinds, or of none.
pub enum UnknownObject<A: Direct3D11> {
    AuthenticatedChannel(<A as Direct3D11>::AuthenticatedChannel),
    BlendState(<A as Direct3D11>::BlendState),
    Buffer(<A as Direct3D11>::Buffer),
    ClassInstance(<A as Direct3D11>::ClassInstance),
    ClassLinkage(<A as Direct3D11>::ClassLinkage),
    CommandList(<A as Direct3D11>::CommandList),
    ComputeShader(<A as Direct3D11>::ComputeShader),
    Counter(<A as Direct3D11>::Counter),
    CryptoSession(<A as Direct3D11>::CryptoSession),
    Debug(<A as Direct3D11>::Debug),
    DepthStencilState(<A as Direct3D11>::DepthStencilState),
    DepthStencilView(<A as Direct3D11>::DepthStencilView),
    Device(<A as Direct3D11>::Device),
    DeviceContext(<A as Direct3D11>::DeviceContext),
    DomainShader(<A as Direct3D11>::DomainShader),
    Fence(<A as Direct3D11>::Fence),
    GeometryShader(<A as Direct3D11>::GeometryShader),
    HullShader(<A as Direct3D11>::HullShader),
    InfoQueue(<A as Direct3D11>::InfoQueue),
    InputLayout(<A as Direct3D11>::InputLayout),
    Multithread(<A as Direct3D11>::Multithread),
    PixelShader(<A as Direct3D11>::PixelShader),
    Predicate(<A as Direct3D11>::Predicate),
    Query(<A as Direct3D11>::Query),
    RasterizerState(<A as Direct3D11>::RasterizerState),
    RefDefaultTrackingOptions(<A as Direct3D11>::RefDefaultTrackingOptions),
    RefTrackingOptions(<A as Direct3D11>::RefTrackingOptions),
    RenderTargetView(<A as Direct3D11>::RenderTargetView),
    SamplerState(<A as Direct3D11>::SamplerState),
    ShaderResourceView(<A as Direct3D11>::ShaderResourceView),
    ShaderTrace(<A as Direct3D11>::ShaderTrace),
    ShaderTraceFactory(<A as Direct3D11>::ShaderTraceFactory),
    SwitchToRef(<A as Direct3D11>::SwitchToRef),
    Texture1D(<A as Direct3D11>::Texture1D),
    Texture2D(<A as Direct3D11>::Texture2D),
    Texture3D(<A as Direct3D11>::Texture3D),
    TracingDevice(<A as Direct3D11>::TracingDevice),
    UnorderedAccessView(<A as Direct3D11>::UnorderedAccessView),
    VertexShader(<A as Direct3D11>::VertexShader),
    VideoContext(<A as Direct3D11>::VideoContext),
    VideoDecoder(<A as Direct3D11>::VideoDecoder),
    VideoDecoderOutputView(<A as Direct3D11>::VideoDecoderOutputView),
    VideoDevice(<A as Direct3D11>::VideoDevice),
    VideoProcessor(<A as Direct3D11>::VideoProcessor),
    VideoProcessorEnumerator(<A as Direct3D11>::VideoProcessorEnumerator),
    VideoProcessorInputView(<A as Direct3D11>::VideoProcessorInputView),
    VideoProcessorOutputView(<A as Direct3D11>::VideoProcessorOutputView),
    DeviceContextState(<A as Direct3D11>::DeviceContextState),
    UserDefinedAnnotation(<A as Direct3D11>::UserDefinedAnnotation),
    Unknown(component_object_model::structures::Foreign),
}
